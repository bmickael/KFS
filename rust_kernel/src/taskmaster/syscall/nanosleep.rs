//! sys_nanosleep implementation

use super::SysResult;

use super::scheduler::SCHEDULER;
use super::scheduler::{auto_preempt};
use super::task::WaitingState;

use errno::Errno;

use crate::drivers::PIT0;
use crate::math::convert::Convert;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct TimeSpec {
    tv_sec: u32,
    tv_nsec: i32,
}

extern "C" {
    fn _get_pit_time() -> u32;
}

fn nanosleep(req: *const TimeSpec, rem: *mut TimeSpec) -> SysResult<u32> {
    let mut scheduler = SCHEDULER.lock();

    let v = &mut scheduler.curr_process_mut().unwrap_running_mut().virtual_allocator;

    v.check_user_ptr::<TimeSpec>(req)?;
    v.check_user_ptr::<TimeSpec>(rem)?;

    let nsec = unsafe { (*req).tv_nsec };
    if nsec < 0 || nsec >= 1000000000 {
        return Err(Errno::Einval);
    }

    // Set precision as 1/1000
    let request_time = unsafe { (*req).tv_sec as f32 + ((*req).tv_nsec / 1000000) as f32 / 1000. };
    let pit_period = 1. / PIT0.lock().get_frequency().expect("PIT0 not initialized");
    let next_wake = (request_time / pit_period) as u32 + unsafe { _get_pit_time() };

    // Set as Sleeping
    scheduler.curr_process_mut().set_waiting(WaitingState::Sleeping(next_wake));

    // auto preemption mechanism set environement as preemptible
    if auto_preempt() < 0 {
        let now = unsafe { _get_pit_time() };
        if now < next_wake {
            let remaining_time = (next_wake - now) as f32 * pit_period;
            unsafe {
                (*rem).tv_sec = remaining_time.trunc() as u32;
                (*rem).tv_nsec = ((remaining_time * 1000.).trunc() as u32 % 1000 * 1000000) as i32;
            }
        }
        Err(Errno::Eintr)
    } else {
        Ok(0)
    }
}

pub fn sys_nanosleep(req: *const TimeSpec, rem: *mut TimeSpec) -> SysResult<u32> {
    unpreemptible_context!({ nanosleep(req, rem) })
}
