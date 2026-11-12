use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// The shell ends with this to be started again as usual, out of safe mode.
pub const AGAIN: i32 = 75;

// This many crashes this close together and the next start is a safe one.
const CRASHES: usize = 3;
const WITHIN: Duration = Duration::from_secs(60);
const PAUSE: Duration = Duration::from_millis(500);

static SAFE: AtomicBool = AtomicBool::new(false);

// Safe mode runs on the defaults and draws on the CPU, and leaves the person's files alone.
pub fn set_safe() {
    SAFE.store(true, Ordering::Relaxed);
}

pub fn safe() -> bool {
    SAFE.load(Ordering::Relaxed)
}

// The shell runs as a child of this process, which starts it again when it crashes, and safe
// after a few crashes in a row, so a bad config or driver can't leave the session without a bar.
pub fn watch(args: &[String], safe: bool) -> ExitCode {
    let mut runs = Runs::new(safe);
    loop {
        let status = match start(args, runs.safe) {
            Ok(status) => status,
            Err(err) => {
                eprintln!("apolline: cannot start the shell: {err}");
                return ExitCode::FAILURE;
            }
        };
        let end = End::of(status);
        // A panic has said why itself; a signal leaves no one else to say it.
        if end == End::Crashed
            && let Some(signal) = status.signal()
        {
            crate::crashes::record(&format!("shell was killed by {}", name(signal)));
        }
        if !compositor_up() {
            return ExitCode::SUCCESS;
        }
        match runs.next(end, Instant::now()) {
            Next::Start => std::thread::sleep(PAUSE),
            Next::Stop => return ExitCode::SUCCESS,
            Next::GiveUp => {
                crate::crashes::record(
                    "shell keeps crashing in safe mode too; not starting it again",
                );
                return ExitCode::FAILURE;
            }
        }
    }
}

fn start(args: &[String], safe: bool) -> std::io::Result<ExitStatus> {
    // The way this one was started, so an upgraded binary is the one that runs next.
    let me = std::env::args_os()
        .next()
        .unwrap_or_else(|| "apolline".into());
    let mut command = Command::new(me);
    command
        .args(args.iter().filter(|arg| *arg != "--safe"))
        .arg("--watched");
    if safe {
        command.arg("--safe").env("ICED_BACKEND", "tiny-skia");
    }
    crate::child::tie(&mut command, libc::SIGTERM).status()
}

// When niri is gone, so is the session: nothing to start again.
fn compositor_up() -> bool {
    let Some(display) = std::env::var_os("WAYLAND_DISPLAY").map(PathBuf::from) else {
        return false;
    };
    let socket = if display.is_absolute() {
        display
    } else {
        match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(dir) => PathBuf::from(dir).join(display),
            None => return false,
        }
    };
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

fn name(signal: i32) -> String {
    let known = match signal {
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGILL => "SIGILL",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        _ => return format!("signal {signal}"),
    };
    known.to_owned()
}

#[derive(Debug, PartialEq)]
enum End {
    Stopped,
    Again,
    Crashed,
}

impl End {
    fn of(status: ExitStatus) -> Self {
        match (status.code(), status.signal()) {
            (Some(0), _) => Self::Stopped,
            (Some(AGAIN), _) => Self::Again,
            // Asked to: pkill, or the session ending.
            (None, Some(libc::SIGTERM | libc::SIGINT | libc::SIGHUP)) => Self::Stopped,
            _ => Self::Crashed,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Next {
    Start,
    Stop,
    GiveUp,
}

struct Runs {
    safe: bool,
    crashes: Vec<Instant>,
}

impl Runs {
    fn new(safe: bool) -> Self {
        Self {
            safe,
            crashes: Vec::new(),
        }
    }

    fn next(&mut self, end: End, now: Instant) -> Next {
        match end {
            End::Stopped => Next::Stop,
            End::Again => {
                self.safe = false;
                self.crashes.clear();
                Next::Start
            }
            End::Crashed => {
                self.crashes
                    .retain(|at| now.saturating_duration_since(*at) < WITHIN);
                self.crashes.push(now);
                if self.crashes.len() < CRASHES {
                    Next::Start
                } else if self.safe {
                    Next::GiveUp
                } else {
                    self.safe = true;
                    self.crashes.clear();
                    Next::Start
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_the_shell_ended() {
        let exited = |code: i32| ExitStatus::from_raw(code << 8);
        let killed = ExitStatus::from_raw;
        assert_eq!(End::of(exited(0)), End::Stopped);
        assert_eq!(End::of(exited(AGAIN)), End::Again);
        assert_eq!(End::of(exited(101)), End::Crashed);
        assert_eq!(End::of(exited(1)), End::Crashed);
        assert_eq!(End::of(killed(libc::SIGTERM)), End::Stopped);
        assert_eq!(End::of(killed(libc::SIGHUP)), End::Stopped);
        assert_eq!(End::of(killed(libc::SIGSEGV)), End::Crashed);
        assert_eq!(End::of(killed(libc::SIGKILL)), End::Crashed);
        assert_eq!(name(libc::SIGSEGV), "SIGSEGV");
        assert_eq!(name(libc::SIGUSR1), format!("signal {}", libc::SIGUSR1));
    }

    #[test]
    fn three_quick_crashes_start_it_safe_and_three_more_give_up() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut runs = Runs::new(false);
        assert_eq!(runs.next(End::Crashed, at(0)), Next::Start);
        assert_eq!(runs.next(End::Crashed, at(10)), Next::Start);
        assert!(!runs.safe);
        assert_eq!(runs.next(End::Crashed, at(20)), Next::Start);
        assert!(runs.safe);
        assert_eq!(runs.next(End::Crashed, at(21)), Next::Start);
        assert_eq!(runs.next(End::Crashed, at(22)), Next::Start);
        assert_eq!(runs.next(End::Crashed, at(23)), Next::GiveUp);
    }

    #[test]
    fn crashes_far_apart_just_start_it_again() {
        let start = Instant::now();
        let mut runs = Runs::new(false);
        for minute in 0..10 {
            let at = start + Duration::from_secs(minute * 61);
            assert_eq!(runs.next(End::Crashed, at), Next::Start);
        }
        assert!(!runs.safe);
    }

    #[test]
    fn asked_again_leaves_safe_mode_and_stopped_stays_stopped() {
        let mut runs = Runs::new(true);
        assert_eq!(runs.next(End::Crashed, Instant::now()), Next::Start);
        assert_eq!(runs.next(End::Again, Instant::now()), Next::Start);
        assert!(!runs.safe && runs.crashes.is_empty());
        assert_eq!(runs.next(End::Stopped, Instant::now()), Next::Stop);
    }
}
