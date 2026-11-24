//! Bounded transfer diagnostics and cooperative process-group cancellation.
use std::{
    ffi::OsStr,
    io::{self, Read},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard, TryLockError,
    },
    thread,
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 64 * 1024;
const PROGRESS_LIMIT: usize = 16 * 1024;

#[derive(Debug)]
struct State {
    cancelled: AtomicBool,
    progress: Mutex<String>,
    started: Instant,
    timeout: Duration,
}

/// Shared cancellation token and bounded live diagnostics for one transfer.
#[derive(Debug, Clone)]
pub struct TransferControl(Arc<State>);

impl Default for TransferControl {
    fn default() -> Self {
        Self::with_timeout(Duration::from_secs(15 * 60))
    }
}

impl TransferControl {
    /// Start a transfer deadline, including time spent waiting for the write lock.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self(Arc::new(State {
            cancelled: AtomicBool::new(false),
            progress: Mutex::new(String::new()),
            started: Instant::now(),
            timeout,
        }))
    }
    /// Request termination; callers must await the operation result before retrying.
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Relaxed);
    }
    /// Read the latest diagnostics, bounded to 16 KiB.
    pub fn progress(&self) -> String {
        self.0
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub(super) fn check(&self) -> io::Result<()> {
        if self.0.cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Transfer cancelled; changes may already have reached the remote. Fetch to verify.",
            ));
        }
        if self.0.started.elapsed() >= self.0.timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Transfer timed out; changes may already have reached the remote. Fetch to verify.",
            ));
        }
        Ok(())
    }
    fn append(&self, bytes: &[u8]) {
        let mut text = self.0.progress.lock().unwrap_or_else(|e| e.into_inner());
        text.push_str(&String::from_utf8_lossy(bytes));
        if text.len() > PROGRESS_LIMIT {
            let mut start = text.len() - PROGRESS_LIMIT;
            while !text.is_char_boundary(start) {
                start += 1;
            }
            text.drain(..start);
        }
    }
}

pub(super) fn lock(control: &TransferControl) -> io::Result<MutexGuard<'static, ()>> {
    loop {
        control.check()?;
        match super::WRITE_LOCK.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => return Err(io::Error::other("Write lock poisoned")),
            Err(TryLockError::WouldBlock) => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn drain(mut pipe: impl Read, control: TransferControl) -> io::Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let count = match pipe.read(&mut chunk) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            return Ok(result);
        }
        control.append(&chunk[..count]);
        result.extend_from_slice(&chunk[..count]);
        if result.len() > OUTPUT_LIMIT {
            result.drain(..result.len() - OUTPUT_LIMIT);
        }
    }
}

pub(super) fn run<S: AsRef<OsStr>>(
    path: &Path,
    args: &[S],
    control: &TransferControl,
) -> io::Result<String> {
    control.check()?;
    let mut command = super::git_command(path);
    command.args(args);
    let mut child = super::process::spawn(&mut command)?;
    let out_control = control.clone();
    let err_control = control.clone();
    let readers = super::process::Readers::start(
        &mut child,
        move |stdout| drain(stdout, out_control),
        move |stderr| drain(stderr, err_control),
    )?;
    let mut status = None;
    let outcome = loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(result) => status = result,
                Err(error) => {
                    super::process::terminate(&mut child);
                    break Err(error);
                }
            }
        }
        if let Some(status) = status {
            if readers.is_finished() {
                break Ok(status);
            }
        }
        if let Err(error) = control.check() {
            super::process::terminate(&mut child);
            break Err(error);
        }
        thread::sleep(Duration::from_millis(20));
    };
    // Readers are drained before releasing the repository write lock.
    let captured = readers.join();
    let status = outcome?;
    let (stdout, stderr) = captured?;
    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    );
    if !status.success() {
        return Err(io::Error::other(diagnostics));
    }
    Ok(diagnostics)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn alias(body: &str, control: &TransferControl) -> io::Result<String> {
        run(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &[
                "-c",
                &format!("alias.transfer-test=!{body}"),
                "transfer-test",
            ],
            control,
        )
    }
    #[test]
    fn cancellation_stops_git_and_its_sleeping_helper() {
        let control = TransferControl::default();
        let worker_control = control.clone();
        let started = Instant::now();
        let worker =
            thread::spawn(move || alias("printf 'ready\r' >&2; sleep 30", &worker_control));
        while !control.progress().contains("ready") {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        control.cancel();
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[test]
    fn deadline_stops_stalled_transfer() {
        let control = TransferControl::with_timeout(Duration::from_millis(100));
        let started = Instant::now();
        assert_eq!(
            alias("sleep 30", &control).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }
    #[test]
    fn output_is_bounded_and_both_pipes_are_drained() {
        let control = TransferControl::default();
        let output = alias(
            "head -c 100000 /dev/zero; head -c 100000 /dev/zero >&2",
            &control,
        )
        .unwrap();
        assert_eq!(output.len(), 2 * OUTPUT_LIMIT);
        assert!(control.progress().len() <= PROGRESS_LIMIT);
    }
    #[test]
    fn deadline_remains_active_after_git_exits_with_inherited_pipes() {
        let control = TransferControl::with_timeout(Duration::from_millis(100));
        let started = Instant::now();
        assert_eq!(
            alias("sleep 30 & exit 0", &control).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        let retry = alias("printf 'retry succeeded'", &TransferControl::default()).unwrap();
        assert_eq!(retry, "retry succeeded");
    }

    #[test]
    fn cancellation_with_inherited_pipes_waits_for_cleanup_before_retry() {
        let control = TransferControl::default();
        let worker_control = control.clone();
        let started = Instant::now();
        let worker =
            thread::spawn(move || alias("printf 'ready' >&2; sleep 30 & exit 0", &worker_control));
        while !control.progress().contains("ready") {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        control.cancel();
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            alias("printf 'retry succeeded'", &TransferControl::default()).unwrap(),
            "retry succeeded"
        );
    }

    #[test]
    fn failed_transfer_retains_both_diagnostic_tails() {
        let control = TransferControl::default();
        let error = alias("head -c 100000 /dev/zero; printf 'stdout tail'; head -c 100000 /dev/zero >&2; printf 'stderr tail' >&2; exit 7", &control).unwrap_err();
        let diagnostics = error.to_string();
        assert_eq!(diagnostics.len(), OUTPUT_LIMIT * 2);
        assert!(diagnostics.contains("stdout tail"));
        assert!(diagnostics.ends_with("stderr tail"));
        assert!(control.progress().len() <= PROGRESS_LIMIT);
    }

    #[test]
    fn progress_keeps_valid_utf8_at_the_size_boundary() {
        let control = TransferControl::default();
        control.append("è".repeat(10000).as_bytes());
        assert!(control.progress().len() <= PROGRESS_LIMIT);
        assert!(control.progress().ends_with('è'));
    }
    #[test]
    fn cancellation_also_interrupts_waiting_for_the_write_lock() {
        let guard = super::super::WRITE_LOCK.lock().unwrap();
        let control = TransferControl::default();
        let worker_control = control.clone();
        let worker = thread::spawn(move || lock(&worker_control).map(|_| ()));
        control.cancel();
        assert_eq!(
            worker.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        drop(guard);
    }
}
