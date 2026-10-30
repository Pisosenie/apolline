use std::process::Command;

// The child gets `signal` when the shell goes away, however it goes.
pub fn tie(command: &mut Command, signal: i32) -> &mut Command {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: prctl is async-signal-safe and touches nothing shared.
        unsafe {
            command.pre_exec(move || {
                libc::prctl(libc::PR_SET_PDEATHSIG, signal);
                Ok(())
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = signal;
    command
}

// Ends a child's whole process group when dropped, unless let go first.
pub struct Group(Option<i32>);

impl Group {
    pub fn new(pid: Option<u32>) -> Self {
        Self(pid.and_then(|pid| i32::try_from(pid).ok()))
    }

    pub fn let_go(&mut self) {
        self.0 = None;
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        if let Some(group) = self.0 {
            // SAFETY: plain syscall; the leader is not reaped yet, so the id is still ours.
            unsafe {
                libc::killpg(group, libc::SIGTERM);
            }
        }
    }
}
