use super::scheduler::SCHEDULER;
use super::vfs::Path;
use super::SysResult;
use core::convert::TryFrom;
use libc_binding::{c_char, utimbuf};

use core::ptr;

pub fn sys_utime(path: *const c_char, times: *const utimbuf) -> SysResult<u32> {
    unpreemptible_context!({
        let mut scheduler = SCHEDULER.lock();

        let (safe_buf, _times) = {
            let v = scheduler
                .current_thread()
                .unwrap_process()
                .get_virtual_allocator();

            (
                v.make_checked_str(path)?,
                if times == ptr::null() {
                    None
                } else {
                    Some(v.make_checked_ref(times))
                },
            )
        };

        let tg = scheduler.current_thread_group_mut();
        // let creds = &tg.credentials;
        let _cwd = &tg.cwd;
        let _path = Path::try_from(safe_buf)?;

        unimplemented!()
    })
}
