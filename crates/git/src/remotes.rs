//! Explicit remote operations using existing Git transport credentials.
use super::{commit_basis, file_path, run_write, transfer, TransferControl, WRITE_LOCK};
use desktop_core::{CommitBasis, Remote, RemoteAction, RemoteUrlKind};
use std::{
    ffi::{OsStr, OsString},
    io,
    path::{Path, PathBuf},
};

const REMOTE_LIMIT: usize = 64;
const URL_LIMIT: usize = 64;
const METADATA_LIMIT: usize = 64 * 1024;

fn read_metadata<S: AsRef<OsStr>>(path: &Path, args: &[S]) -> io::Result<Vec<u8>> {
    super::read_output::complete(path, args, METADATA_LIMIT)
}

fn lines(bytes: &[u8]) -> io::Result<Vec<OsString>> {
    bytes
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .map(|s| file_path(s).map(PathBuf::into_os_string))
        .collect()
}

pub(super) fn list(path: &Path) -> io::Result<Vec<Remote>> {
    let basis = commit_basis(path)?;
    let names = lines(&read_metadata(path, &["remote"])?)?;
    if names.len() > REMOTE_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Repository exceeds the 64-remote limit",
        ));
    }
    let mut upstream_remote = None;
    let mut upstream_ref = None;
    if let Some(branch) = &basis.branch {
        let raw = read_metadata(
            path,
            &[
                OsStr::new("for-each-ref"),
                OsStr::new("--format=%(upstream:remotename)%00%(upstream:remoteref)"),
                OsStr::new("--"),
                branch.as_os_str(),
            ],
        )?;
        let raw = raw.strip_suffix(b"\n").unwrap_or(&raw);
        if let Some(separator) = raw.iter().position(|b| *b == 0) {
            let (remote, rest) = raw.split_at(separator);
            let reference = &rest[1..];
            if !remote.is_empty() {
                upstream_remote = Some(file_path(remote)?.into_os_string());
                upstream_ref = Some(file_path(reference)?.into_os_string());
            }
        }
    }
    names
        .into_iter()
        .map(|name| {
            let fetch_urls = lines(&read_metadata(
                path,
                &[
                    OsStr::new("remote"),
                    OsStr::new("get-url"),
                    OsStr::new("--all"),
                    OsStr::new("--"),
                    &name,
                ],
            )?)?;
            let push_urls = lines(&read_metadata(
                path,
                &[
                    OsStr::new("remote"),
                    OsStr::new("get-url"),
                    OsStr::new("--push"),
                    OsStr::new("--all"),
                    OsStr::new("--"),
                    &name,
                ],
            )?)?;
            if fetch_urls.len() > URL_LIMIT || push_urls.len() > URL_LIMIT {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Remote endpoint exceeds the 64-URL limit",
                ));
            }
            let target_branch = if basis.head.is_none() {
                None
            } else if let Some(upstream_remote) = &upstream_remote {
                if upstream_remote == &name {
                    upstream_ref.clone()
                } else {
                    None
                }
            } else {
                basis.branch.clone()
            };
            Ok(Remote {
                name,
                fetch_urls,
                push_urls,
                target_branch,
            })
        })
        .collect()
}

/// Fetch, fast-forward pull, or non-forced push to an explicitly selected remote.
/// Push sends only HEAD to the tracked remote branch, or the same local branch
/// name when unpublished, and establishes tracking after success.
///
/// # Errors
///
/// Returns an error for unsupported endpoints, stale repository state, cancellation, timeout or Git/transport failure.
pub fn remote_operation(
    path: &Path,
    remote: &Remote,
    action: RemoteAction,
    expected: &CommitBasis,
) -> io::Result<String> {
    remote_operation_with_control(path, remote, action, expected, &TransferControl::default())
}

/// Execute a remote action with live diagnostics, cancellation and a deadline.
///
/// # Errors
///
/// Returns an error for unsupported endpoints, stale state, cancellation, timeout or Git/transport failure.
pub fn remote_operation_with_control(
    path: &Path,
    remote: &Remote,
    action: RemoteAction,
    expected: &CommitBasis,
    control: &TransferControl,
) -> io::Result<String> {
    let _guard = transfer::lock(control)?;
    if !list(path)?.contains(remote) {
        return Err(io::Error::other(
            "Remote configuration changed; refresh before continuing",
        ));
    }
    if !matches!(action, RemoteAction::Fetch) && &commit_basis(path)? != expected {
        return Err(io::Error::other(
            "Index or HEAD changed; refresh before using the remote",
        ));
    }
    let mut args: Vec<OsString> = ["-c", "protocol.ext.allow=never"]
        .map(OsString::from)
        .to_vec();
    match action {
        RemoteAction::Fetch => {
            args.extend(
                ["fetch", "--progress", "--no-recurse-submodules", "--"].map(OsString::from),
            );
            args.push(remote.name.clone());
        }
        RemoteAction::Pull | RemoteAction::Push => {
            let target = remote.target_branch.as_ref().ok_or_else(|| {
                io::Error::other(
                    "Select the tracked remote and a committed local branch before pull/push",
                )
            })?;
            if !target.as_encoded_bytes().starts_with(b"refs/heads/") {
                return Err(io::Error::other("Remote target must be a branch"));
            }
            if matches!(action, RemoteAction::Pull) {
                args.extend(
                    [
                        "pull",
                        "--progress",
                        "--ff-only",
                        "--no-rebase",
                        "--no-autostash",
                        "--no-squash",
                        "--no-edit",
                        "--no-recurse-submodules",
                        "--",
                    ]
                    .map(OsString::from),
                );
                args.push(remote.name.clone());
                args.push(target.clone());
            } else {
                // Git loads remote.<name>.mirror after parsing --no-mirror;
                // override the configuration as well as supplying the CLI flag.
                let mut mirror = OsString::from("remote.");
                mirror.push(&remote.name);
                mirror.push(".mirror=false");
                args.push(OsString::from("-c"));
                args.push(mirror);
                args.extend(
                    [
                        "push",
                        "--progress",
                        "--porcelain",
                        "--no-force",
                        "--no-mirror",
                        "--no-follow-tags",
                        "--set-upstream",
                        "--",
                    ]
                    .map(OsString::from),
                );
                args.push(remote.name.clone());
                let mut refspec = OsString::from("HEAD:");
                refspec.push(target);
                args.push(refspec);
            }
        }
    }
    let output = transfer::run(path, &args, control)?;
    Ok(if output.is_empty() {
        format!("{action:?} completed")
    } else {
        output
    })
}

/// Clone into a newly reserved destination. Existing paths are never reused.
/// A failed clone may leave partial files for manual inspection; no recursive
/// cleanup is performed. Credentials come from Git helpers/SSH, not this API.
///
/// # Errors
///
/// Returns an error for invalid sources/destinations, cancellation, timeout or Git/transport failure; partial output may remain.
pub fn clone_repository(source: &str, destination: &Path) -> io::Result<PathBuf> {
    clone_repository_with_control(source, destination, &TransferControl::default())
}

/// Clone with live diagnostics, cancellation and a deadline.
///
/// # Errors
///
/// Returns an error for invalid sources/destinations, cancellation, timeout or Git/transport failure; partial output may remain.
pub fn clone_repository_with_control(
    source: &str,
    destination: &Path,
    control: &TransferControl,
) -> io::Result<PathBuf> {
    let _guard = transfer::lock(control)?;
    validate_source(source)?;
    let destination = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        std::env::current_dir()?.join(destination)
    };
    let name = destination
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid clone destination"))?;
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing destination parent"))?
        .canonicalize()?;
    let destination = parent.join(name);
    std::fs::create_dir(&destination)?;
    control.check()?;
    transfer::run(
        &parent,
        &[
            OsStr::new("-c"),
            OsStr::new("protocol.ext.allow=never"),
            OsStr::new("clone"),
            OsStr::new("--progress"),
            OsStr::new("--"),
            OsStr::new(source),
            destination.as_os_str(),
        ],
        control,
    )?;
    Ok(destination)
}

fn validate_source(source: &str) -> io::Result<()> {
    if source.trim().is_empty()
        || source.starts_with('-')
        || source.split_once("::").is_some_and(|(prefix, _)| {
            !prefix.is_empty()
                && prefix
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        })
        || source.contains(['\n', '\r', '\0'])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid Git URL or local path",
        ));
    }
    if let Some((scheme, _)) = source.split_once("://") {
        if !matches!(scheme, "https" | "http" | "ssh" | "git" | "file") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unsupported Git transport",
            ));
        }
    }
    Ok(())
}

fn validate_endpoint(url: &str) -> io::Result<()> {
    validate_source(url)?;
    if url.len() >= METADATA_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Remote URL exceeds the metadata limit",
        ));
    }
    Ok(())
}

/// Add a named remote without fetching or changing branch tracking.
///
/// # Errors
///
/// Returns an error for invalid/duplicate names, unsupported URLs or Git/configuration failure.
pub fn add_remote(path: &Path, name: &str, url: &str) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    if name.is_empty() || name.starts_with('-') || name == "." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid remote name",
        ));
    }
    read_metadata(
        path,
        &["check-ref-format", &format!("refs/remotes/{name}/probe")],
    )?;
    validate_endpoint(url)?;
    let remotes = list(path)?;
    if remotes.iter().any(|remote| remote.name == OsStr::new(name)) {
        return Err(io::Error::other("Remote already exists"));
    }
    let names_size: usize = remotes
        .iter()
        .map(|remote| remote.name.as_encoded_bytes().len() + 1)
        .sum();
    if remotes.len() == REMOTE_LIMIT
        || names_size.saturating_add(name.len()).saturating_add(1) > METADATA_LIMIT
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Adding this remote would exceed the remote metadata limits",
        ));
    }
    run_write(path, &["remote", "add", "--", name, url], None)?;
    Ok(format!("Added remote {name}"))
}

fn validate_remote(path: &Path, selected: &Remote) -> io::Result<()> {
    if !list(path)?.contains(selected) {
        return Err(io::Error::other(
            "Remote configuration changed; refresh before editing",
        ));
    }
    Ok(())
}

/// Replace the sole URL for one endpoint, retaining the other endpoint's config.
/// Multi-URL endpoints are rejected rather than silently replacing only one URL.
///
/// # Errors
///
/// Returns an error for a stale remote, invalid/unsupported endpoints or Git/configuration failure.
pub fn set_remote_url(
    path: &Path,
    selected: &Remote,
    kind: RemoteUrlKind,
    url: &str,
) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_endpoint(url)?;
    validate_remote(path, selected)?;
    let urls = match kind {
        RemoteUrlKind::Fetch => &selected.fetch_urls,
        RemoteUrlKind::Push => &selected.push_urls,
    };
    if urls.len() != 1 {
        return Err(io::Error::other(
            "Use Git to edit a remote endpoint with multiple URLs",
        ));
    }
    let mut args = vec![OsStr::new("remote"), OsStr::new("set-url")];
    if matches!(kind, RemoteUrlKind::Push) {
        args.push(OsStr::new("--push"));
    }
    args.extend([OsStr::new("--"), selected.name.as_os_str(), OsStr::new(url)]);
    run_write(path, &args, None)?;
    Ok(format!(
        "Updated {kind:?} URL for {}",
        selected.name.to_string_lossy()
    ))
}

/// Remove local remote configuration and its remote-tracking refs using Git.
/// Local branches, working-tree files and the server repository are not deleted.
///
/// # Errors
///
/// Returns an error if the selected remote changed or Git/configuration removal fails.
pub fn remove_remote(path: &Path, selected: &Remote) -> io::Result<String> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| io::Error::other("Write lock poisoned"))?;
    validate_remote(path, selected)?;
    run_write(
        path,
        &[
            OsStr::new("remote"),
            OsStr::new("remove"),
            OsStr::new("--"),
            selected.name.as_os_str(),
        ],
        None,
    )?;
    Ok(format!(
        "Removed remote {}",
        selected.name.to_string_lossy()
    ))
}
