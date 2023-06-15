use super::{Driver, FileOperation, InodeId, IpcResult, ProcFsOperations, SysResult, VFS};
use crate::taskmaster::SCHEDULER;

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use fallible_collections::TryCollect;

use libc_binding::OpenFlags;
use sync::DeadMutex;

type Mutex<T> = DeadMutex<T>;

use libc_binding::{off_t, Whence};
use libc_binding::{Errno, Pid};

#[derive(Debug, Clone)]
pub struct CommDriver {
    inode_id: InodeId,
    pid: Pid,
}

unsafe impl Send for CommDriver {}

#[derive(Debug)]
pub struct CommOperations {
    inode_id: InodeId,
    pid: Pid,
    // the cwd of the process at the moment this CommOperations was created.
    offset: usize,
}

impl Driver for CommDriver {
    fn open(&mut self, _flags: OpenFlags) -> SysResult<IpcResult<Arc<Mutex<dyn FileOperation>>>> {
        let res = Arc::try_new(Mutex::new(CommOperations::new(self.inode_id, self.pid, 0)))?;
        Ok(IpcResult::Done(res))
    }
}

impl CommDriver {
    pub fn new(inode_id: InodeId, pid: Pid) -> Self {
        Self { inode_id, pid }
    }
}

impl CommOperations {
    pub fn new(inode_id: InodeId, pid: Pid, offset: usize) -> Self {
        Self {
            inode_id,
            pid,
            offset,
        }
    }
}

impl ProcFsOperations for CommOperations {
    fn get_offset(&mut self) -> &mut usize {
        &mut self.offset
    }

    fn get_seq_string(&self) -> SysResult<Cow<str>> {
        SCHEDULER.force_unlock();
        let scheduler = SCHEDULER.lock();

        let comm = {
            match scheduler
                .get_thread_group(self.pid)
                .expect("CommOperations::read(): The Process should exist")
                .argv
                .as_ref()
            {
                Some(comm) => comm,
                None => return Ok(Cow::from("")),
            }
        };

        let bytes: Vec<u8> = comm
            .strings()
            .next()
            .iter()
            .flat_map(|s| s.iter().map(|b| *b as u8))
            .filter(|c| *c != '\0' as u8)
            .skip(self.offset)
            .chain("\n".bytes())
            .try_collect()?;

        Ok(Cow::from(String::from_utf8(bytes).map_err(|_| {
            log::error!("invalid utf8 in environ operation");
            Errno::EINVAL
        })?))
    }
}

impl FileOperation for CommOperations {
    fn get_inode_id(&self) -> SysResult<InodeId> {
        Ok(self.inode_id)
    }

    fn read(&mut self, buf: &mut [u8]) -> SysResult<IpcResult<u32>> {
        self.seq_read(buf)
    }

    fn lseek(&mut self, offset: off_t, whence: Whence) -> SysResult<off_t> {
        self.proc_lseek(offset, whence)
    }
}

impl Drop for CommOperations {
    fn drop(&mut self) {
        VFS.lock().close_file_operation(self.inode_id);
    }
}
