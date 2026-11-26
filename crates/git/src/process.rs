//! Shared child setup and cleanup; output policies live with their callers.
use std::{
    io,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
};

#[cfg(unix)]
use std::{thread, time::Duration};

mod write;
pub(super) use write::{complete_write_output, diagnostic_output, write_output};

mod readers;
pub(super) use readers::Readers;

#[cfg(unix)]
mod bounded;
#[cfg(unix)]
pub(crate) use bounded::{bounded_output, prefix_output, OutputLimits, PrefixOutput};

/// A child created by this module; on Unix its private constructor binds group
/// cleanup to isolation setup instead of accepting arbitrary Child handles.
#[derive(Debug)]
pub(super) struct ManagedChild {
    child: Child,
}

impl ManagedChild {
    pub(super) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    pub(super) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub(super) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(super) fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }
}

/// Start a piped child with closed stdin and a separate process group on Unix.
pub(super) fn spawn(command: &mut Command) -> io::Result<ManagedChild> {
    spawn_with_stdin(command, Stdio::null())
}

/// Start a managed child with the caller's input policy and two output pipes.
pub(super) fn spawn_with_stdin(command: &mut Command, stdin: Stdio) -> io::Result<ManagedChild> {
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn().map(|child| ManagedChild { child })
}

/// Stop the managed process group on Unix, then reap the direct child.
/// Windows currently stops only the direct child; native tree cleanup is pending.
#[cfg_attr(
    unix,
    expect(
        unsafe_code,
        reason = "Unix process-group signalling is isolated here; Git children must stop together"
    )
)]
pub(super) fn terminate(managed: &mut ManagedChild) {
    let child = &mut managed.child;
    #[cfg(unix)]
    {
        // Every managed command starts its own process group, including Git's transport helpers.
        let group = -(child.id() as libc::pid_t);
        // SAFETY: kill takes integer identifiers, with no pointer/lifetime obligations.
        // The group comes from our child started with process_group(0).
        unsafe {
            libc::kill(group, libc::SIGTERM);
        }
        thread::sleep(Duration::from_millis(250));
        // SAFETY: same integer-only syscall contract; failure is tolerated because
        // the direct child is killed and waited below as well.
        unsafe {
            libc::kill(group, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}
