//! This file contains all the stuff about Pipes

use super::SysResult;

use super::FileOperation;
use super::IpcResult;
use super::Mode;

/// This structure represents a FileOperation of type Pipe
#[derive(Debug, Default)]
pub struct Pipe {
    input_ref: usize,
    output_ref: usize,
}

/// Main implementation for Pipe
impl Pipe {
    pub fn new() -> Self {
        Self {
            input_ref: Default::default(),
            output_ref: Default::default(),
        }
    }
}

/// Main Trait implementation
impl FileOperation for Pipe {
    fn register(&mut self, access_mode: Mode) {
        match access_mode {
            Mode::ReadOnly => self.input_ref += 1,
            Mode::WriteOnly => self.output_ref += 1,
            _ => panic!("Pipe invalid access mode"),
        };
    }
    fn unregister(&mut self, access_mode: Mode) {
        match access_mode {
            Mode::ReadOnly => self.input_ref -= 1,
            Mode::WriteOnly => self.output_ref -= 1,
            _ => panic!("Pipe invalid access mode"),
        };
    }
    fn read(&mut self, _buf: &mut [u8]) -> SysResult<IpcResult<u32>> {
        unimplemented!();
    }
    fn write(&mut self, _buf: &[u8]) -> SysResult<IpcResult<u32>> {
        unimplemented!();
    }
}

/// Some boilerplate to check if all is okay
impl Drop for Pipe {
    fn drop(&mut self) {
        println!("Pipe droped !");
    }
}
