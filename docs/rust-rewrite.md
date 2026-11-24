# Rust rewrite

The fork targets full behavioral parity with stable GitHub Desktop releases,
starting with Linux x86_64. Iced is the Rust UI; Git CLI remains the Git engine.
The Electron sources are reference material, not the implementation to merge into Rust.

## Running

Install a stable Rust toolchain and Git, then run `cargo run -p desktop-rust`.
The preview opens a local repository and displays status and recent history.
Select a changed file to inspect staged and working-tree differences separately.
Untracked text files have a preview up to 1 MiB; binary files are identified,
and symbolic links show their target rather than following it. Refresh reloads
status and history. Paths retain their filesystem encoding on Unix.
Repository operations run on a worker thread. Stage and Unstage act on a single
file and preserve working-tree contents when unstaging. Enter a summary and optional
description to commit **all staged changes**, including files staged outside the app.
Commit respects Git identity, hooks and signing configuration. Hook failures keep
the draft message. Successful writes automatically refresh status and history.
Stage/unstage reject stale status entries and unresolved conflicts. Before commit,
the app checks the captured index entries, HEAD and branch; changes require refresh.
Writes within this process are serialized; external clients can still race between
the freshness check and Git acquiring its mandatory locks. Stage uses current file
contents, so edits made after previewing a diff must be reviewed before staging.
Editing the repository path invalidates the loaded write target. Full-file staging
is supported; hunk selection and interactive hook/signing prompts remain planned.
The branch controls list local branches, show the current branch (or detached HEAD),
and create-and-switch or switch to an existing branch. New branch names are checked
as literal refs: options and previous-checkout shorthand are rejected. Switching
requires an unchanged source index/HEAD and an unchanged target branch commit.
Git rejects overwrites of local tracked or untracked files and branches used by a
linked worktree. Compatible local edits are carried across normally. No force,
automatic stash or remote branch guessing is used. Before the first commit,
create-and-switch sets the initial branch name; no branch ref exists until commit.
Commit drafts survive branch changes; a failed creation also retains the branch name.

## Rename the current branch

In Branches, enter a new name in New name for the current branch and choose
Rename current branch. This renames only the observed, committed local branch;
Git moves its reflog and local branch configuration. The commit ID, staged and
unstaged changes and untracked files are preserved. Tracking keeps its existing
remote destination: the remote branch is not renamed or deleted, and no network
request is made. Refresh then shows the new name and updated branch list.

The action rejects detached/unborn HEAD, active merge/rebase/cherry-pick/revert
operations, changed index/HEAD/branch observations, occupied or invalid names,
and destinations with an existing local branch configuration section. A branch
also checked out in another worktree is rejected; renaming a branch used only by
the current linked worktree is supported. Git uses its mandatory ref locks, with
no forced overwrite. Preflight checks do not fence concurrent external clients;
if Git fails after changing a ref, refresh shows the actual resulting state.

The rename field is separate from branch creation. Failures retain it, successful
renames clear only that field, and changing repositories clears it. Commit and
branch-creation drafts survive the rename. Integration tests cover tracking,
reflog, index/file preservation, stale observations, destination collisions,
sequencer state, linked worktrees and native non-UTF-8 source names on Unix.

This is a prototype under the existing `branches` feature in the upstream ledger.
Renaming a different selected branch, deleting branches and case-only renames
requiring a forced retry remain planned. The reference implementation in
`app/src/lib/git/branch.ts` supports a wider scope; this increment does not claim
complete branch parity or advance release checkpoints. The input accepts a UTF-8
new name, while an existing source name retains its native filesystem encoding.
The updated resting Branches panel has been captured and visually inspected on
Linux with the software renderer at 960×640 and 1200×800. Interactive keyboard
and end-to-end UI rename checks remain outstanding.

## Commit history

Switch between Changes and History to inspect commits without checking them out.
History lists 100 commits per page, with Load older commits anchored to the observed
HEAD object ID. Moving HEAD outside the app does not shift an in-progress page;
Open / Refresh reloads the list from the new tip. Selecting a commit shows its full
ID, author, authored date, message and changed files. Selecting a file shows its
committed diff, independent of current staged/working-tree edits.

Root commits are compared with an empty tree; merge commits use their first parent.
Renames retain both paths, including non-UTF-8 paths on Unix. External diff and
textconv helpers are disabled. Commit message previews are limited to 64 KiB and
committed diff previews to 256 KiB. These are display limits; Git output is currently
read into memory before preview truncation. Large-repository virtualization and
streamed historical diffs remain planned. Commit controls are hidden in History;
switch to Changes to prepare a new commit. Resting layouts have been visually
checked; interactive history navigation QA remains outstanding.

## Development checks

The Rust app uses Cargo. The inherited source is also checked with the repository's
Prettier and ESLint configuration. Install those tools from the existing lockfile:

```sh
npm ci --ignore-scripts --no-audit --no-fund
npm run lint:fix
npm run lint
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

The `--ignore-scripts` install is sufficient for lint and avoids Electron downloads,
application dependency installation and native builds. It does not set up the
original Electron app; follow its development guide for that. `target/` and generated
`sync-output/` reports are excluded from Prettier. The fork CI checks npm lint in a
separate job using the same install command.

## Rebase linear local history

Select a local branch, then Preview rebase to review the candidate commits and the
exact target commit. Already-based history is left alone, a direct ancestor can
fast-forward, and divergent linear commits are replayed with new IDs. The source
must be a committed local branch with a clean working tree. Source merge commits
and unrelated histories are rejected rather than flattened implicitly. The preview
lists up to 100 subjects but checks the full candidate sequence before execution.

Rebase reviewed commits uses the merge backend with `ort`, no fork-point guessing,
autosquash, automatic stash or updates to other refs. Originally empty commits and
commits whose patch is already present are kept. It does not force-push rewritten
history. Normal Git identity, signing settings and hooks remain in effect. Preview
is a commit-list review, not a prediction that every replay will be conflict-free.

A paused rebase is detected from Git's worktree-specific metadata after refresh or
reopening. Resolve text conflicts using Review conflict and Mark resolved, then
Continue rebase. The ordinary Commit action is blocked while rebasing. Continue
preserves the original messages without launching a terminal editor. Abort rebase
requires confirmation, returns to the original branch and discards resolution edits.
Original branch movement, stale index/HEAD/sequencer state and ignored or untracked
checkout collisions stop the action. Collision checks also include intermediate
replayed commits and are repeated before continue/abort.

Only plain pick sequences on the merge backend are supported for continuation.
Apply-backend rebases, custom todo commands, automatic stash and automatic other-ref
updates must be managed with Git. Interactive editing, skip and rebase-merges remain
planned. Preflight checks do not fence concurrent external clients or filesystem
changes. UI state is tested; interactive rebase and conflict UI QA remains outstanding.

## Merge local branches

Select a local branch and use Preview merge. The preview distinguishes already
up-to-date history, fast-forward, a clean divergent merge and predicted conflicts.
A committed local branch and a clean working tree (including untracked files) are
required. Preview checks do not change HEAD, index or project files; divergent
checks use `git merge-tree --write-tree`, which writes Git objects and can invoke
configured low-level merge drivers. Git with merge-tree support (2.38+) is needed.

Merge reviewed branch rechecks the observed source, target commit and preview tree.
It merges the exact reviewed commit with the `ort` strategy, disables automatic
stash/squash behavior and protects ignored local files. Per-branch mergeOptions
are overridden so they cannot silently select a different strategy. Fast-forwards
finish immediately; divergent merges stop before commit for inspection. Review the
staged changes, resolve any conflicts, then use Complete merge commit. The default
merge message fills an empty draft; an existing draft is preserved. A divergent
merge with an unchanged tree can still be committed with both parents.

The current merge survives reopening the repository. Its parent metadata is part
of the commit freshness check, including in linked worktrees. Other merges, branch
switching and stash operations are blocked until it is finished or aborted. Abort
merge requires confirmation because it discards the merge result and resolution
edits. Git abort preserves saved stash entries and unrelated untracked files; it
is not an undo for a completed fast-forward or merge commit.

Unrelated histories and detached/unborn HEAD are rejected. Previews are not a
transaction against external Git clients or filesystem changes. Interactive rebase,
squash merge and cherry-pick workflows remain planned; interactive merge UI QA is outstanding.

## Review and mark conflicts resolved

Conflicted files show Review conflict instead of ordinary Stage/Unstage. Edit the
file in your editor, then review its current contents in the app. The preview shows
up to 64 KiB, while the freshness check covers the full file up to 8 MiB. Marker
checks include start/separator/end lines and diff3 base lines, using the effective
`conflict-marker-size` attribute. Marker-like lines are treated conservatively;
files containing intentional marker examples may require staging with Git instead.

Mark reviewed file resolved stages only that file after rechecking its contents,
executable mode, index, HEAD and marker settings. Locally deleted files can be
reviewed and staged as a deletion. The action does not rewrite files, choose ours
or theirs, or create a commit. Review again after any relevant change. Once all
conflicts are resolved, use the existing commit form; a merge started outside the
app keeps its merge parents. A stash conflict can be resolved and committed while
the saved stash remains available.

Binary files, symlinks, submodules and files above 8 MiB still need an external Git
tool. The UI does not yet edit conflict blocks or offer interactive rebase.
The checks are preflight validation, not a transaction against concurrent external
filesystem edits. Visual QA and an integrated conflict editor remain outstanding.

## Saved changes

Open Saved changes to save staged and unstaged tracked changes. Staged new files
are included; unstaged new files remain in the working tree. An optional description
labels the stash. The first commit must already exist. Restore requires a clean
working tree, restores the staged/unstaged split and **keeps the saved stash**.
Entries are selected by object ID, so new stashes do not change what gets restored.
Stashes created outside the app are listed too, including entries with untracked
files. Ignored-file and symlink-directory collisions are checked before restore.

Existing work, stale index/HEAD or removed stash entries stop restore. If Git reports
conflicts, the app refreshes status and retains the stash. Edit conflicted text
files, review them in the app and mark them resolved before committing the result.
Automatic conflict editing, stash deletion and pop are not implemented yet. UI tests verify state transitions; the resting Saved changes panel has been visually checked,
but interactive save/restore QA remains outstanding. Stash operations share the process write lock with other
local writes and do not fence concurrent external edits.

## Create a local repository

Enter a project folder in the Create repository form and optionally an initial
branch name (default `main`). The parent must already exist. The app can create a
new folder or initialize an existing folder containing ordinary project files.
Project files are preserved and left untracked; no README, ignore file, license or
initial commit is generated automatically. After creation the repository opens in
the main view: review changes, stage the files and enter the first commit message.

Existing repositories, bare repositories, linked-worktree gitfiles, destinations
inside an existing repository and symlink destinations are rejected. Branch names
are validated before creating folders or metadata. Existing `.git` markers are
never reused, even if malformed or broken. Failures preserve the open repository
and form inputs; newly created folders or metadata may remain for inspection.
Concurrent external filesystem changes are not a transaction with this preflight.
Git commands ignore inherited repository-routing variables such as `GIT_DIR`,
`GIT_WORK_TREE` and `GIT_INDEX_FILE`, while keeping normal identity, helper and SSH
configuration. Initialization is tested end-to-end through the first staged commit;
the resting Create / Clone panel has been visually checked, but interactive UI QA remains outstanding.

## Clone and remotes

Clone accepts a Git URL or local source and a new destination folder whose parent
already exists. Existing destinations, including empty ones, are rejected. The app
reserves the folder before cloning; failed clones may leave partial files and never
trigger recursive cleanup. A successful clone opens the new repository; failure
keeps the previous repository and form inputs.

Select a configured remote to enable Fetch. Pull and Push additionally require a
committed local branch; they use its configured upstream branch, or the same local
branch name if unpublished. A branch tracking another remote cannot be pulled or
pushed via the selected remote. The remote branch target is shown before action.
Pull uses fast-forward only, without automatic stash/rebase/merge commits. Push
sends only HEAD to that branch without force, mirror or automatic tag pushes, and
sets upstream on success. Fetch leaves the working tree and local HEAD unchanged.
Remote URLs and the branch/index basis are revalidated before pull/push. These are
preflight checks, not a transaction against concurrent external Git clients.
A failed pull can still update remote-tracking references during its fetch phase.

Open Remote settings to add a configured remote (default name `origin`), or select
an existing remote to edit its fetch/push URLs. Saving the fetch URL also changes
the push destination when Git is using its default fallback; an explicit push URL
is preserved. Saving a push URL creates/updates that override. Multi-URL endpoints
must be edited using Git; the UI never silently changes only their first URL.
Settings changes are local and do not initiate a fetch or push. A newly added
remote is selected after refresh so a committed local branch can be published
with Push branch. This publishes to an existing server repository; it does not
create a repository on GitHub.

Removing a remote requires confirmation in the settings panel. Git removes its
local configuration, remote-tracking refs and associated upstream assignments;
local branches, index contents, project files and the server repository are kept.
Remote settings are revalidated before edits/removal and all local writes share
the process write lock. Failed saves preserve input drafts, while changing the
repository clears pending edits and removal confirmation. Concurrent external Git
configuration changes can still race after the preflight check.

Authentication currently uses existing Git credential helpers and SSH configuration.
Terminal credential prompts are disabled. There is no in-app GitHub OAuth or secure
credential store integration yet. Transfers stream bounded Git diagnostics into
the UI (16 KiB tail), with a Cancel
transfer button and a 15-minute deadline including time waiting for the write lock.
On Unix, Git and its transport helpers run in a dedicated process group; cancellation
sends TERM, then KILL after a short grace period, and drains output before unlocking.
Windows process-tree termination still needs a native adapter; only the direct
child is killed there. There is no percentage parser or throughput chart yet.
Cancellation is not rollback: a push may already have reached the server, a pull
may have fetched refs, and a clone may leave partial files. The app refreshes the
repository after interrupted remote actions and does not delete partial clones or
Git lockfiles. Fetch again to verify remote state before retrying an interrupted push.
Remote operations run only on explicit actions. Tests use local bare repositories; live
GitHub authentication and interactive transfer UI QA have not been performed.
The resting Remotes and Create / Clone layouts have been visually checked.
No credentials or original application databases are imported.

## UI layout and visual checks

The sidebar separates Changes, History, Branches, Remotes, Saved changes and
Create / Clone. File lists and diffs occupy the main workspace; commit fields
appear only in Changes. A merge/rebase banner keeps operation controls reachable
while resolving files. Navigation preserves drafts and revokes hidden previews
and abort/remove confirmations. Busy operations keep their current panel.

The default window is 1200×800 with a minimum of 960×640. A renderer layer clips
sidebar content, including long paths with the software renderer. Views live in
`crates/ui/src/layout.rs`; Git operations and their freshness checks are unchanged.

Developer launch options accept a repository, panel and window size:

```sh
cargo run --locked -p desktop-rust -- --repository /path/to/repo --panel history
cargo run --locked -p desktop-rust -- --repository /path/to/repo --size 960x640 --capture /tmp/fresh-window.png
```

`--capture` saves only this application's window as PNG and exits. It refuses
existing output files and reports failure if the window closes before capture.
Launch-time repository inspection is read-only and happens before window creation.
Interactive repository operations continue to use worker threads.

A repeatable offline fixture captures all six panels without touching a real
project or contacting a remote:

```sh
cargo build --locked -p desktop-rust
python3 script/rust-ui-smoke.py --output /tmp/fresh-ui-1200 --size 1200x800
python3 script/rust-ui-smoke.py --output /tmp/fresh-ui-960 --size 960x640
```

Each output directory must be new. The script requires a graphical session,
uses the software renderer unless `ICED_BACKEND` is set, creates and removes a
temporary repository, and checks that its working files, config, status, refs and
index remain unchanged. PNGs and `report.json` are local review artifacts, not
committed baselines. Compare the images manually: capture success alone does not
establish visual correctness. Automated CI currently runs state tests, not captures.

On 2026-11-24, all six basic panels were captured and visually inspected on Linux
at 1200×800 and 960×640 using the software renderer. This covers the resting
layouts with sample changes, history, remote configuration and a stash. Keyboard
navigation, focus, IME, HiDPI, accessibility, other renderers/platforms, long-list
scrolling and complete interactive write/conflict flows remain unverified.

## Stable upstream maintenance

Run `python3 script/upstream/sync.py --output /tmp/desktop-rust-sync-new`.
Use a fresh output path for each run; existing reports are never overwritten.
Validate the committed ledger with `python3 script/upstream/registry.py`.
See [the maintenance procedure](upstream-maintenance.md) for the ledger format,
checkpoint gates, backport handling and watchdog.
The weekly workflow runs this command and uploads proposed state and a report.
Enable scheduled Actions on the fork and use manual dispatch to recover missed runs.
GitHub schedules run on the default branch; deployment of these files is required.

Review `state.json` in the artifact and commit it to `upstream/state.json` only
through a reviewed PR. Without that commit the next run intentionally repeats the
proposal. The first run selects the latest stable release as an explicit baseline.
Subsequent runs capture every unseen stable release and detect moved tags.
Analysis and parity checkpoints never advance during discovery.

For each release, inspect its full source diff, tests, dependencies and release
notes. Register behavior changes in `changes` with id, upstream commits/PR,
area, status, Rust PR and regression test evidence. Allowed statuses are
pending-analysis, pending-implementation, deferred, implemented, not-applicable.
Implemented items require test evidence; non-applicable items require a reason.
Changes for features not yet available remain deferred against that milestone.
Advance `analyzed` only after classification; advance `parity` only after all
applicable changes for supported features have passed verification.

Port behaviors in separate PRs rather than merging TypeScript into Rust.
Prioritize data loss, credential issues and security fixes outside the weekly cycle.
Follow AGENTS.md and the prepare-team-pr skill for team-authored PR preparation.

## Delivery sequence

1. Foundations: stable discovery, workspace and CI (initial implementation here).
2. Vertical prototype: repository status, diff, history; validate Iced keyboard,
   focus, IME, HiDPI, accessibility and large repositories before expanding UI.
3. Local alpha: staging, commits, branch operations and real Git fixture tests.
4. Remote alpha: clone/fetch/pull/push, SSH, OAuth and secure OS credential storage.
5. Linux beta: conflicts, rebase, stash, cherry-pick, tags, LFS, submodules, .deb.
6. Parity: Enterprise, pull requests, multiple accounts and advanced integrations.
7. Windows/macOS: native adapters, packaging, signing and update verification.

Reserve 25% of capacity for upstream work; increase it if backlog grows over two
cycles. Publish the upstream stable baseline and known limitations with releases.
Use independent fixture copies for differential tests against the original.

## Remaining infrastructure

Before publishing: create/protect `main`, set it as default, retain an unmodified
`upstream/stable` reference, and replace official release/update infrastructure.
Automatic PR creation requires verified human ownership and the repository's
team-authored review process; discovery currently produces review artifacts only.
The daily stale-run watchdog and ledger/evidence validator are implemented locally.
They still require deployment to the default branch and enabled Actions. Scheduled
workflows cannot detect their own complete suspension; monitor Actions externally
if unattended availability is required.
The preview now supports local initialization, stage/unstage, commit, and branch
creation/switching.
Clone, fetch, fast-forward pull, push and upstream setup are implemented as a
prototype. Renaming the current committed branch is supported locally; branch
deletion, renaming other selected branches and in-app authentication remain planned.
Diff rendering is plain monospaced text; syntax highlighting, hunk selection,
large-diff virtualization and interactive conflict resolution remain planned.
Automated Git integration tests cover staged/unstaged changes, unborn branches,
rename, deletion, binary files, conflicts, literal pathspecs and non-UTF-8 paths.
Git tests also cover branch creation/switching, detached and unborn HEAD, invalid
names, stale source/target refs, linked worktrees and preservation of dirty files.
UI state tests cover draft retention, branch operation results, busy controls,
write-target invalidation and failed refresh.
The six basic Iced panels have had Linux visual layout checks at two window sizes.
Interactive workflow QA and accessibility testing remain outstanding; see the UI checks above.
