//! This file contains definition of a task

use super::process::{CpuState, UserProcess};
use super::scheduler::Pid;
use super::signal_interface::SignalInterface;
use super::syscall::clone::CloneFlags;
use super::syscall::WaitOption;
use super::thread_group::Status;
use super::SysResult;

use core::ffi::c_void;
use fallible_collections::FallibleBox;

use alloc::boxed::Box;
use alloc::collections::TryReserveError;

#[derive(Debug, Copy, Clone)]
pub enum AutoPreemptReturnValue {
    None,
    Wait {
        dead_process_pid: Pid,
        status: Status,
    },
}

impl Default for AutoPreemptReturnValue {
    fn default() -> Self {
        Self::None
    }
}

/// Main Task definition
#[derive(Debug)]
pub struct Thread {
    /// Current process state
    pub process_state: ProcessState,
    /// Signal Interface
    pub signal: SignalInterface,
    /// Return value for auto_preempt
    autopreempt_return_value: Box<SysResult<AutoPreemptReturnValue>>,
}

impl Thread {
    pub fn new(process_state: ProcessState) -> Result<Self, TryReserveError> {
        Ok(Self {
            process_state,
            signal: SignalInterface::new(),
            autopreempt_return_value: Box::try_new(Ok(Default::default()))?,
        })
    }

    pub fn get_waiting_state(&self) -> Option<&WaitingState> {
        match &self.process_state {
            ProcessState::Waiting(_, waiting_state) => Some(waiting_state),
            _ => None,
        }
    }

    pub fn sys_clone(
        &self,
        kernel_esp: u32,
        child_stack: *const c_void,
        flags: CloneFlags,
    ) -> SysResult<Self> {
        Ok(Self {
            signal: self.signal.fork(),
            process_state: match &self.process_state {
                ProcessState::Running(Some(p)) => {
                    ProcessState::Running(Some(p.sys_clone(kernel_esp, child_stack, flags)?))
                }
                _ => panic!("Non running process should not clone"),
            },
            autopreempt_return_value: Box::try_new(Ok(Default::default()))?,
        })
    }

    pub fn unwrap_process_mut(&mut self) -> &mut UserProcess {
        match &mut self.process_state {
            ProcessState::Waiting(Some(process), _) | ProcessState::Running(Some(process)) => process,
            _ => panic!("unwrap_process_mut failed!")
        }
    }

    pub fn unwrap_process(&self) -> &UserProcess {
        match &self.process_state {
            ProcessState::Running(Some(process)) | ProcessState::Waiting(Some(process), _) => process,
            _ => panic!("unwrap_process failed!")
        }
    }

    pub fn set_return_value_autopreempt(
        &mut self,
        return_value: SysResult<AutoPreemptReturnValue>,
    ) {
        let cpu_state = self.unwrap_process().kernel_esp as *mut CpuState;
        *self.autopreempt_return_value = return_value;
        unsafe {
            (*(cpu_state)).registers.eax = self.autopreempt_return_value.as_ref()
                as *const SysResult<AutoPreemptReturnValue>
                as u32;
        }
    }

    // /// For blocking call, set the return value witch will be transmitted by auto_preempt fn
    // pub fn set_return_value(&self, return_value: i32) {
    //     let cpu_state = self.unwrap_process().kernel_esp as *mut CpuState;
    //     unsafe {
    //         (*(cpu_state)).registers.eax = return_value as u32;
    //     }
    // }

    #[allow(dead_code)]
    pub fn is_waiting(&self) -> bool {
        match self.process_state {
            ProcessState::Waiting(_, _) => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        match self.process_state {
            ProcessState::Running(_) => true,
            _ => false,
        }
    }

    pub fn set_waiting(&mut self, waiting_state: WaitingState) {
        self.process_state.set_waiting(waiting_state);
    }

    pub fn set_running(&mut self) {
        self.process_state.set_running();
    }
}

#[derive(Debug, PartialEq)]
pub enum WaitingState {
    /// The Process is sleeping until pit time >= u32 value
    Sleeping(u32),
    /// The sys_pause command was invoqued, the process is waiting for a signal
    Pause,
    /// The Process is looking for the death of his child
    Waitpid {
        pid: Pid,
        pgid: Pid,
        options: WaitOption,
    },
    /// In Waiting to read
    Read(usize),
    /// In Waiting to write
    Write(usize),
    /// In Waiting to open
    Open(usize),
    /// In waiting for a socket accepting connection
    Connect(usize),
    /// In waiting for a socket connection
    Accept(usize),
}

#[derive(Debug)]
pub enum ProcessState {
    /// The process is currently on running state
    Running(Option<Box<UserProcess>>),
    /// The process is currently waiting for something
    Waiting(Option<Box<UserProcess>>, WaitingState),
}

impl ProcessState {
    pub fn set_waiting(&mut self, waiting_state: WaitingState) {
        let p = match self {
            ProcessState::Running(p) | ProcessState::Waiting(p, _) => p.take(),
        };
        p.as_ref().expect("Option<Box<T>> cannot be None.");
        *self = ProcessState::Waiting(p, waiting_state);
    }
    pub fn set_running(&mut self) {
        let p = match self {
            ProcessState::Waiting(p, _) => p.take(),  //ProcessState::Running(p),
            _ => panic!("already running"),
        };
        p.as_ref().expect("Option<Box<T>> cannot be None.");
        *self = ProcessState::Running(p);
    }
}
