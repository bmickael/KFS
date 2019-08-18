use super::scheduler::{Pid, SCHEDULER};
use super::SysResult;
use errno::Errno;

/// The setpgid() function shall either join an existing process group
/// or create a new process group within the session of the calling
/// process.
///
/// The process group ID of a session leader shall not change.
///
/// Upon successful completion, the process group ID of the process
/// with a process ID that matches pid shall be set to pgid.
///
/// As a special case, if pid is 0, the process ID of the calling
/// process shall be used. Also, if pgid is 0, the process ID of the
/// indicated process shall be used.
pub fn sys_setpgid(pid: Pid, pgid: Pid) -> SysResult<u32> {
    unpreemptible_context!({
        let mut scheduler = SCHEDULER.lock();

        let pid = if pid == 0 {
            scheduler.current_task_id().0
        } else {
            pid
        };
        let pgid = if pgid == 0 { pid } else { pgid };
        scheduler
            .get_thread_group_mut(pid)
            .ok_or(Errno::Esrch)?
            .pgid = pgid;

        Ok(0)
    })
}
