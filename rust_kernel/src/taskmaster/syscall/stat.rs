use super::SysResult;

use super::scheduler::{Scheduler, SCHEDULER};
use super::vfs::{Path, VFS};
use core::convert::TryFrom;
use libc_binding::{c_char, stat};

pub fn statfn(scheduler: &Scheduler, path: Path) -> SysResult<stat> {
    let tg = scheduler.current_thread_group();
    let creds = &tg.credentials;
    let cwd = &tg.cwd;
    VFS.lock().stat(cwd, creds, path)
}

pub fn sys_stat(filename: *const c_char, buf: *mut stat) -> SysResult<u32> {
    unpreemptible_context!({
        let scheduler = SCHEDULER.lock();

        // Check if given pointers are not bullshit
        let (safe_filename, safe_buf) = {
            let v = scheduler
                .current_thread()
                .unwrap_process()
                .get_virtual_allocator();

            (
                v.make_checked_str(filename)?,
                v.make_checked_ref_mut::<stat>(buf)?,
            )
        };
        let path = Path::try_from(safe_filename)?;
        match statfn(&scheduler, path) {
            Ok(stat) => {
                *safe_buf = stat;
                Ok(0)
            }
            Err(value) => Err(value),
        }
    })
}
