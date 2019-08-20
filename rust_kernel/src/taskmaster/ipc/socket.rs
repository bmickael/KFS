//! This file contains all the stuff about Socket

use super::SysResult;

use super::IpcResult;
use super::KernelFileDescriptor;
use super::Mode;

/// This structure represents a KernelFileDescriptor of type Socket
#[derive(Debug, Default)]
pub struct Socket {}

/// Main implementation for Socket
impl Socket {
    pub fn new() -> Self {
        Self {}
    }
}

/// Main Trait implementation
impl KernelFileDescriptor for Socket {
    fn register(&mut self, _access_mode: Mode) {}
    fn unregister(&mut self, _access_mode: Mode) {}
    fn read(&mut self, _buf: &mut [u8]) -> SysResult<IpcResult<u32>> {
        Ok(IpcResult::Continue(0))
    }
    fn write(&mut self, _buf: &[u8]) -> SysResult<IpcResult<u32>> {
        Ok(IpcResult::Continue(0))
    }
}

/// Some boilerplate to check if all is okay
impl Drop for Socket {
    fn drop(&mut self) {
        println!("Socket droped !");
    }
}
