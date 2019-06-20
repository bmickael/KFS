//! all kernel syscall start by sys_ and userspace syscall (which will be in libc anyway) start by user_

use super::SysResult;

use super::ipc;
use super::process;
use super::process::CpuState;
use super::scheduler;
use super::scheduler::unpreemptible;
use super::scheduler::{Pid, SCHEDULER, SIGNAL_LOCK};
use super::signal;
use super::signal::StructSigaction;
use super::task;

mod mmap;
use mmap::{sys_mmap, sys_mprotect, sys_munmap, MmapArgStruct, MmapProt};

mod nanosleep;
use nanosleep::{sys_nanosleep, TimeSpec};

mod waitpid;
use waitpid::sys_waitpid;

pub mod signalfn;
use signalfn::{sys_kill, sys_pause, sys_sigaction, sys_signal, sys_sigreturn};

mod close;
use close::sys_close;

mod unlink;
use unlink::sys_unlink;

mod socket;
use socket::{sys_socketcall, SocketArgsPtr};

pub mod read;
use read::sys_read;

mod power;
use power::{sys_reboot, sys_shutdown};

mod execve;
use execve::sys_execve;

use errno::Errno;

use core::ffi::c_void;

use crate::ffi::c_char;
use crate::interrupts::idt::{GateType, IdtGateEntry, InterruptTable};
use crate::memory::tools::address::Virt;
use crate::system::BaseRegisters;

extern "C" {
    fn _isr_syscall();
    fn _sys_test() -> i32;
    fn _get_esp() -> u32;

    fn _get_pit_time() -> u32;
    fn _get_process_end_time() -> u32;
}

/// Preemptif coherency checker
unsafe fn sys_test() -> SysResult<u32> {
    if _sys_test() == 0 {
        Ok(0)
    } else {
        Err(Errno::Eperm)
    }
}

/// Write something into the screen
fn sys_write(fd: i32, buf: *const u8, count: usize) -> SysResult<u32> {
    if fd != 1 {
        Err(Errno::Ebadf)
    } else {
        unsafe {
            unpreemptible_context!({
                /*
                    print!(
                    "{:?} / {:?} : {}",
                    _get_pit_time(),
                    _get_process_end_time(),
                    core::str::from_utf8_unchecked(core::slice::from_raw_parts(buf, count))
                );
                */
                print!("{}", core::str::from_utf8_unchecked(core::slice::from_raw_parts(buf, count)));
            })
        }
        Ok(count as u32)
    }
}

/// Exit from a process
unsafe fn sys_exit(status: i32) -> ! {
    unpreemptible();
    SCHEDULER.lock().current_task_exit(status);
}

/// Fork a process
unsafe fn sys_fork(kernel_esp: u32) -> SysResult<u32> {
    unpreemptible_context!({ SCHEDULER.lock().current_task_fork(kernel_esp) })
}

unsafe fn sys_getpid() -> SysResult<u32> {
    Ok(unpreemptible_context!({ SCHEDULER.lock().current_task_pid() }))
}

/// Do a stack overflow on the kernel stack
#[allow(unconditional_recursion)]
unsafe fn sys_stack_overflow(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32) -> SysResult<u32> {
    unpreemptible_context!({
        println!("Stack overflow syscall on the fly: v = {:?}, esp: {:#X?}", a + (b + c + d + e + f) * 0, _get_esp());
    });

    Ok(sys_stack_overflow(a + 1, b + 1, c + 1, d + 1, e + 1, f + 1).unwrap())
}

/// Global syscall interrupt handler called from assembly code
/// See https://www.informatik.htw-dresden.de/~beck/ASM/syscall_list.html
#[no_mangle]
pub unsafe extern "C" fn syscall_interrupt_handler(cpu_state: *mut CpuState) {
    #[allow(unused_variables)]
    let BaseRegisters { eax, ebx, ecx, edx, esi, edi, ebp, .. } = (*cpu_state).registers;

    let result = match eax {
        1 => sys_exit(ebx as i32),       // This syscall doesn't return !
        2 => sys_fork(cpu_state as u32), // CpuState represents kernel_esp
        3 => sys_read(ebx as i32, ecx as *mut u8, edx as usize),
        4 => sys_write(ebx as i32, ecx as *const u8, edx as usize),
        6 => sys_close(ebx as i32),
        7 => sys_waitpid(ebx as i32, ecx as *mut i32, edx as i32),
        10 => sys_unlink(ebx as *const u8),
        11 => sys_execve(ebx as *const c_char, ecx as *const *const c_char, edx as *const *const c_char),
        20 => sys_getpid(),
        // 24 => sys_getuid(), TODO: need to be implemented
        29 => sys_pause(),
        37 => sys_kill(ebx as Pid, ecx as u32),
        48 => sys_signal(ebx as u32, ecx as usize),
        67 => sys_sigaction(ebx as u32, ecx as *const StructSigaction, edx as *mut StructSigaction),
        88 => sys_reboot(),
        90 => sys_mmap(ebx as *const MmapArgStruct),
        91 => sys_munmap(Virt(ebx as usize), ecx as usize),
        102 => sys_socketcall(ebx as u32, ecx as SocketArgsPtr),
        125 => sys_mprotect(Virt(ebx as usize), ecx as usize, MmapProt::from_bits_truncate(edx)),
        162 => sys_nanosleep(ebx as *const TimeSpec, ecx as *mut TimeSpec),
        200 => sys_sigreturn(cpu_state),
        293 => sys_shutdown(),
        0x80000000 => sys_test(),
        0x80000001 => sys_stack_overflow(0, 0, 0, 0, 0, 0),
        // set thread area: WTF
        0xf3 => Err(Errno::Eperm),
        sysnum => panic!("wrong syscall {}", sysnum),
    };

    let is_in_blocked_syscall = result == Err(Errno::Eintr);
    // Note: do not erase eax if we've just been interrupted from a blocked syscall as we must keep
    // the syscall number contained in eax, in case of SA_RESTART behavior
    if is_in_blocked_syscall == false {
        // Return value will be on EAX. Errno always represents the low 7 bits
        (*cpu_state).registers.eax = match result {
            Ok(return_value) => return_value as u32,
            Err(errno) => (-(errno as i32)) as u32,
        };
    }
    // If ring3 process -> Mark process on signal execution state, modify CPU state, prepare a signal frame. UNLOCK interruptible().
    // If ring0 process -> Can't happened normally
    unpreemptible_context! {{
        if SIGNAL_LOCK {
            SIGNAL_LOCK = false;
            SCHEDULER.lock().current_task_deliver_pending_signals(cpu_state, is_in_blocked_syscall);
        }
    }}
}

extern "C" {
    fn _schedule_next();
}

/// Initialize all the syscall system by creation of a new IDT entry at 0x80
pub fn init() {
    let mut interrupt_table = unsafe { InterruptTable::current_interrupt_table().unwrap() };

    let mut gate_entry = *IdtGateEntry::new()
        .set_storage_segment(false)
        .set_privilege_level(3)
        .set_selector(1 << 3)
        .set_gate_type(GateType::TrapGate32);
    gate_entry.set_handler(_isr_syscall as *const c_void as u32);
    interrupt_table[0x80] = gate_entry;
}
