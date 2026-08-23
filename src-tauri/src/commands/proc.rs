use execute::shell;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// How often to poll a spawned child for completion while waiting on it.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Runs `command` in a shell, capturing stdout/stderr, but never blocks for
/// longer than `timeout`. If the command hasn't exited by then, it is killed
/// and an error is returned instead.
///
/// Desktop-integration commands (`plasma-apply-colorscheme`, a GTK
/// `gsettings` call, a user's own template `post_hook`) are arbitrary
/// external processes. Without a bound, one that hangs — a stuck DBus call,
/// a `spicetify watch` that never sees its app reload — stalls the entire
/// "apply colors" flow for however long it takes, which from the UI looks
/// like the whole app freezing.
pub fn run_with_timeout(command: &str, timeout: Duration) -> Result<Output, String> {
    let mut cmd: Command = shell(command);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start command: {}", e))?;

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    let _ = err.read_to_end(&mut stderr);
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "Command timed out after {:?} and was killed: {}",
                        timeout, command
                    ));
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(e) => return Err(format!("Failed to wait on command: {}", e)),
        }
    }
}

/// Fire-and-forget variant for best-effort hooks whose output nobody reads
/// (e.g. template `post_hook`s): stdout/stderr go to `/dev/null` so a chatty
/// command can never fill a pipe buffer and deadlock, and the result is
/// discarded either way — the caller already treats these as non-fatal.
pub fn run_ignoring_result(command: &str, timeout: Duration) {
    let mut cmd: Command = shell(command);
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return;
    };

    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_with_timeout_returns_promptly_when_command_hangs() {
        let start = Instant::now();
        let result = run_with_timeout("sleep 5", Duration::from_millis(300));
        let elapsed = start.elapsed();
        assert!(
            result.is_err(),
            "a hung command must be reported as timed out"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "run_with_timeout blocked for {:?} instead of respecting the 300ms bound",
            elapsed
        );
    }

    #[test]
    fn run_with_timeout_returns_output_for_fast_commands() {
        let output = run_with_timeout("echo hello", Duration::from_secs(5)).unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn run_ignoring_result_returns_promptly_when_command_hangs() {
        let start = Instant::now();
        run_ignoring_result("sleep 5", Duration::from_millis(300));
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "run_ignoring_result blocked for {:?} instead of respecting the 300ms bound",
            elapsed
        );
    }
}
