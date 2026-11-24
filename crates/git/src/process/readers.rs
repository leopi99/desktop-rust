//! Reader-pair ownership: fallible startup and unconditional joining of both pipes.
use std::{
    io,
    process::{ChildStderr, ChildStdout},
    thread::{self, JoinHandle},
};

use super::ManagedChild;

/// Own both pipe workers until the controller has stopped or reaped its child.
#[derive(Debug)]
pub(crate) struct Readers<T> {
    stdout: JoinHandle<io::Result<T>>,
    stderr: JoinHandle<io::Result<T>>,
}

impl<T: Send + 'static> Readers<T> {
    /// On any startup failure, stop/reap the child and join any started reader.
    pub(crate) fn start(
        child: &mut ManagedChild,
        stdout: impl FnOnce(ChildStdout) -> io::Result<T> + Send + 'static,
        stderr: impl FnOnce(ChildStderr) -> io::Result<T> + Send + 'static,
    ) -> io::Result<Self> {
        let Some(out) = child.take_stdout() else {
            super::terminate(child);
            return Err(io::Error::other("Git stdout pipe was not created"));
        };
        let Some(err) = child.take_stderr() else {
            super::terminate(child);
            return Err(io::Error::other("Git stderr pipe was not created"));
        };
        Self::start_with(
            child,
            move || {
                thread::Builder::new()
                    .name("git-output-stdout".into())
                    .spawn(move || stdout(out))
            },
            move || {
                thread::Builder::new()
                    .name("git-output-stderr".into())
                    .spawn(move || stderr(err))
            },
        )
    }

    fn start_with(
        child: &mut ManagedChild,
        stdout: impl FnOnce() -> io::Result<JoinHandle<io::Result<T>>>,
        stderr: impl FnOnce() -> io::Result<JoinHandle<io::Result<T>>>,
    ) -> io::Result<Self> {
        let stdout = match stdout() {
            Ok(reader) => reader,
            Err(error) => {
                super::terminate(child);
                return Err(error);
            }
        };
        let stderr = match stderr() {
            Ok(reader) => reader,
            Err(error) => {
                super::terminate(child);
                let _ = stdout.join();
                return Err(error);
            }
        };
        Ok(Self { stdout, stderr })
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.stdout.is_finished() && self.stderr.is_finished()
    }

    /// Always join both before propagating either thread's panic or I/O error.
    /// The caller must monitor/terminate the child before joining blocked readers.
    pub(crate) fn join(self) -> io::Result<(T, T)> {
        let stdout = self.stdout.join();
        let stderr = self.stderr.join();
        let stdout = stdout.map_err(|_| io::Error::other("Git stdout reader failed"))?;
        let stderr = stderr.map_err(|_| io::Error::other("Git stderr reader failed"))?;
        Ok((stdout?, stderr?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn both_readers_are_joined_before_returning_a_panic_or_io_error() {
        for panic in [false, true] {
            let (release, wait) = mpsc::channel();
            let (finished, result) = mpsc::channel();
            let stdout = thread::spawn(move || -> io::Result<()> {
                if panic {
                    panic!("injected reader failure");
                }
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected I/O failure",
                ))
            });
            let stderr = thread::spawn(move || {
                wait.recv().unwrap();
                Ok(())
            });
            let worker = thread::spawn(move || {
                finished.send(Readers { stdout, stderr }.join()).unwrap();
            });
            let premature = result.recv_timeout(Duration::from_millis(50));
            release.send(()).unwrap();
            worker.join().unwrap();
            assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)));
            let error = result.recv().unwrap().unwrap_err();
            assert_eq!(
                error.kind(),
                if panic {
                    io::ErrorKind::Other
                } else {
                    io::ErrorKind::BrokenPipe
                }
            );
        }
    }

    #[cfg(unix)]
    fn child() -> ManagedChild {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", "sleep 30"]);
        super::super::spawn(&mut command).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn first_reader_start_failure_reaps_child_without_starting_second() {
        let mut child = child();
        let error = Readers::<Vec<u8>>::start_with(
            &mut child,
            || {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "injected spawn failure",
                ))
            },
            || panic!("second reader must not start"),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(child.try_wait().unwrap().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn second_reader_start_failure_reaps_child_and_joins_first_reader() {
        use std::{
            io::Read,
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
        };
        let mut child = child();
        let mut stdout = child.take_stdout().unwrap();
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let error = Readers::start_with(
            &mut child,
            move || {
                thread::Builder::new().spawn(move || {
                    let mut bytes = Vec::new();
                    stdout.read_to_end(&mut bytes)?;
                    worker_finished.store(true, Ordering::Release);
                    Ok(bytes)
                })
            },
            || {
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "injected spawn failure",
                ))
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(finished.load(Ordering::Acquire));
        assert!(child.try_wait().unwrap().is_some());
    }
}
