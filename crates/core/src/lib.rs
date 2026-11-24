//! UI-independent repository models.
use std::path::PathBuf;

/// Read-only version reported by the Git executable resolved through PATH.
/// This observation does not pin the binary used by later repository operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRuntime {
    /// Reported version string, preserving vendor and prerelease suffixes.
    pub version: String,
}

/// A working repository identified by its filesystem root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    /// Filesystem root in its native encoding.
    pub path: PathBuf,
}

/// A working repository identified by its filesystem root.
/// Observed repository state used by views and freshness checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    /// Repository to which this observation belongs.
    pub repository: Repository,
    /// Human-readable short status and branch summary.
    pub status: String,
    /// Human-readable summary of the recent commit page.
    pub history: String,
    /// Recent commit page anchored at the observed HEAD.
    pub commits: Vec<CommitSummary>,
    /// Observed staged, unstaged, untracked and conflicted paths.
    pub changes: Vec<FileChange>,
    /// Observed local refs with their object IDs.
    pub branches: Vec<LocalBranch>,
    /// Configured remotes and explicit transfer destinations.
    pub remotes: Vec<Remote>,
    /// Saved changes addressed by immutable object IDs.
    pub stashes: Vec<StashEntry>,
    /// Suggested message for an in-progress merge, if any.
    pub merge_message: Option<String>,
    /// Index and HEAD captured for the pre-commit freshness check.
    pub commit_basis: CommitBasis,
}

/// A porcelain status entry; filesystem paths retain their original encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Current path relative to the repository root.
    pub path: PathBuf,
    /// Previous relative path for a rename or copy, when reported.
    pub original_path: Option<PathBuf>,
    /// Index status character from Git porcelain output.
    pub index: char,
    /// Working-tree status character from Git porcelain output.
    pub worktree: char,
}

/// Separate full-file patches for index and working-tree changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Patch relative to HEAD for staged contents.
    pub staged: String,
    /// Patch relative to the index for working-tree contents.
    pub unstaged: String,
}

impl FileChange {
    /// Whether Git reports an unmerged index entry.
    pub fn is_conflicted(&self) -> bool {
        self.index == 'U'
            || self.worktree == 'U'
            || matches!((self.index, self.worktree), ('A', 'A') | ('D', 'D'))
    }

    /// Whether full-file staging is available in this preview.
    pub fn can_stage(&self) -> bool {
        !self.is_conflicted() && self.worktree != ' '
    }

    /// Whether an index change can be reset without touching the working tree.
    pub fn can_unstage(&self) -> bool {
        !self.is_conflicted() && !matches!(self.index, ' ' | '?')
    }
}

/// Basis reviewed by the user before a commit is requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitBasis {
    /// NUL-separated index entries from ls-files --stage.
    pub index: Vec<u8>,
    /// Observed HEAD object ID as returned by Git; absent before the first commit.
    pub head: Option<String>,
    /// Observed full symbolic local ref; absent for detached HEAD.
    pub branch: Option<std::ffi::OsString>,
    /// Raw merge parent metadata, if a merge is in progress.
    pub merge_heads: Option<Vec<u8>>,
    /// Observed worktree-specific rebase metadata, if present.
    pub rebase: Option<RebaseState>,
}

/// A local branch with its exact filesystem encoding and observed target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBranch {
    /// Local name without the refs/heads prefix, in native encoding.
    pub name: std::ffi::OsString,
    /// Observed full object ID of the branch tip.
    pub oid: String,
    /// Whether Git identified this ref as the current branch.
    pub is_current: bool,
}

impl std::fmt::Display for LocalBranch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.name.to_string_lossy(),
            if self.is_current { " (current)" } else { "" }
        )
    }
}

/// A configured remote and the explicit branch used for pull/push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    /// Configured remote name in native encoding.
    pub name: std::ffi::OsString,
    /// Configured fetch endpoints in their original encoding.
    pub fetch_urls: Vec<std::ffi::OsString>,
    /// Effective push endpoints in their original encoding.
    pub push_urls: Vec<std::ffi::OsString>,
    /// None for detached/unborn HEAD or when the branch tracks another remote.
    pub target_branch: Option<std::ffi::OsString>,
}

impl std::fmt::Display for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name.to_string_lossy())
    }
}

/// Explicit user-requested remote action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteAction {
    /// Fetch refs and objects from the explicitly selected remote.
    Fetch,
    /// Fetch and fast-forward the explicitly selected branch.
    Pull,
    /// Publish the explicitly selected local branch without force.
    Push,
}

/// Endpoint edited in a configured remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteUrlKind {
    /// Configure the fetch endpoint.
    Fetch,
    /// Configure the push endpoint.
    Push,
}

/// A saved change identified by immutable object ID rather than stash@{index}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    /// Full stash commit object ID used for selection.
    pub oid: String,
    /// Display subject reported by Git.
    pub subject: String,
}

impl std::fmt::Display for StashEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} — {}",
            self.oid.get(..7).unwrap_or(&self.oid),
            self.subject
        )
    }
}

/// Current file contents explicitly reviewed before staging a conflict resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictContent {
    /// Reviewed text bytes and their executable mode.
    Text {
        /// Complete reviewed contents retained for freshness checking.
        bytes: Vec<u8>,
        /// Whether the reviewed file has executable mode.
        executable: bool,
    },
    /// The reviewed working-tree file is absent.
    Deleted,
}

/// A review binds a conflict to its repository, index, marker configuration and file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictReview {
    /// Repository containing the reviewed path.
    pub repository: Repository,
    /// Exact porcelain record selected for review.
    pub change: FileChange,
    /// Index, HEAD and sequencer observation bound to this review.
    pub basis: CommitBasis,
    /// Contents or deletion explicitly reviewed by the user.
    pub content: ConflictContent,
    /// Configured width of conflict markers.
    pub marker_size: usize,
    /// Number of remaining marker-like lines in the reviewed contents.
    pub marker_count: usize,
}

/// Git index side, whose meaning follows Git also during a rebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictSide {
    /// Index stage 2; during rebase this is the rebased-onto side.
    Ours,
    /// Index stage 3; during rebase this is the replayed commit side.
    Theirs,
}

/// Bounded display data from one immutable side of an unmerged index entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictSideContent {
    /// Lossy UTF-8 text for display only; never a write path or resolution token.
    Text {
        /// Up to 64 KiB raw input converted for display.
        preview: String,
        /// Whether raw contents exceeded the displayed prefix.
        truncated: bool,
    },
    /// Binary data is identified without rendering arbitrary bytes as text.
    Binary {
        /// Size of the complete bounded blob in bytes.
        size: usize,
    },
    /// The selected index side has no entry, representing deletion.
    Deleted,
}

/// Read-only index-side preview. This is not authorization to overwrite a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictSidePreview {
    /// Repository whose index was inspected.
    pub repository: Repository,
    /// Exact selected porcelain record.
    pub change: FileChange,
    /// Git index side inspected.
    pub side: ConflictSide,
    /// Full immutable blob ID, or None for deletion.
    pub oid: Option<String>,
    /// Whether the selected regular-file entry is executable.
    pub executable: bool,
    /// Bounded display content or deletion metadata.
    pub content: ConflictSideContent,
}

/// Expected shape of merging an observed local branch into the current branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeKind {
    /// The target is already reachable from the current HEAD.
    AlreadyUpToDate,
    /// The source can advance directly to the target.
    FastForward,
    /// Diverged histories have a clean predicted merge tree.
    CleanMerge,
    /// The merge preview reports conflicting paths.
    Conflicts,
}

/// Immutable merge inputs and result computed without changing HEAD or index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePreview {
    /// Repository where the preview was computed.
    pub repository: Repository,
    /// Observed branch selected for merging.
    pub target: LocalBranch,
    /// Source state that must remain fresh before execution.
    pub basis: CommitBasis,
    /// Predicted merge relationship and conflict outcome.
    pub kind: MergeKind,
    /// Predicted merge tree object ID when available.
    pub tree_oid: Option<String>,
    /// Relative paths reported as conflicting by the preview.
    pub conflicts: Vec<PathBuf>,
}

/// A linear commit selected for replay in a non-interactive rebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseCommit {
    /// Full commit object ID selected for replay.
    pub oid: String,
    /// Display subject of the candidate commit.
    pub subject: String,
}

/// Relationship between the observed source and rebase target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseKind {
    /// The source already contains the target history.
    AlreadyUpToDate,
    /// The source can advance directly to the target.
    FastForward,
    /// Linear source commits must be replayed onto the target.
    Replay,
}

/// The exact source/target and candidate commits reviewed before rewriting history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePreview {
    /// Repository where candidates were inspected.
    pub repository: Repository,
    /// Observed target branch and object ID.
    pub target: LocalBranch,
    /// Source state that must remain fresh before execution.
    pub basis: CommitBasis,
    /// Whether execution is a no-op, fast-forward or replay.
    pub kind: RebaseKind,
    /// Candidate commit previews in replay order.
    pub commits: Vec<RebaseCommit>,
}

/// Persisted Git sequencer state, included in write freshness checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseState {
    /// Original full local branch ref in native encoding.
    pub branch: std::ffi::OsString,
    /// Full original branch tip before replay.
    pub original_head: String,
    /// Full target object ID onto which commits are replayed.
    pub onto: String,
    /// Current sequencer step reported by Git.
    pub step: usize,
    /// Total sequencer steps reported by Git.
    pub total: usize,
    /// Whether the observed metadata matches supported plain-pick continuation.
    pub supported: bool,
    /// Observed commit tokens from done and pending pick commands.
    pub commits: Vec<String>,
    /// Named raw metadata files used to detect stale sequencer state.
    pub metadata: Vec<(String, Vec<u8>)>,
}

/// Read-only commit metadata, addressed by full object ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    /// Full commit object ID anchoring further reads.
    pub oid: String,
    /// Display subject from the commit message.
    pub subject: String,
    /// Display author name reported by Git.
    pub author: String,
    /// Authored timestamp in Git ISO 8601 form.
    pub authored_at: String,
}

/// One committed path change relative to the first parent (or the empty root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFile {
    /// Current relative path in the selected commit.
    pub path: PathBuf,
    /// Previous relative path for a committed rename or copy.
    pub original_path: Option<PathBuf>,
    /// Change kind reported by Git name-status output.
    pub status: char,
}

/// A commit with its message, parents and first-parent path changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetails {
    /// Summary of the selected commit.
    pub commit: CommitSummary,
    /// Full parent IDs in commit order; empty for a root commit.
    pub parents: Vec<String>,
    /// Message preview, subject to the adapter display limit.
    pub message: String,
    /// Changes against the first parent or empty root tree.
    pub files: Vec<CommitFile>,
}
