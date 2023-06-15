use super::drivers::{ipc::FifoDriver, DefaultDriver, Driver, FileOperation};
use super::kmodules::CURRENT_UNIX_TIME;
use super::sync::SmartMutex;
use super::thread_group::Credentials;
use super::{IpcResult, SysResult};

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::sync::atomic::Ordering;
use fallible_collections::{btree::BTreeMap, FallibleArc, FallibleBox, TryCollect};
use lazy_static::lazy_static;
use sync::DeadMutex;

use fallible_collections::TryClone;
use itertools::unfold;

mod tools;
use tools::{Incrementor, KeyGenerator};

mod path;
pub mod posix_consts;
pub use posix_consts::{NAME_MAX, PATH_MAX, SYMLOOP_MAX};

pub use path::{Filename, Path};

mod direntry;
pub use direntry::{DirectoryEntry, DirectoryEntryBuilder, DirectoryEntryId};

mod dcache;

pub use dcache::Dcache;

mod inode;
pub use inode::InodeId;
use inode::{Inode, InodeData};
use libc_binding::OpenFlags;

use libc_binding::c_char;
use libc_binding::dirent;
use libc_binding::statfs;
use libc_binding::Errno::*;
use libc_binding::FileType;
use libc_binding::{gid_t, stat, time_t, uid_t, utimbuf, Amode, Errno};

pub mod init;
pub use init::{init, VFS};

mod filesystem;
use filesystem::{DeadFileSystem, FileSystem, FileSystemId, FileSystemSource, FileSystemType};

pub struct VirtualFileSystem {
    mounted_filesystems: BTreeMap<FileSystemId, MountedFileSystem>,

    // superblocks: Vec<Superblock>,
    inodes: BTreeMap<InodeId, Inode>,
    dcache: Dcache,
}

pub struct MountedFileSystem {
    source: FileSystemSource,
    fs_type: FileSystemType,
    target: Path,
    fs: Arc<DeadMutex<dyn FileSystem>>,
}

use core::fmt::{self, Debug};

impl Debug for VirtualFileSystem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VirtualFileSystem")
    }
}

#[allow(unused)]
type Vfs = VirtualFileSystem;

impl VirtualFileSystem {
    pub fn new() -> SysResult<VirtualFileSystem> {
        let mut new = Self {
            mounted_filesystems: BTreeMap::new(),
            inodes: BTreeMap::new(),
            dcache: Dcache::new(),
        };

        let root_inode = Inode::root_inode()?;
        let root_inode_id = root_inode.id;

        new.inodes.try_insert(root_inode_id, root_inode)?;
        Ok(new)
    }
    fn add_inode(&mut self, inode: Inode) -> SysResult<()> {
        if self.inodes.contains_key(&inode.get_id()) {
            // if it is not from an hard link we panic
            if inode.link_number == 1 {
                // panic!("inode already there {:?}", inode);
                panic!("Inode already there");
            } else {
                // else we already put the inode pointed by the hard link
                return Ok(());
            }
        }
        self.inodes.try_insert(inode.get_id(), inode)?;
        Ok(())
    }

    fn get_filesystem(&self, inode_id: InodeId) -> Option<&Arc<DeadMutex<dyn FileSystem>>> {
        Some(&self.mounted_filesystems.get(&inode_id.filesystem_id?)?.fs)
    }

    fn get_filesystem_mut(
        &mut self,
        inode_id: InodeId,
    ) -> Option<&mut Arc<DeadMutex<dyn FileSystem>>> {
        Some(
            &mut self
                .mounted_filesystems
                .get_mut(&inode_id.filesystem_id?)?
                .fs,
        )
    }

    fn add_entry_from_filesystem(
        &mut self,
        fs: Arc<DeadMutex<dyn FileSystem>>,
        parent: Option<DirectoryEntryId>,
        (direntry, inode_data, driver): (DirectoryEntry, InodeData, Option<Box<dyn Driver>>),
    ) -> SysResult<DirectoryEntryId> {
        if self
            .dcache
            .children(parent.unwrap_or(DirectoryEntryId::new(2)))?
            .any(|entry| entry.filename == direntry.filename)
        {
            return Err(Errno::EEXIST);
        }
        // eprintln!(
        //     "Adding inode {:?} for direntry: {}",
        //     inode_data.id, direntry.filename
        // );

        let direntry = self.dcache.add_entry(parent, direntry)?;

        let inode = Inode::new(
            fs,
            // For now, we're just gonna override the fifo filetype's drivers.
            if inode_data.is_fifo() {
                Box::try_new(FifoDriver::try_new(inode_data.id)?)?
            } else if inode_data.is_socket() {
                Box::try_new(DefaultDriver)?
            } else {
                if let Some(driver) = driver {
                    driver
                } else {
                    log::warn!("Default driver registered for id: {:?}", inode_data.id);
                    Box::try_new(DefaultDriver)?
                }
                // // TODO: handle others drivers
                // Box::try_new(Ext2DriverFile::new(inode_data.id))?
            },
            inode_data,
        );
        self.add_inode(inode)?;
        Ok(direntry)
    }

    fn recursive_remove_dentries(&mut self, direntry_id: DirectoryEntryId) -> SysResult<()> {
        let children: Vec<DirectoryEntryId> = self
            .iter_directory_entries(direntry_id)?
            .map(|entry| entry.id)
            .try_collect()?;

        Ok(for child in children {
            let entry = self
                .dcache
                .get_entry(&child)
                .expect("There should be a child here");

            // let inode_id = entry.inode_id;
            if entry.is_directory() {
                self.recursive_remove_dentries(child)?;
            }
            // This means that dynamic filesystems shall not support multiple hardlinks for now.
            // self.inodes.remove(&inode_id).ok_or(Errno::ENOENT)?;
            // eprintln!("Callling funlink.");
            self.funlink(child)?;
            // self.dcache.remove_entry(child)?;
        })
    }

    /// construct the files in directory `direntry_id` in ram form the filesystem
    /// Removes every files previously contained in it from the VFS.
    fn lookup_directory(&mut self, direntry_id: DirectoryEntryId) -> SysResult<()> {
        // unimplemented!()
        // removes entries already existing. (cause that can only (for now) happen in dynamic filesystems (aka. procfs)).
        // TODO: remove the inodes also...
        self.recursive_remove_dentries(direntry_id)?;

        let current_entry = self.dcache.get_entry(&direntry_id)?;
        let inode_id = current_entry.inode_id;

        let fs_cloned = self.get_filesystem(inode_id).ok_or(Errno::EINVAL)?.clone();
        let iter = self
            .get_filesystem(inode_id)
            .ok_or(Errno::EINVAL)?
            .lock()
            .lookup_directory(inode_id.inode_number as u32)?;

        for (direntry, inode_data, driver) in iter {
            let fs_entry = (direntry, inode_data, Some(driver));
            self.add_entry_from_filesystem(fs_cloned.clone(), Some(direntry_id), fs_entry)
                .or_else(|e| {
                    if e == Errno::EEXIST {
                        panic!("lookup_directory: Tried to add entry on an existing one when none was expected to exist.");
                    } else {
                        Err(e)
                    }
                })
                .expect("add entry from filesystem failed");
        }
        Ok(())
    }

    /// Construct a path from a DirectoryEntryId by follow up its
    /// parent
    pub fn dentry_path(&self, id: DirectoryEntryId) -> SysResult<Path> {
        let mut rev_path = self.dcache.dentry_path(id)?;
        rev_path.set_absolute(true)?;
        Ok(rev_path)
    }

    /// resolve the path `pathname` from root `root`, return the
    /// directory_entry_id associate with the file, used for lstat
    pub fn pathname_resolution_no_follow_last_symlink(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        pathname: &Path,
    ) -> SysResult<DirectoryEntryId> {
        let root = if pathname.is_absolute() {
            self.dcache.root_id
        } else {
            debug_assert!(cwd.is_absolute());
            self._pathname_resolution(self.dcache.root_id, creds, cwd, 0, true)?
        };
        self._pathname_resolution(root, creds, pathname, 0, false)
    }

    /// resolve the path `pathname` from root `root`, return the
    /// directory_entry_id associate with the file
    pub fn pathname_resolution(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        pathname: &Path,
    ) -> SysResult<DirectoryEntryId> {
        let root = if pathname.is_absolute() {
            self.dcache.root_id
        } else {
            debug_assert!(cwd.is_absolute());
            self._pathname_resolution(self.dcache.root_id, creds, cwd, 0, true)?
        };
        self._pathname_resolution(root, creds, pathname, 0, true)
    }

    /// this method follow the mount point
    /// if current_entry is mounted, it set current_entry and
    /// current_dir_id to the direntry and direntry_id of the mount
    /// point
    fn handle_mount_point<'a>(
        &'a self,
        current_entry: &mut &'a DirectoryEntry,
        current_dir_id: &mut DirectoryEntryId,
    ) {
        if let Ok(true) = current_entry.is_mounted() {
            *current_dir_id = current_entry
                .get_mountpoint_entry()
                .expect("mount point entry should be there");
            *current_entry// : &DirectoryEntry
                = self
                .dcache
                .get_entry(current_dir_id)
                .expect("mount point should be there");
        }
    }

    fn _pathname_resolution(
        &mut self,
        mut root: DirectoryEntryId,
        creds: &Credentials,
        pathname: &Path,
        recursion_level: usize,
        follow_last_symlink: bool,
    ) -> SysResult<DirectoryEntryId> {
        if recursion_level > SYMLOOP_MAX {
            return Err(Errno::ELOOP);
        }

        if pathname.is_absolute() {
            root = self.dcache.root_id;
        }

        if !self.dcache.contains_entry(&root) {
            return Err(ENOENT);
        }

        let mut current_dir_id = root;
        let mut components = pathname.components();
        let mut was_symlink = false;
        let mut current_entry = self.dcache.get_entry(&current_dir_id)?;

        self.handle_mount_point(&mut current_entry, &mut current_dir_id);
        // quick fix, this handle / mount point
        for component in components.by_ref() {
            self.handle_mount_point(&mut current_entry, &mut current_dir_id);

            let current_dir = current_entry.get_directory()?;

            if component == &"." {
                continue;
            } else if component == &".." {
                current_dir_id = current_entry.parent_id;
                current_entry = self.dcache.get_entry(&current_dir_id)?;
                self.handle_mount_point(&mut current_entry, &mut current_dir_id);
                continue;
            }

            if {
                let inode = self
                    .inodes
                    .get(&current_entry.inode_id)
                    .expect("No corresponding inode fir direntry");
                !creds.is_access_granted(inode.access_mode, Amode::SEARCH, (inode.uid, inode.gid))
            } {
                return Err(Errno::EACCES);
            }
            let should_lookup = {
                current_dir.entries().count() == 0
                    || self
                        .get_filesystem(current_entry.inode_id)
                        .expect("No corresonding filesystem for direntry")
                        .lock()
                        .is_dynamic()
            };

            if should_lookup {
                self.lookup_directory(current_dir_id)?;
                current_entry = self.dcache.get_entry(&current_dir_id)?;
                let current_dir = current_entry.get_directory()?;
                //TODO:
                let next_entry_id = current_dir
                    .entries()
                    .find(|x| {
                        let filename = &self
                            .dcache
                            .get_entry(x)
                            .expect("Invalid entry id in a directory entry that is a directory")
                            .filename;
                        filename == component
                    })
                    .ok_or(ENOENT)?;

                current_entry = self.dcache.get_entry(next_entry_id)?;
                if current_entry.is_symlink() {
                    was_symlink = true;
                    break;
                }
                // current_di_id is set after checking if we are on a
                // symlink, as on a symlink current_dir_id must point
                // to the directory, not the symlink
                current_dir_id = *next_entry_id;
                continue;
            }
            let next_entry_id = current_dir
                .entries()
                .find(|x| {
                    let filename = &self
                        .dcache
                        .get_entry(x)
                        .expect("Invalid entry id in a directory entry that is a directory")
                        .filename;
                    filename == component
                })
                .ok_or(ENOENT)?;

            current_entry = self.dcache.get_entry(next_entry_id)?;
            if current_entry.is_symlink() {
                was_symlink = true;
                break;
            }
            // current_di_id is set after checking if we are on a
            // symlink, as on a symlink current_dir_id must point
            // to the directory, not the symlink
            current_dir_id = *next_entry_id;
            self.handle_mount_point(&mut current_entry, &mut current_dir_id);
        }
        if was_symlink {
            if components.len() == 0 && !follow_last_symlink {
                return Ok(current_entry.id);
            }
            let mut new_path = current_entry
                .get_symbolic_content()
                .expect("should be symlink")
                .clone();
            new_path.chain(components.try_into()?)?;

            self._pathname_resolution(
                current_dir_id,
                creds,
                &new_path,
                recursion_level + 1,
                follow_last_symlink,
            )
        } else {
            Ok(self.dcache.get_entry(&current_dir_id).unwrap().id)
        }
    }

    pub fn get_driver(&mut self, inode_id: InodeId) -> SysResult<&mut dyn Driver> {
        Ok(self.get_inode(inode_id).map_err(|_| ENOENT)?.get_driver())
    }
    /// Ici j'enregistre un filename associe a son driver (que je
    /// provide depuis l'ipc)
    /// constrainte: Prototype, filename et Arc<DeadMutex<dyn Driver>>
    /// en param
    /// Je pense pas qu'il soit oblige d'envoyer un
    /// Arc<DeadMutes<...>> ici, une simple Box<dyn ...> pourrait
    /// faire l'affaire
    /// L'arc ca peut apporter un avantage pour gerer les liens
    /// symboliques en interne, mais c'est tout relatif
    /// Je te passe l'ownership complet du 'Driver'
    pub fn new_driver(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        mode: FileType,
        driver: Box<dyn Driver>,
    ) -> SysResult<()> {
        // la fonction driver.set_inode_id() doit etre appele lors de la creation. C'est pour joindre l'inode au cas ou
        // Je ne sais pas encore si ce sera completement indispensable. Il vaut mieux que ce soit un type primitif afin
        // qu'il n'y ait pas de reference croisees (j'ai mis usize dans l'exemple)

        // let entry_id;
        match self.pathname_resolution(cwd, creds, &path) {
            Ok(_id) => return Err(EEXIST),
            Err(_e) => {
                //TODO: Option(FileSystemId)
                let new_id = self.get_available_id(None);

                let mut inode_data: InodeData = Default::default();
                inode_data
                    .set_id(new_id)
                    .set_access_mode(mode)
                    .set_uid(creds.uid)
                    .set_gid(creds.gid); // posix does not really like this.

                inode_data.link_number += 1;

                let new_inode = Inode::new(
                    Arc::try_new(DeadMutex::new(DeadFileSystem))?,
                    driver,
                    inode_data,
                );

                let mut new_direntry = DirectoryEntry::default();
                let parent_id = self.pathname_resolution(cwd, creds, &path.parent()?)?;

                new_direntry
                    .set_filename(*path.filename().unwrap())
                    .set_inode_id(new_id);

                new_direntry.set_regular();

                self.add_inode(new_inode)?;
                self.dcache
                    .add_entry(Some(parent_id), new_direntry)
                    /*CLEANUP*/
                    .map_err(|e| {
                        self.inodes.remove(&new_id);
                        e
                    })?;
            }
        }

        // let entry = self.dcache.get_entry(&entry_id)?;
        // let entry_inode_id = entry.inode_id;
        // let entry_id = entry.id;
        // self.open_inode(current, entry_inode_id, entry_id, flags);
        Ok(())
    }

    #[allow(dead_code)]
    fn iter_directory_entries(
        &self,
        dir: DirectoryEntryId,
    ) -> SysResult<impl Iterator<Item = &DirectoryEntry>> {
        let dir = self.dcache.get_entry(&dir)?.get_directory()?;

        let mut entries = dir.entries();
        Ok(unfold((), move |_| {
            if let Some(entry_id) = entries.next() {
                let entry = self
                    .dcache
                    .get_entry(&entry_id)
                    .expect("Some entries from this directory are corrupted");
                Some(entry)
            } else {
                None
            }
        }))
    }

    /// Returns the FileType of the file pointed by the Path `path`.
    pub fn file_type(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: &Path,
    ) -> SysResult<FileType> {
        let direntry_id = self.pathname_resolution(cwd, creds, &path)?;
        let inode_id = &self
            .dcache
            .get_entry(&direntry_id)
            .expect("Dcache is corrupted: Could not find expected direntry")
            .inode_id;
        Ok(self
            .inodes
            .get(inode_id)
            .expect("Vfs Inodes are corrupted: Could not find expected inode")
            .access_mode)
    }

    /// Returns the owner (uid) and group (gid) of the file pointed by the Path `path`.
    pub fn get_file_owner(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: &Path,
    ) -> SysResult<(uid_t, gid_t)> {
        let direntry_id = self.pathname_resolution(cwd, creds, &path)?;
        let inode_id = &self
            .dcache
            .get_entry(&direntry_id)
            .expect("Dcache is corrupted: Could not find expected direntry")
            .inode_id;
        let inode = self
            .inodes
            .get(inode_id)
            .expect("Vfs Inodes are corrupted: Could not find expected inode");

        Ok((inode.uid, inode.gid))
    }

    // fn recursive_build_subtree(
    //     // This should be refactored with recursive_creat.
    //     &mut self,
    //     current_dir_id: DirectoryEntryId,
    //     fs_id: FileSystemId,
    // ) -> SysResult<()> {
    //     let direntry = self.dcache.get_entry(&current_dir_id)?;

    //     // Inode unexpectedly does not exists...
    //     let inode = self.inodes.get(&direntry.inode_id).ok_or(ENOENT)?;

    //     if !inode.is_directory() {
    //         return Ok(());
    //     }
    //     let entries = inode
    //         .inode_operations
    //         .lookup_entries
    //         .expect("Directory does not have lookup_entries() method")(&inode);

    //     for mut entry in entries {
    //         let fs = self.mounted_filesystems.get(&fs_id).unwrap(); // remove this unwrap

    //         entry.inode_id.filesystem_id = fs_id;

    //         let mut new_inode = fs.load_inode(entry.inode_id.inode_number).unwrap(); // fix this unwrap
    //         new_inode.id.filesystem_id = fs_id;
    //         let inode_id = new_inode.id;

    //         // clean up in error case (if any)
    //         let entry_id = self.dcache.add_entry(Some(current_dir_id), entry)?;
    //         let is_dir = new_inode.is_directory();
    //         self.inodes.insert(inode_id, new_inode).unwrap(); // fix this unwrap.
    //         if is_dir {
    //             self.recursive_build_subtree(entry_id, fs_id)?
    //         }
    //     }
    //     Ok(())
    // }
    /// Mount the filesystem `filesystem` with filesystem id `fs_id`
    /// on mount dir `mount_dir_id`
    pub fn mount_filesystem(
        &mut self,
        filesystem: MountedFileSystem,
        fs_id: FileSystemId,
        mount_dir_id: DirectoryEntryId,
    ) -> SysResult<()> {
        let mount_dir = self.dcache.get_entry_mut(&mount_dir_id)?;
        if !mount_dir.is_directory() {
            return Err(ENOTDIR);
        }

        if mount_dir.is_mounted()? {
            return Err(EBUSY);
        }
        let (mut root_dentry, mut root_inode_data, driver) = filesystem.fs.lock().root()?;

        root_inode_data.id.filesystem_id = Some(fs_id);
        root_dentry.inode_id = root_inode_data.id;

        let root_dentry_id = self.add_entry_from_filesystem(
            filesystem.fs.clone(),
            Some(mount_dir_id),
            (root_dentry, root_inode_data, Some(driver)),
        )?;

        let mount_dir = self
            .dcache
            .get_entry_mut(&mount_dir_id)
            .expect("WTF: mount_dir_id should be valid");
        mount_dir
            .set_mounted(root_dentry_id)
            .expect("WTF: and should be a directory");

        self.mounted_filesystems.try_insert(fs_id, filesystem)?;
        //TODO: cleanup root dentry_id

        Ok(())
    }

    /// mount the source `source` on the target `target`
    pub fn mount(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        source: Path,
        target: Path,
    ) -> SysResult<()> {
        use ext2::Ext2Filesystem;
        use filesystem::devfs::DiskWrapper;
        use filesystem::Ext2fs;

        let flags = libc_binding::OpenFlags::O_RDWR;
        let mode = FileType::from_bits(0o777).expect("file permission creation failed");
        let source_path = self.resolve_path(cwd, creds, &source)?;
        let file_operation = self
            .open(cwd, creds, source, flags, mode)
            .expect("open sda1 failed")
            .expect("disk driver open failed");

        let ext2_disk = DiskWrapper(file_operation);
        VFS.force_unlock();

        let ext2 = Ext2Filesystem::new(Box::try_new(ext2_disk)?).map_err(|_| Errno::EINVAL)?;
        let fs_id: FileSystemId = self.gen();

        // we handle only ext2 fs right now
        // HARDFIX: `mount kernel.elf .` should be solved in a better way.
        let filesystem = Ext2fs::new(ext2, fs_id);
        let mount_dir_id = self.pathname_resolution(cwd, creds, &target)?;
        let target = self.resolve_path(cwd, creds, &target)?;
        self.mount_filesystem(
            MountedFileSystem {
                source: FileSystemSource::File { source_path },
                // we only handle ext2
                fs_type: FileSystemType::Ext2,
                target,
                fs: Arc::try_new(DeadMutex::new(filesystem))?,
            },
            fs_id,
            mount_dir_id,
        )
    }

    fn recursive_trash(&mut self, root_dentry_id: DirectoryEntryId) {
        let direntry = self.dcache.d_entries.remove(&root_dentry_id);
        if let Some(direntry) = direntry {
            self.inodes.remove(&direntry.inode_id);
            if let Ok(directory) = direntry.get_directory() {
                for child in directory.entries() {
                    self.recursive_trash(*child);
                }
            }
        }
    }

    pub fn umount(&mut self, cwd: &Path, creds: &Credentials, path: Path) -> SysResult<()> {
        let mut mount_dir_id = self.pathname_resolution(cwd, creds, &path)?;
        let mount_dir = self.dcache.get_entry_mut(&mount_dir_id)?;
        // get the parent to have the mount point as pathname
        // resolution follow mount points
        mount_dir_id = mount_dir.parent_id;
        let mount_dir = self.dcache.get_entry_mut(&mount_dir_id)?;

        if !mount_dir.is_mounted()? {
            return Err(EINVAL);
        }

        let root_dentry_id = mount_dir.get_mountpoint_entry().ok_or(EINVAL)?;

        // this set the mount_dir as unmouted
        mount_dir.unset_mounted()?;
        mount_dir.remove_entry(root_dentry_id)?;

        let mounted_dir = self.dcache.get_entry_mut(&root_dentry_id)?;
        let fs_id = mounted_dir.inode_id.filesystem_id.ok_or(EINVAL)?;

        self.recursive_trash(root_dentry_id);

        self.mounted_filesystems.remove(&fs_id);

        Ok(())
    }

    pub fn opendir(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
    ) -> SysResult<Vec<dirent>> {
        //TODO check this function
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;

        let inode = self
            .inodes
            .get(&entry.inode_id)
            .expect("No corresponding Inode for direntry");
        if !creds.is_access_granted(
            // check for write permission in the parent.
            inode.access_mode,
            Amode::READ,
            (inode.uid, inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        let entry_count = entry.get_directory()?.entries().count();

        let should_lookup = {
            entry_count == 0
                || self
                    .get_filesystem(entry.inode_id)
                    .expect("No corresonding filesystem for direntry")
                    .lock()
                    .is_dynamic()
        };

        if should_lookup {
            self.lookup_directory(entry_id)?;
        }

        let direntry = self.dcache.get_entry(&entry_id)?;
        let dir = direntry.get_directory()?;
        Ok(dir
            .entries()
            .map(|e| {
                let child = self
                    .dcache
                    .get_entry(&e)
                    .expect("entry not found vfs is bullshit");
                child.dirent()
            })
            // recreate on the fly the . and .. file as it is not stocked
            // in the vfs
            .chain(Some(dirent {
                d_name: {
                    let mut name = [0; NAME_MAX + 1];
                    name[0] = '.' as c_char;
                    name
                },
                d_ino: direntry.inode_id.inode_number as u32,
            }))
            .chain(Some(dirent {
                d_name: {
                    let mut name = [0; NAME_MAX + 1];
                    name[0] = '.' as c_char;
                    name[1] = '.' as c_char;
                    name
                },
                d_ino: self
                    .dcache
                    .get_entry(&direntry.parent_id)
                    .unwrap()
                    .inode_id
                    .inode_number as u32,
            }))
            .try_collect()?)
    }

    pub fn unlink(&mut self, cwd: &Path, creds: &Credentials, path: Path) -> SysResult<()> {
        let entry_id = self.pathname_resolution_no_follow_last_symlink(cwd, creds, &path)?;
        let parent_id;

        {
            let entry = self.dcache.get_entry_mut(&entry_id)?;
            if entry.is_directory() {
                // unlink on directory not supported
                return Err(EISDIR);
            }
            parent_id = entry.parent_id;
        }

        let parent_inode_id = self.dcache.get_entry_mut(&parent_id)?.inode_id;
        let parent_inode = self
            .inodes
            .get(&parent_inode_id)
            .expect("No corresponding Inode for direntry");
        if !creds.is_access_granted(
            // check for write permission in the parent.
            parent_inode.access_mode,
            Amode::WRITE,
            (parent_inode.uid, parent_inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        Ok(self.funlink(entry_id)?)
    }

    pub fn funlink(&mut self, entry_id: DirectoryEntryId) -> SysResult<()> {
        let entry = self.dcache.get_entry(&entry_id)?;
        let parent_inode_number = self
            .dcache
            .get_entry(&entry.parent_id)
            .expect("No corresponding parent direntry for direntry")
            .inode_id
            .inode_number;

        let corresponding_inode = self
            .inodes
            .get_mut(&entry.inode_id)
            .expect("No corresponding Inode for direntry");
        let inode_id = corresponding_inode.id;

        // If the link number reach 0 and there is no open file
        // operation, we unlink on the filesystem directly, else we
        // will unlink when the last file operation is closed
        // eprintln!(
        //     "Calling funlink on inode: {}({:?})",
        //     entry.filename, entry.inode_id
        // );
        let free_inode_data: bool = corresponding_inode.unlink();
        let filename = entry.filename.clone(); //TODO: remove this
        self.dcache.remove_entry(entry_id)?;

        // we remove the inode only if we free the inode data
        if free_inode_data {
            self.inodes.remove(&inode_id).ok_or(ENOENT)?;
        } // else if corresponding_inode.lazy_unlink {
          //     eprintln!(
          //         "Lazy unlinking entry for {}, hardlinks: {}",
          //         filename, corresponding_inode.link_number
          //     );
          // } else {
          //     eprintln!(
          //         "There are still {} hardlinks for {}",
          //         corresponding_inode.link_number, filename
          //     );
          // }
        let fs = self.get_filesystem(inode_id).expect("no filesystem");
        fs.lock().unlink(
            parent_inode_number,
            filename.as_str(),
            free_inode_data,
            inode_id.inode_number,
        )?;
        Ok(())
    }

    pub fn close_file_operation(&mut self, inode_id: InodeId) {
        let corresponding_inode = self.inodes.get_mut(&inode_id).expect("no such inode");
        let inode_id = corresponding_inode.get_id();
        if corresponding_inode.close() {
            self.inodes.remove(&inode_id).expect("no such inode");
            if let Some(fs) = self.get_filesystem(inode_id) {
                fs.lock()
                    .remove_inode(inode_id.inode_number)
                    .expect("remove inode failed");
            }
        }
    }

    fn get_available_id(&self, filesystem_id: Option<FileSystemId>) -> InodeId {
        let mut current_id = InodeId::new(2, filesystem_id); // check this
        loop {
            if let None = self.inodes.get(&current_id) {
                return current_id;
            }

            // this is unchecked
            current_id = InodeId::new(current_id.inode_number + 1, filesystem_id);
        }
    }

    /// Gets the corresponding inode for a directory entry of id `direntry_id`.
    /// This methods helps removing the currently popular boilerplate.
    ///
    /// Panic:
    /// Panics if there is no corresponding inode for the given direntry_id.
    fn get_inode_from_direntry_id(&self, direntry_id: DirectoryEntryId) -> SysResult<&Inode> {
        let direntry = self.dcache.get_entry(&direntry_id)?;

        // should we remove this panic
        Ok(self
            .inodes
            .get(&direntry.inode_id)
            .expect("No corresponding Inode for Directory"))
    }

    // /// Gets the corresponding inode mutably for a directory entry of id `direntry_id`.
    // /// This methods helps removing the currently popular boilerplate.
    // ///
    // /// Panic:
    // /// Panics if there is no corresponding inode for the given direntry_id.
    // fn get_inode_from_direntry_id_mut(
    //     &mut self,
    //     direntry_id: DirectoryEntryId,
    // ) -> SysResult<&mut Inode> {
    //     let direntry = self.dcache.get_entry(&direntry_id)?;

    //     // should we remove this panic
    //     Ok(self
    //         .inodes
    //         .get_mut(&direntry.inode_id)
    //         .expect("No corresponding Inode for Directory"))
    // }

    /// Checks if the given `amode` is permitted for the file pointed by `path`
    pub fn is_access_granted(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: &Path,
        amode: Amode,
    ) -> bool {
        let direntry_id = match self.pathname_resolution(cwd, creds, path) {
            Err(_) => return false,
            Ok(id) => id,
        };

        let inode = self.get_inode_from_direntry_id(direntry_id).unwrap();

        // Ensure that non-regular files are not executables regardless of the file permissions.
        if !inode.access_mode.is_regular() && amode.contains(Amode::EXECUTE) {
            return false;
        }

        creds.is_access_granted(inode.access_mode, amode, (inode.uid, inode.gid))
    }

    /// La fonction open() du vfs sera appelee par la fonction open()
    /// de l'ipc
    /// Elle doit logiquement renvoyer un FileOperation ou une erreur
    /// C'est le driver attache a l'inode qui se gere de retourner le
    /// bon FileOperation
    /// Open du driver doit etre appele
    /// constrainte: Prototype, filename en param et Arc<DeadMutex<dyn FileOperation>> en retour
    /// Ce sont les 'Driver' qui auront l'ownership des 'FileOperation'
    pub fn open(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path, // Could be a ref.
        flags: OpenFlags,
        mode: FileType,
    ) -> SysResult<IpcResult<Arc<DeadMutex<dyn FileOperation>>>> {
        let entry_id;
        match self.pathname_resolution(cwd, creds, &path) {
            Ok(_id) if flags.contains(OpenFlags::O_CREAT | OpenFlags::O_EXCL) => {
                return Err(Errno::EEXIST)
            }
            Ok(id) => {
                let amode = Amode::from(flags);
                let inode = self.get_inode_from_direntry_id(id)?;

                if !creds.is_access_granted(inode.access_mode, amode, (inode.uid, inode.gid)) {
                    return Err(Errno::EACCES);
                }

                entry_id = id;
            }
            Err(e) if !flags.contains(OpenFlags::O_CREAT) => return Err(e.into()),
            _ => {
                let parent_id = self.pathname_resolution(cwd, creds, &path.parent()?)?;
                let parent_entry = self.dcache.get_entry(&parent_id)?;
                let parent_inode = self.get_inode_from_direntry_id(parent_id)?;

                let inode_id = parent_entry.inode_id;
                let inode_number = inode_id.inode_number as u32;

                if !creds.is_access_granted(
                    parent_inode.access_mode,
                    Amode::WRITE,
                    (parent_inode.uid, parent_inode.gid),
                ) {
                    return Err(Errno::EACCES);
                }

                let fs = self.get_filesystem_mut(inode_id).expect("no filesystem");
                let fs_cloned = fs.clone();

                let (direntry, inode_data, driver) = fs.lock().create(
                    path.filename().expect("no filename").as_str(),
                    inode_number,
                    // Open creates regular files
                    FileType::REGULAR_FILE | mode,
                    (creds.euid, creds.egid),
                )?;
                let fs_entry = (direntry, inode_data, Some(driver));
                entry_id = self.add_entry_from_filesystem(fs_cloned, Some(parent_id), fs_entry)?;
            }
        }

        let entry = self.dcache.get_entry(&entry_id)?;
        let entry_inode_id = entry.inode_id;
        if flags.contains(OpenFlags::O_DIRECTORY) && !entry.is_directory() {
            return Err(Errno::ENOTDIR);
        }

        self.inodes
            .get_mut(&entry_inode_id)
            .ok_or(ENOENT)?
            .open(flags)
    }

    // pub fn creat(
    //     &mut self,
    //     current: &mut Current,
    //     path: Path,
    //     mode: FileType,
    // ) -> SysResult<Fd> {
    //     let mut flags = OpenFlags::O_WRONLY | OpenFlags::O_CREAT | OpenFlags::O_TRUNC;

    //     if mode.contains(FileType::S_IFDIR) {
    //         flags |= OpenFlags::O_DIRECTORY
    //     }

    //     // This effectively does not release fd.
    //     Ok(self.open(current, path, flags, mode)?)
    // }

    // pub fn recursive_creat(
    //     &mut self,
    //     current: &mut Current,
    //     path: Path,
    //     mode: FileType,
    // ) -> SysResult<Fd> {
    //     let mut ancestors = path.ancestors();

    //     let child = ancestors.next_back().ok_or(EINVAL)?;
    //     let ancestors = ancestors; //uncomment this
    //     for ancestor in ancestors {
    //         self.creat(current, ancestor, FileType::S_IFDIR)
    //             .unwrap(); // forget fd?
    //     }

    //     Ok(self.creat(current, child, mode)?)
    // }

    pub fn chmod(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        mut mode: FileType,
    ) -> SysResult<()> {
        let mask = FileType::SPECIAL_BITS | FileType::PERMISSIONS_MASK;
        mode &= mask;

        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;

        let inode_id = entry.inode_id;
        self.fchmod(creds, inode_id, mode)
    }

    pub fn fchmod(
        &mut self,
        creds: &Credentials,
        inode_id: InodeId,
        mut mode: FileType,
    ) -> SysResult<()> {
        let mask = FileType::SPECIAL_BITS | FileType::PERMISSIONS_MASK;
        mode &= mask;

        let inode = self
            .inodes
            .get(&inode_id)
            .expect("Fchmod was called on unknown inode");

        if !creds.is_root() && creds.euid != inode.uid {
            return Err(Errno::EPERM);
        }

        self.get_filesystem(inode_id)
            .expect("No corresponding filesystem")
            .lock()
            .chmod(inode_id.inode_number as u32, mode)?;

        let inode = self
            .inodes
            .get_mut(&inode_id)
            .expect("No corresponding inode for direntry");
        let mut new_mode = inode.access_mode;

        new_mode.remove(mask);
        new_mode.insert(mode);

        inode.set_access_mode(new_mode);
        Ok(())
    }

    pub fn chown(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        owner: uid_t,
        group: gid_t,
    ) -> SysResult<()> {
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;

        let inode_id = entry.inode_id;
        self.fchown(creds, inode_id, owner, group)
    }

    pub fn fchown(
        &mut self,
        creds: &Credentials,
        inode_id: InodeId,
        owner: uid_t,
        group: gid_t,
    ) -> SysResult<()> {
        let inode = self
            .inodes
            .get(&inode_id)
            .expect("Fchown was called on unknown inode");

        if !creds.is_root() && creds.euid != inode.uid {
            return Err(Errno::EPERM);
        }

        let fs = self.get_filesystem(inode_id).expect("no filesystem");
        fs.lock()
            .chown(inode_id.inode_number as u32, owner, group)?;

        let inode = self
            .inodes
            .get_mut(&inode_id)
            .expect("No corresponding inode for direntry");

        if owner != uid_t::max_value() {
            inode.set_uid(owner);
        }

        if group != gid_t::max_value() {
            inode.set_gid(group);
        }
        Ok(())
    }

    pub fn utime(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        times: Option<&utimbuf>,
    ) -> SysResult<()> {
        // Handle permissions here too.
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let inode_id = self.dcache.get_entry(&entry_id)?.inode_id;
        let fs = self.get_filesystem(inode_id).expect("No filesystem");

        fs.lock().utime(inode_id.inode_number as u32, times)?;

        let inode = self.inodes.get_mut(&inode_id).ok_or(ENOENT)?;

        if let Some(times) = times {
            inode.atime = times.actime;
            inode.mtime = times.modtime;
        } else {
            let current_time = unsafe { CURRENT_UNIX_TIME.load(Ordering::Relaxed) };

            inode.atime = current_time as time_t;
            inode.mtime = current_time as time_t;
        }
        Ok(())
    }

    pub fn mkdir(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        mut path: Path,
        mode: FileType,
    ) -> SysResult<()> {
        if let Ok(_) = self.pathname_resolution(cwd, creds, &path) {
            return Err(EEXIST);
        }
        let filename = path.pop().ok_or(EINVAL)?;
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;
        if !entry.is_directory() {
            return Err(ENOTDIR);
        }

        let parent_inode = self.get_inode_from_direntry_id(entry_id)?;
        if !creds.is_access_granted(
            parent_inode.access_mode,
            Amode::WRITE,
            (parent_inode.uid, parent_inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        let inode_id = entry.inode_id;

        let fs = self.get_filesystem(inode_id).expect("no filesystem");
        let fs_cloned = fs.clone();
        let (direntry, inode_data, driver) = fs.lock().create_dir(
            inode_id.inode_number,
            filename.as_str(),
            mode,
            (creds.euid, creds.egid),
        )?;

        let fs_entry = (direntry, inode_data, Some(driver));
        self.add_entry_from_filesystem(fs_cloned, Some(entry_id), fs_entry)?;
        Ok(())
    }

    pub fn inode_id_from_absolute_path(
        &mut self,
        path: &Path,
        creds: &Credentials,
    ) -> SysResult<InodeId> {
        if !path.is_absolute() {
            panic!("path is not absolute");
        }
        let entry_id = self
            .pathname_resolution(&Path::root(), creds, path)
            .map_err(|_| ENOENT)?;
        let entry = self.dcache.get_entry(&entry_id)?;
        Ok(entry.inode_id)
    }

    pub fn mknod(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        mut path: Path,
        mode: FileType,
    ) -> SysResult<InodeId> {
        if !mode.is_fifo() && !mode.is_socket() {
            //TODO: remove that, and check create function filesystem
            unimplemented!()
        }

        if mode & FileType::S_IFMT == FileType::FIFO && !creds.is_root() {
            return Err(EPERM);
        }

        if let Ok(_) = self.pathname_resolution(cwd, creds, &path) {
            return Err(EEXIST);
        }
        let filename = path.pop().ok_or(EINVAL)?;
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;
        if !entry.is_directory() {
            return Err(ENOTDIR);
        }
        let inode_id = entry.inode_id;

        let inode = self
            .get_inode(inode_id)
            .expect("No corresponding Inode for Direntry");
        if !creds.is_access_granted(inode.access_mode, Amode::WRITE, (inode.uid, inode.gid)) {
            return Err(Errno::EACCES);
        }

        let fs = self.get_filesystem(inode_id).expect("no filesystem");
        let fs_cloned = fs.clone();

        let (direntry, inode_data, driver) = fs.lock().create(
            filename.as_str(),
            inode_id.inode_number,
            mode,
            (creds.euid, creds.egid),
        )?;
        let fs_entry = (direntry, inode_data, Some(driver));
        let new_entry_id = self.add_entry_from_filesystem(fs_cloned, Some(entry_id), fs_entry)?;
        let new_entry = self.dcache.get_entry(&new_entry_id)?;
        Ok(new_entry.inode_id)
    }

    /// create an inode which contains the driver and is not connected by a direntry
    pub fn add_orphan_driver(&mut self, driver: Box<dyn Driver>) -> SysResult<InodeId> {
        let mut inode_data: InodeData = Default::default();
        let new_inode_id = self.get_available_id(None);
        inode_data.id = new_inode_id;
        let new_inode = Inode::new(
            Arc::try_new(DeadMutex::new(DeadFileSystem))?,
            driver,
            inode_data,
        );
        self.add_inode(new_inode)?;
        Ok(new_inode_id)
    }

    pub fn remove_orphan_driver(&mut self, inode_id: InodeId) -> SysResult<Box<dyn Driver>> {
        let inode = self.inodes.remove(&inode_id).ok_or(ENOENT)?;
        Ok(inode.driver)
    }

    // TODO: Sticky bit (EPERM condition in posix) is not implemented for now.
    pub fn rmdir(&mut self, cwd: &Path, creds: &Credentials, path: Path) -> SysResult<()> {
        let filename = path.filename().ok_or(EINVAL)?;
        if filename == &"." || filename == &".." {
            return Err(EINVAL);
        }

        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;

        if !entry.is_directory() {
            return Err(ENOTDIR);
        }
        if !entry.is_directory_empty()? {
            return Err(ENOTEMPTY);
        }
        let inode_id = entry.inode_id;
        let parent_id = entry.parent_id;

        let parent_inode = self
            .get_inode_from_direntry_id(parent_id)
            .expect("No corresponding inode");
        let parent_inode_id = parent_inode.id;

        if !creds.is_access_granted(
            // check for write permission in the parent.
            parent_inode.access_mode,
            Amode::WRITE,
            (parent_inode.uid, parent_inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        self.dcache.remove_entry(entry_id)?;
        self.inodes.remove(&inode_id).expect("inode should be here");

        let fs = self.get_filesystem(inode_id).expect("no filesystem");
        fs.lock().rmdir(
            parent_inode_id.inode_number as u32,
            path.filename().expect("no filename").as_str(),
        )?;
        Ok(())
    }

    pub fn get_inode(&mut self, inode_id: InodeId) -> SysResult<&mut Inode> {
        self.inodes.get_mut(&inode_id).ok_or(ENOENT)
    }

    /// this implementation follow symbolic links
    pub fn link(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        oldpath: Path,
        newpath: Path,
    ) -> SysResult<()> {
        let oldentry_id = self.pathname_resolution(cwd, creds, &oldpath)?;
        let oldentry = self.dcache.get_entry(&oldentry_id)?;

        if oldentry.is_directory() {
            // link on directories not currently supported.
            return Err(EISDIR);
        }

        // works only on regular files
        if !oldentry.is_regular() {
            return Err(EINVAL);
        }

        if self.pathname_resolution(cwd, creds, &newpath).is_ok() {
            return Err(EEXIST);
        }

        let parent_new_id = self.pathname_resolution(cwd, creds, &newpath.parent()?)?;
        let parent_inode_id = self.dcache.get_entry_mut(&parent_new_id)?.inode_id;
        let parent_inode_number = parent_inode_id.inode_number;

        let parent_inode = self
            .get_inode(parent_inode_id)
            .expect("No corresponding inode");

        if !creds.is_access_granted(
            // check for write permission in the parent.
            parent_inode.access_mode,
            Amode::WRITE,
            (parent_inode.uid, parent_inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        let oldentry = self.dcache.get_entry(&oldentry_id)?;

        let inode_id = oldentry.inode_id;
        let target_inode_number = inode_id.inode_number;

        let inode = self.inodes.get_mut(&oldentry.inode_id).ok_or(ENOENT)?;

        let filename = newpath.filename().ok_or(EINVAL)?;
        // let mut newentry = oldentry.clone();
        // newentry.filename = *newpath.filename().unwrap(); // remove this unwrap somehow.
        inode.link_number += 1;

        let fs = self.get_filesystem(inode_id).expect("no filesystem");

        let newentry =
            fs.lock()
                .link(parent_inode_number, target_inode_number, filename.as_str())?;
        // self.add_entry_from_filesystem(fs_cloned, Some(parent_new_id), fs_entry)?;
        self.dcache.add_entry(Some(parent_new_id), newentry)?;
        Ok(())
    }

    pub fn stat(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
    ) -> SysResult<stat> {
        let entry_id = self.pathname_resolution(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;
        let inode_id = entry.inode_id;
        let inode = self.get_inode(inode_id)?;
        inode.stat()
    }

    pub fn lstat(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
    ) -> SysResult<stat> {
        let entry_id = self.pathname_resolution_no_follow_last_symlink(cwd, creds, &path)?;
        let entry = self.dcache.get_entry(&entry_id)?;
        let inode_id = entry.inode_id;
        let inode = self.get_inode(inode_id)?;
        inode.stat()
    }

    pub fn readlink(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        buf: &mut [c_char],
    ) -> SysResult<u32> {
        let entry_id = self.pathname_resolution_no_follow_last_symlink(cwd, creds, &path)?;
        let symbolic_content = self
            .dcache
            .get_entry(&entry_id)?
            .get_symbolic_content()
            .ok_or(EINVAL)?;

        symbolic_content.write_path_in_buffer(buf)
    }

    pub fn symlink(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        target: &str,
        mut linkname: Path,
    ) -> SysResult<()> {
        if let Ok(_) = self.pathname_resolution(cwd, creds, &linkname) {
            return Err(EEXIST);
        }
        let filename = linkname.pop().expect("no filename");
        let direntry_id = self.pathname_resolution(cwd, creds, &linkname)?;
        let direntry = self.dcache.get_entry(&direntry_id)?;
        if !direntry.is_directory() {
            return Err(ENOENT);
        }

        let parent_inode_id = direntry.inode_id;

        // let's remove this code duplication with `get_inode_from_direntry_id` or something.
        let parent_inode = self
            .inodes
            .get(&parent_inode_id)
            .expect("No corresponding Inode for direntry");
        if !creds.is_access_granted(
            // check for write permission in the parent.
            parent_inode.access_mode,
            Amode::WRITE,
            (parent_inode.uid, parent_inode.gid),
        ) {
            return Err(Errno::EACCES);
        }

        let fs_cloned = self
            .get_filesystem(parent_inode_id)
            .expect("no filesystem")
            .clone();
        let fs = self.get_filesystem(parent_inode_id).expect("no filesystem");
        let (direntry, inode_data, driver) = fs.lock().symlink(
            parent_inode_id.inode_number as u32,
            target,
            filename.as_str(),
        )?;

        let fs_entry = (direntry, inode_data, Some(driver));
        self.add_entry_from_filesystem(fs_cloned.clone(), Some(direntry_id), fs_entry)
            .expect("add entry from filesystem failed");
        Ok(())
    }

    pub fn resolve_path(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: &Path,
    ) -> SysResult<Path> {
        let direntry_id = self.pathname_resolution(cwd, creds, &path)?;
        self.dentry_path(direntry_id)
    }

    //TODO: permissions here not currently implemented.
    pub fn rename(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        oldpath: Path,
        newpath: Path,
    ) -> SysResult<()> {
        // If either pathname argument refers to a path whose final
        // component is either dot or dot-dot, rename() shall fail.
        let old_filename = oldpath.filename().ok_or(EINVAL)?;

        if old_filename == &"." || old_filename == &".." {
            return Err(Errno::EINVAL);
        }
        let new_filename = newpath.filename().ok_or(EINVAL)?;

        if new_filename == &"." || new_filename == &".." {
            return Err(Errno::EINVAL);
        }

        let oldentry_id = self.pathname_resolution_no_follow_last_symlink(cwd, creds, &oldpath)?;
        // The old pathname shall not name an ancestor directory of
        // the new pathname.
        let resolved_old_path = self.resolve_path(cwd, creds, &oldpath)?;
        let mut resolved_new_path = self.resolve_path(cwd, creds, &newpath.parent()?)?;
        resolved_new_path.push(*new_filename)?;
        if resolved_new_path
            .ancestors()
            .any(|p| p == resolved_old_path)
        {
            return Err(Errno::EINVAL);
        }

        match self.pathname_resolution_no_follow_last_symlink(cwd, creds, &newpath) {
            Ok(new_entry_id) => {
                let new_entry = self.dcache.get_entry(&new_entry_id)?;

                let oldentry = self.dcache.get_entry(&oldentry_id)?;
                if oldentry.is_directory()
                    && (!new_entry.is_directory() || !new_entry.is_directory_empty()?)
                {
                    // If the old argument points to the pathname of a
                    // directory, the new argument shall not point to the
                    // pathname of a file that is not a directory, it
                    // shall be required to be an empty directory.
                    return Err(Errno::EEXIST);
                } else if !oldentry.is_directory() && new_entry.is_directory() {
                    // If the old argument points to the pathname of a
                    // file that is not a directory, the new argument
                    // shall not point to the pathname of a directory
                    return Err(Errno::EISDIR);
                }
                // If the old argument and the new argument resolve to
                // either the same existing directory entry or
                // different directory entries for the same existing
                // file, rename() shall return successfully and
                // perform no other action.
                if new_entry.inode_id.inode_number == oldentry.inode_id.inode_number {
                    return Ok(());
                }

                if new_entry.is_directory() {
                    self.rmdir(cwd, creds, newpath.try_clone()?)?;
                } else {
                    self.unlink(cwd, creds, newpath.try_clone()?)?;
                }
            }
            Err(_) => {}
        }
        // newpath does not exist in either case

        let oldentry = self.dcache.get_entry(&oldentry_id)?;
        let old_parent_id = oldentry.parent_id;
        let old_parent_inode_id = self.dcache.get_entry_mut(&old_parent_id)?.inode_id;
        let old_parent_inode_nbr = old_parent_inode_id.inode_number;

        let new_parent_id = self.pathname_resolution(cwd, creds, &newpath.parent()?)?;
        let new_parent_inode_id = self.dcache.get_entry_mut(&new_parent_id)?.inode_id;
        let new_parent_inode_nbr = new_parent_inode_id.inode_number;

        let fs = self
            .get_filesystem(old_parent_inode_id)
            .expect("no filesystem");

        fs.lock().rename(
            old_parent_inode_nbr,
            old_filename.as_str(),
            new_parent_inode_nbr,
            new_filename.as_str(),
        )?;

        let oldentry_id = self.dcache.move_dentry(oldentry_id, new_parent_id)?;

        let entry = self
            .dcache
            .d_entries
            .get_mut(&oldentry_id)
            .expect("oldentry sould be there");

        entry.set_filename(*new_filename);
        Ok(())
    }

    pub fn statfs(
        &mut self,
        cwd: &Path,
        creds: &Credentials,
        path: Path,
        buf: &mut statfs,
    ) -> SysResult<()> {
        let direntry_id = self
            .pathname_resolution(cwd, creds, &path)
            .or(Err(Errno::ENOENT))?;
        let inode_id = {
            self.dcache
                .get_entry(&direntry_id)
                .expect("No corresponding inode for direntry")
                .inode_id
        };

        self.fstatfs(inode_id, buf)
    }

    pub fn fstatfs(&self, inode_id: InodeId, buf: &mut statfs) -> SysResult<()> {
        inode_id.filesystem_id.ok_or(Errno::ENOSYS)?; // really not sure about that.
        let fs = self
            .get_filesystem(inode_id)
            .expect("No filesystem match the filesystem_id from an InodeId");

        fs.lock().statfs(buf)
    }
}

// pub type VfsHandler<T> = fn(VfsHandlerParams) -> SysResult<T>;

// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub enum VfsHandlerKind {
//     Open,
//     LookupInode,
//     LookupEntries,
//     Creat,
//     Rename,
//     Chmod,
//     Chown,
//     Lchown,
//     Truncate,
//     TestOpen,
// }
// // #[derive(Debug, Clone, Default)]
// #[derive(Default)]
// pub struct VfsHandlerParams<'a> {
//     inode: Option<&'a Inode>,
//     file: Option<&'a File>,
//     path: Option<&'a Path>,
// }

// impl<'a> VfsHandlerParams<'a> {
//     pub fn new() -> Self {
//         Self::default()
//     }

//     pub fn set_inode(mut self, inode: &'a Inode) -> Self {
//         self.inode = Some(inode);
//         self
//     }

//     pub fn set_file(mut self, file: &'a File) -> Self {
//         self.file = Some(file);
//         self
//     }

//     pub fn set_path(mut self, path: &'a Path) -> Self {
//         self.path = Some(path);
//         self
//     }

//     pub fn unset_inode(mut self) -> Self {
//         self.inode = None;
//         self
//     }

//     pub fn unset_file(mut self) -> Self {
//         self.file = None;
//         self
//     }

//     pub fn unset_path(mut self) -> Self {
//         self.path = None;
//         self
//     }
// }

impl KeyGenerator<FileSystemId> for VirtualFileSystem {
    fn gen_filter(&self, id: FileSystemId) -> bool {
        !self.mounted_filesystems.contains_key(&id)
    }
}

// impl Mapper<FileSystemId, Arc<DeadMutex<dyn FileSystem>>> for VirtualFileSystem {
//     fn get_map(&mut self) -> &mut BTreeMap<FileSystemId, Arc<DeadMutex<dyn FileSystem>>> {
//         &mut self.mounted_filesystems
//     }
// }

#[cfg(test)]
mod vfs {

    use super::*;
    // rename this
    macro_rules! make_test {
        ($body: expr, $name: ident) => {
            #[test]
            fn $name() {
                $body
            }
        };
        (failing, $body: expr, $name: ident) => {
            #[test]
            #[should_panic]
            fn $name() {
                $body
            }
        };
    }

    macro_rules! vfs_test {
        ($body: block, $name: ident) => {
            make_test! {$body, $name}
        };
        (failing, $body: block, $name: ident) => {
            make_test! {failing, $body, $name}
        };
    }

    // macro_rules! vfs_file_exists_test {
    //     ($body: block, $path: expr, $name: ident) => {
    //         make_test! {{
    //             let mut vfs = Vfs::new().unwrap();
    //             let mut current = default_current();
    //             let path: &str = $path;
    //             let path: Path = std::convert::TryInto::try_into(path).unwrap();

    //             if path != std::convert::TryInto::try_into("/").unwrap() {
    //                 vfs.recursive_creat(&mut current, path.clone(), FileType::S_IRWXU).unwrap();
    //             }
    //             assert!(vfs.file_exists(&current, path).unwrap())
    //         }, $name}
    //     };
    //     (failing, $body: block, $path: expr, $name: ident) => {
    //         make_test! {failing, {
    //             let mut vfs = Vfs::new().unwrap();
    //             let mut current = default_current();
    //             let path: &str = $path;
    //             let path: Path = std::convert::TryInto::try_into(path).unwrap();

    //             if path != std::convert::TryInto::try_into("/").unwrap() {
    //                 vfs.recursive_creat(&mut current, path.clone(), FileType::S_IRWXU).unwrap();
    //             }
    //             assert!(vfs.file_exists(&current, path).unwrap())
    //         }, $name}
    //     };
    // }

    // vfs_file_exists_test! {{}, "/", file_exists_root}
    // vfs_file_exists_test! {failing, {}, "", file_exists_null}
    // vfs_file_exists_test! {{
    // }, "a", file_exists_basic_a}
    // vfs_file_exists_test! {{
    // }, "/a", file_exists_basic_root_a}

    // vfs_file_exists_test! {{
    // }, "a/b", file_exists_basic_a_b}
    // vfs_file_exists_test! {{
    // }, "a/b/c", file_exists_basic_a_b_c}
    // vfs_file_exists_test! {{
    // }, "a/b/c/d", file_exists_basic_a_b_c_d}
    // vfs_file_exists_test! {{
    // }, "a/b/c/d/e/f", file_exists_basic_a_b_c_d_e_f}

    // vfs_file_exists_test! {{
    // }, "/a/b", file_exists_basic_root_a_b}
    // vfs_file_exists_test! {{
    // }, "/a/b/c", file_exists_basic_root_a_b_c}
    // vfs_file_exists_test! {{
    // }, "/a/b/c/d", file_exists_basic_root_a_b_c_d}
    // vfs_file_exists_test! {{
    // }, "/a/b/c/d/e/f", file_exists_basic_root_a_b_c_d_e_f}

    // macro_rules! vfs_recursive_creat_test {
    //     ($path: expr, $name: ident) => {
    //         make_test! {{
    //             let mut vfs = Vfs::new().unwrap();
    //             let mut current = default_current();
    //             let path: &str = $path;
    //             let path: Path = std::convert::TryInto::try_into(path).unwrap();

    //             vfs.recursive_creat(&mut current
    //                                 , path.clone()
    //                                 , FileType::S_IRWXU).unwrap();
    //             assert!(vfs.file_exists(&current, path).unwrap())
    //         }, $name}
    //     };
    //     (failing, $path: expr, $name: ident) => {
    //         make_test! {failing, {
    //             let mut vfs = Vfs::new().unwrap();
    //             let mut current = default_current();
    //             let path: &str = $path;
    //             let path: Path = path.try_into().unwrap();

    //             vfs.recursive_creat(&mut current
    //                                 , path.clone()
    //                                 , FileType::S_IRWXU).unwrap();
    //             for ancestors in path.ancestors() {
    //                 assert!(vfs.file_exists(&current, ancestor).unwrap())
    //             }
    //         }, $name}
    //     };
    // }

    // vfs_recursive_creat_test! {"a/b/c/d/e/f/g", recursive_creat_a_b_c_d_e_f_g}
    // vfs_recursive_creat_test! {"a/b/c/d/e/f  ", recursive_creat_a_b_c_d_e_f}
    // vfs_recursive_creat_test! {"a/b/c/d/e    ", recursive_creat_a_b_c_d_e}
    // vfs_recursive_creat_test! {"a/b/c/d      ", recursive_creat_a_b_c_d}
    // vfs_recursive_creat_test! {"a/b/c        ", recursive_creat_a_b_c}
    // vfs_recursive_creat_test! {"a/b          ", recursive_creat_a_b} // infinite loop
    // vfs_recursive_creat_test! {"a            ", recursive_creat_a}
}
