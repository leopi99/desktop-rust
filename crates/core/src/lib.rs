//! UI-independent repository models.
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    pub repository: Repository,
    pub status: String,
    pub history: String,
    pub commits: Vec<CommitSummary>,
    pub changes: Vec<FileChange>,
    pub branches: Vec<LocalBranch>,
    pub remotes: Vec<Remote>,
    pub stashes: Vec<StashEntry>,
    pub merge_message: Option<String>,
    /// Index and HEAD captured for the pre-commit freshness check.
    pub commit_basis: CommitBasis,
}

/// A porcelain status entry; filesystem paths retain their original encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
    pub index: char,
    pub worktree: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub staged: String,
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
    pub index: Vec<u8>,
    pub head: Option<String>,
    pub branch: Option<std::ffi::OsString>,
    pub merge_heads: Option<Vec<u8>>,
    pub rebase: Option<RebaseState>,
}

/// A local branch with its exact filesystem encoding and observed target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBranch {
    pub name: std::ffi::OsString,
    pub oid: String,
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
    pub name: std::ffi::OsString,
    pub fetch_urls: Vec<std::ffi::OsString>,
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
    Fetch,
    Pull,
    Push,
}

/// Endpoint edited in a configured remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteUrlKind {
    Fetch,
    Push,
}

/// A saved change identified by immutable object ID rather than stash@{index}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StashEntry {
    pub oid: String,
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
    Text { bytes: Vec<u8>, executable: bool },
    Deleted,
}

/// A review binds a conflict to its repository, index, marker configuration and file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictReview {
    pub repository: Repository,
    pub change: FileChange,
    pub basis: CommitBasis,
    pub content: ConflictContent,
    pub marker_size: usize,
    pub marker_count: usize,
}

/// Expected shape of merging an observed local branch into the current branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeKind {
    AlreadyUpToDate,
    FastForward,
    CleanMerge,
    Conflicts,
}

/// Immutable merge inputs and result computed without changing HEAD or index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergePreview {
    pub repository: Repository,
    pub target: LocalBranch,
    pub basis: CommitBasis,
    pub kind: MergeKind,
    pub tree_oid: Option<String>,
    pub conflicts: Vec<PathBuf>,
}

/// A linear commit selected for replay in a non-interactive rebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseCommit {
    pub oid: String,
    pub subject: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebaseKind {
    AlreadyUpToDate,
    FastForward,
    Replay,
}

/// The exact source/target and candidate commits reviewed before rewriting history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePreview {
    pub repository: Repository,
    pub target: LocalBranch,
    pub basis: CommitBasis,
    pub kind: RebaseKind,
    pub commits: Vec<RebaseCommit>,
}

/// Persisted Git sequencer state, included in write freshness checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseState {
    pub branch: std::ffi::OsString,
    pub original_head: String,
    pub onto: String,
    pub step: usize,
    pub total: usize,
    pub supported: bool,
    pub commits: Vec<String>,
    pub metadata: Vec<(String, Vec<u8>)>,
}

/// Read-only commit metadata, addressed by full object ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub oid: String,
    pub subject: String,
    pub author: String,
    pub authored_at: String,
}

/// One committed path change relative to the first parent (or the empty root).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFile {
    pub path: PathBuf,
    pub original_path: Option<PathBuf>,
    pub status: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetails {
    pub commit: CommitSummary,
    pub parents: Vec<String>,
    pub message: String,
    pub files: Vec<CommitFile>,
}
