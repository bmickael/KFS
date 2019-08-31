use super::drivers::FileOperation;
use super::IpcResult;
/// The User File Descriptor are sorted into a Binary Tree
/// Key is the user number and value the structure FileDescriptor
use super::SysResult;
use super::VFS;

use libc_binding::Errno;

use super::drivers::ipc::{Fifo, Pipe, Socket};
use alloc::sync::Arc;

use fallible_collections::btree::BTreeMap;
use fallible_collections::FallibleArc;
use fallible_collections::TryClone;

use try_clone_derive::TryClone;

use sync::{DeadMutex, DeadMutexGuard};

pub type Fd = u32;

#[derive(Debug, TryClone)]
pub struct FileDescriptorInterface {
    user_fd_list: BTreeMap<Fd, FileDescriptor>,
}

/// Main implementation
impl FileDescriptorInterface {
    const MAX_FD: Fd = 128;

    /// Global constructor
    pub fn new() -> Self {
        Self {
            // New BTreeMap does not allocate memory
            user_fd_list: BTreeMap::new(),
        }
    }

    /// Clear all the owned content into the File Descriptor Interface
    pub fn delete(&mut self) {
        self.user_fd_list.clear();
    }

    pub fn get_file_operation(&self, fd: Fd) -> SysResult<DeadMutexGuard<dyn FileOperation>> {
        let elem = self.user_fd_list.get(&fd).ok_or::<Errno>(Errno::EBADF)?;
        Ok(elem.file_operation.lock())
    }

    // TODO: fix dummy access_mode && manage flags
    /// Open a file and give a file descriptor
    pub fn open(
        &mut self,
        filename: &str, /* access_mode: Mode ? */
    ) -> SysResult<IpcResult<Fd>> {
        // TODO: REMOVE THIS SHIT
        let mut current = super::vfs::Current {
            cwd: super::vfs::DirectoryEntryId::new(2),
            uid: 0,
            euid: 0,
            gid: 0,
            egid: 0,
            open_fds: alloc::collections::BTreeMap::new(),
        };
        // TODO: REMOVE THIS SHIT
        let mode =
            super::vfs::FilePermissions::from_bits(0o777).expect("file permission creation failed");
        use core::convert::TryFrom;
        // TODO: REMOVE THIS SHIT
        let path = super::vfs::Path::try_from(filename)?;
        // TODO: REMOVE THIS SHIT
        let flags = libc_binding::OpenFlags::O_RDWR;

        let file_operator =
            VFS.lock()
                .open(&mut current, path, flags, mode /* access_mode */)?;
        match file_operator {
            IpcResult::Done(file_operator) => {
                let fd = self.insert_user_fd(Mode::ReadWrite, file_operator)?;
                Ok(IpcResult::Done(fd))
            }
            IpcResult::Wait(file_operator, file_op_uid) => {
                let fd = self.insert_user_fd(Mode::ReadWrite, file_operator)?;
                Ok(IpcResult::Wait(fd, file_op_uid))
            }
        }
    }

    /// Clone one file descriptor
    pub fn close_fd(&mut self, fd: Fd) -> SysResult<()> {
        self.user_fd_list.remove(&fd).ok_or::<Errno>(Errno::EBADF)?;
        Ok(())
    }

    /// Read something from the File Descriptor: Can block
    /// Important ! When in blocked syscall, the slice must be verified before read op and
    /// we have fo find a solution to avoid the DeadLock when multiple access to fd occured
    pub fn read(&mut self, fd: Fd, buf: &mut [u8]) -> SysResult<IpcResult<u32>> {
        let elem = self.user_fd_list.get(&fd).ok_or::<Errno>(Errno::EBADF)?;

        elem.file_operation.lock().read(buf)
    }

    /// Write something into the File Descriptor: Can block
    /// Important ! When in blocked syscall, the slice must be verified before write op and
    /// we have fo find a solution to avoid the DeadLock when multiple access to fd occured
    pub fn write(&mut self, fd: Fd, buf: &[u8]) -> SysResult<IpcResult<u32>> {
        let elem = self.user_fd_list.get(&fd).ok_or::<Errno>(Errno::EBADF)?;

        elem.file_operation.lock().write(buf)
    }

    /// Made two File Descriptors connected with a Pipe
    pub fn new_pipe(&mut self) -> SysResult<(Fd, Fd)> {
        let pipe = Arc::try_new(DeadMutex::new(Pipe::new()))?;
        let cloned_pipe = pipe.clone();

        let input_fd = self.insert_user_fd(Mode::ReadOnly, pipe)?;
        let output_fd = self
            .insert_user_fd(Mode::WriteOnly, cloned_pipe)
            .map_err(|e| {
                let _r = self.user_fd_list.remove(&input_fd);
                e
            })?;

        Ok((input_fd, output_fd))
    }

    /// Duplicate one File Descriptor
    pub fn dup(&mut self, oldfd: Fd, minimum: Option<Fd>) -> SysResult<Fd> {
        if let Some(elem) = self.user_fd_list.get(&oldfd) {
            let new_elem = elem.try_clone()?;
            let newfd = self
                .get_lower_fd_value(minimum.unwrap_or(0))
                .ok_or::<Errno>(Errno::EMFILE)?;

            self.user_fd_list.try_insert(newfd, new_elem)?;
            return Ok(newfd);
        }
        Err(Errno::EBADF)
    }

    /// Duplicate one file descriptor with possible override
    pub fn dup2(&mut self, oldfd: Fd, newfd: Fd) -> SysResult<Fd> {
        if newfd > Self::MAX_FD {
            return Err(Errno::EBADF);
        }

        // If oldfd is not a valid file descriptor, then the call fails, and newfd is not closed.
        if let Some(elem) = self.user_fd_list.get(&oldfd) {
            let new_elem = elem.try_clone()?;
            let _r = self.close_fd(newfd);

            self.user_fd_list.try_insert(newfd, new_elem)?;
            return Ok(newfd);
        }
        Err(Errno::EBADF)
    }

    /// Insert a new User File Descriptor atached to a Kernel File Descriptor:
    /// return value: User File Descriptor index
    fn insert_user_fd(
        &mut self,
        mode: Mode,
        file_operation: Arc<DeadMutex<dyn FileOperation>>,
    ) -> SysResult<Fd> {
        let user_fd = self.get_lower_fd_value(0).ok_or::<Errno>(Errno::EMFILE)?;
        self.user_fd_list
            .try_insert(user_fd, FileDescriptor::new(mode, file_operation))?;
        Ok(user_fd)
    }

    /// Get the first available File Descriptor number that is superior to `minimum`
    fn get_lower_fd_value(&self, minimum: Fd) -> Option<Fd> {
        let mut lower_fd = minimum;

        for &key in self.user_fd_list.keys().skip_while(|&key| *key < minimum) {
            if lower_fd < key {
                break;
            } else {
                lower_fd += 1;
            }
        }
        if lower_fd > Self::MAX_FD {
            None
        } else {
            Some(lower_fd)
        }
    }

    // TODO: This function may be trashed in the furure
    /// Open a Fifo. Block until the fifo is not open in two directions.
    #[allow(dead_code)]
    pub fn open_fifo(&mut self, access_mode: Mode) -> SysResult<IpcResult<Fd>> {
        if access_mode == Mode::ReadWrite {
            return Err(Errno::EACCES);
        }

        let fifo = Arc::try_new(DeadMutex::new(Fifo::new()))?;
        let fd = self.insert_user_fd(access_mode, fifo)?;

        Ok(IpcResult::Done(fd))
    }

    // TODO: This function may be trashed in the future
    /// Open a Socket
    /// The socket type must be pass as parameter
    #[allow(dead_code)]
    pub fn open_socket(&mut self, access_mode: Mode) -> SysResult<Fd> {
        let socket = Arc::try_new(DeadMutex::new(Socket::new()))?;
        let fd = self.insert_user_fd(access_mode, socket)?;

        Ok(fd)
    }
}

/// Some boilerplate to check if all is okay
impl Drop for FileDescriptorInterface {
    fn drop(&mut self) {
        //         println!("FD interface droped");
    }
}

/// This structure design a User File Descriptor
/// We can normally clone the Arc
#[derive(Debug)]
struct FileDescriptor {
    access_mode: Mode,
    file_operation: Arc<DeadMutex<dyn FileOperation>>,
}

use alloc::collections::CollectionAllocErr;

/// TryClone Boilerplate. The ref counter of the FileOperation must be incremented when Cloning
impl TryClone for FileDescriptor {
    fn try_clone(&self) -> Result<Self, CollectionAllocErr> {
        self.file_operation.lock().register(self.access_mode);
        Ok(Self {
            access_mode: self.access_mode.clone(),
            file_operation: self.file_operation.clone(),
        })
    }
}

/// Standard implementation of an user File Descriptor
impl FileDescriptor {
    /// When a new FileDescriptor is invoqued, Increment reference
    fn new(access_mode: Mode, file_operation: Arc<DeadMutex<dyn FileOperation>>) -> Self {
        file_operation.lock().register(access_mode);
        Self {
            access_mode,
            file_operation,
        }
    }
}

/// Drop boilerplate for an FileDescriptor structure. Decremente reference
impl Drop for FileDescriptor {
    fn drop(&mut self) {
        self.file_operation.lock().unregister(self.access_mode);
    }
}

/// The Access Mode of the File Descriptor
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryClone)]
pub enum Mode {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}