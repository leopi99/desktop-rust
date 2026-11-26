# Rust rewrite

The fork targets full behavioral parity with stable GitHub Desktop releases,
starting with Linux x86_64. Iced is the Rust UI; Git CLI remains the Git engine.
The Electron sources are reference material, not the implementation to merge into Rust.

## Project tracking and Rust rules

Use [status](rust/status.md) for completed/prototype work,
[the backlog](rust/backlog.md) for ordered remaining tasks and batch acceptance,
and [the Rust quality standard](rust/quality.md) for required engineering checks.
Rust-specific agent instructions are in [crates/AGENTS.md](../crates/AGENTS.md).

## Running

Install a stable Rust toolchain and Git, then run `cargo run -p desktop-rust`.
The preview opens a local repository and displays status and recent history.
Select a changed file to inspect staged and working-tree differences separately.
Tracked staged and unstaged patch previews each retain at most 256 KiB of raw
output, with an explicit marker when clipped. On Unix each patch subprocess has
a 30-second deadline and a 16 KiB stderr cap; excess stdout is drained/discarded,
and the final exit status is checked before displaying the prefix. UTF-8
replacement can enlarge the displayed byte size. This limits parent capture,
not Git's own memory. Windows still buffers the full patch before clipping and
has no equivalent patch deadline (PLAT-01). The preceding status read has its
own 8 MiB capture cap and Unix deadline. These separate limits are not one
deadline for the entire `diff` operation. Selecting Stage still stages the complete current file.
Regular untracked files up to 1 MiB can be previewed; binary contents within that
bound are identified without text rendering. The read itself is capped at the
limit plus one overflow probe byte, including when the file grows after metadata
inspection. Oversized/growing files have no accepted partial prefix and retain
full-file staging. Symbolic links show their target spelling without reading its
contents. Refresh reloads status and history. Paths retain their native encoding.
Preview requests from a repository subdirectory resolve root-relative selected
paths against Git's actual working-tree root, including root names with newlines.

Untracked preview and conflict/hunk working-content capture share the bounded
filesystem reader. Conflict/hunk capture retains its 8 MiB limit. Ancestor symlinks
and non-directories are refused; Unix final-component opens use no-follow and
nonblocking flags, with regular-file descriptor checks before reading. Ancestors
are rechecked after opening, but these checks do not fence external renames.
Windows retains best-effort metadata/type checks without the Unix leaf guarantee.
The byte bounds do not impose a deadline on regular/network filesystem I/O; they
are separate from the bounded Git subprocess reads described above.
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
is supported; the scoped hunk-staging workflow below is also available. Line-level
selection and interactive hook/signing prompts remain planned.
The branch controls list local branches, show the current branch (or detached HEAD),
and create-and-switch or switch to an existing branch. New branch names are checked
as literal refs: options and previous-checkout shorthand are rejected. Switching
requires an unchanged source index/HEAD and an unchanged target branch commit.
Git rejects overwrites of local tracked or untracked files and branches used by a
linked worktree. Compatible local edits are carried across normally. No force,
automatic stash or remote branch guessing is used. Before the first commit,
create-and-switch sets the initial branch name; no branch ref exists until commit.
Commit drafts survive branch changes; a failed creation also retains the branch name.

## Stage or unstage selected hunks

For a regular text modification, addition or deletion in Changes, choose Review
unstaged hunks.
The app reads a complete, separate patch and displays context-bearing blocks with
Stage hunk buttons. Each action adds just that block to the index, keeps the working
file intact and preserves already staged changes and other files. Commit still
commits the entire index, so review its contents normally. To stage another block,
review again after the automatic refresh. Close hunk review discards the choice.
Use Include hunk checkboxes to select several blocks in the same file, then choose
Stage selected or Unstage selected below the scrolling review. The action shows
the selection count and is disabled for an empty selection. Individual Stage hunk
and Unstage hunk buttons still act only on their named block. Either action consumes
the review and clears the checkbox selection; review again after refresh.
The ordinary, potentially clipped diff preview cannot authorize a partial write.

For a staged text change, choose Review staged hunks and then Unstage hunk.
This reverses only that block from the HEAD-to-index comparison; it never restores
HEAD into the working file. Other staged blocks and additional working-only edits
remain intact. The staged/unstaged label and action are bound to the review:
a forward-stage review cannot authorize unstaging, and the reverse is also refused.
Unstage at file level continues to reset the whole file's index entry.

This scope accepts regular UTF-8 text modifications with unchanged Git file mode,
regular additions/deletions, working edits at staged rename destinations and no
content-conversion policy. Coordinated rename changes, symlinks, submodules,
binary/non-UTF-8 data and
filters/text/EOL/ident/encoding conversion settings require full-file staging or an
external Git tool. CRLF bytes without Git conversion and no-newline markers are
preserved. Staged mode changes to existing files remain outside this scope.
Absent working files use a complete bounded source read from the index, or HEAD
for a staged deletion; source text must be UTF-8 without NUL bytes.

For a new file, review compares against the null path without inserting an
intent-to-add entry. Both HEAD and index must lack the path; recreated tracked
paths and explicit intent entries require full-file staging. Selected added rows
form the new index file, leaving all working bytes intact. This also works before
the first commit. Further working edits to a staged addition can be reviewed normally.
An addition has one complete hunk; removing some staged rows retains the index
file and its executable mode. Removing the entire hunk or all its changed rows
removes the index entry, leaving the working file untracked. Empty new files have
no text hunk and use full-file staging. Existing complete-patch and line-control
limits apply to additions as well.

For an absent deleted file, selecting minus rows stages just those line removals.
Unselected rows remain in the index with its original executable mode. The whole
hunk or all changed rows stages the entire deletion, removing the index entry.
Unstaging selected minus rows of a staged deletion restores only those rows to
the index, retaining the HEAD file mode. Review again to remove further staged
changes; staged modifications/additions can also be reviewed while the working
file remains absent. This includes additions staged before being removed from
disk. None of these actions recreates a working file. A recreated working path
invalidates a deletion review. Empty deleted files have no text hunk and use
full-file controls. Binary, special and converted sources remain unsupported.

After Git reports an already staged rename, Review unstaged hunks can select
working edits at its destination. The comparison is index-to-working at the new
path; the cached patch updates only that destination index entry. The original
path stays removed from the index, and an independently recreated working file at
the old name is preserved. Prior staged edits and the destination executable mode
stay in place. If the destination is missing, selecting deleted rows keeps the
remaining index text under the new name. Staging that entire deletion removes
the destination entry too, leaving the original path's staged deletion.

A pure rename has no unstaged text hunk. Staged-side hunk review of a rename,
partial unstaging of its content/move, working-tree renames and copies remain
outside this scope; file-level Unstage still resets both observed rename paths.
The app uses Git's observed status and does not infer a move from separate
addition/deletion records. Detection/configuration or similarity changes can
invalidate the review and require a fresh selection.

The complete patch is limited to 512 KiB and 128 hunks; working-file contents to
8 MiB, also for source blobs read when working files are absent. An oversized or
unsupported patch is refused entirely, never staged from a clipped prefix. Git path quoting is retained exactly, including native Unix names.
The parser checks a single regular-file header and each hunk's row counts. Before
writing, the app rereads the full review: comparison direction, bytes/mode, status, index/HEAD/operation
state, attributes, settings and every patch block must still match. Navigation,
selection, another preview or refresh invalidates the review. Busy controls cannot
redirect a write, and commit drafts survive successes and failures.

The API accepts a nonempty set of unique valid hunk indices, normalizes them to
patch order and rejects duplicate or out-of-range choices without staging a prefix.
Git first checks then applies one patch containing the header and the selected
context-bearing changes with apply --cached, adding --reverse only for a staged
review. It does not loop over separate index writes for each block. There is no checkout, force, three-way merge, rejected-patch output or working-file
write. Writes share the existing process lock and Git's index lock. External clients
and configured hooks can still race preflight checks. A write failure refreshes the
repository and does not automatically roll back over subsequent edits. Read bounds
follow the shared Unix policy; automatic cancellation of stalled local writes and
native Windows capture guarantees remain separate backlog items.

## Select individual changed lines

Within a hunk review, added and deleted rows have their own checkboxes when the
complete review fits 64 KiB and 1,024 changed rows. Select both minus and plus rows
to include a replacement; selecting only one means just that deletion or addition.
For example, staging only the added row of `old` → `new` retains `old` and adds
`new` after it; staging only the deleted row removes `old` without adding `new`.
The same rows in a staged review identify the changes to remove from the index.
Context rows and hunk headers cannot be selected. Within a contiguous change run,
retained old rows precede selected additions; choose the full run when the entire
replacement order matters.

Whole hunks and individual rows can be combined in one action. Choosing a whole
hunk replaces its individual choices; unchecking one of its checked changed rows
refines that hunk into the remaining individual rows. The fixed footer then shows
Stage selected lines or Unstage selected lines and the total number of changed
rows, including any selected whole hunks. Single-hunk buttons still act only on
the named hunk. All selections clear with review invalidation or a write; commit
drafts stay intact.

The app rebuilds context and counts for the selected direction: unselected deleted
rows become context for staging, and unselected added rows become context for
unstaging. The opposite unselected rows are omitted. Counts and offsets across
hunks are checked before the same cached check/apply workflow runs. No working-file
rewrite, global index reset, three-way fallback or automatic rollback is introduced.
The complete review, native quoted paths and freshness guards remain unchanged.

Missing-final-newline markers do not block line controls. The reconstructed old
and new streams are checked independently: once a stream reaches an unterminated
row, no later row may belong to that stream. This permits partial edits before an
unchanged unterminated context line and valid EOF deletions without adding bytes.
A selection that would append after a missing newline is refused before index
mutation; choose the adjoining EOF changes together or the whole hunk instead.
For example, adding `new` after retained unterminated `old` would require an
implicit separator and is refused. Selecting their complete replacement preserves
its exact bytes. Newline-only changes and transitions to/from empty contents are
supported. CRLF without Git conversion remains supported. Larger reviews retain
whole-hunk controls up to their original limits.
Line selection applies to the supported modifications, additions, deletions and
staged rename destinations described above. Coordinated rename operations,
special/binary formats, conversion policies and full interactive/platform parity
remain outside this scope.

## Create a branch from history

In History, select a commit, enter New branch name and choose Create branch at
selected commit. The full destination commit ID stays visible above the form,
including while inspecting one of its file diffs. This creates a local reference
without checking it out; the current branch or detached HEAD, staged/unstaged
contents, untracked files and commit drafts remain in place. No upstream tracking
is configured automatically, even with automatic tracking enabled in Git settings.
Use Branches to switch to the created branch separately.

Only a full commit object ID is accepted by the Git API: symbolic revisions,
abbreviated hashes, missing objects, trees, blobs and tag objects are refused.
The UI requires a loaded selection from the observed history list, an idle History
panel and a name. Index/HEAD/branch freshness, active merge/rebase/cherry-pick/revert
state, occupied destination refs and saved local destination configuration are
checked before creation. Git owns ref locks; concurrent external clients can still
race the preflight checks. No force, checkout, reset or remote write is requested.

Success clears the shared branch-creation name; failure retains it. Both outcomes
refresh repository/history state while keeping the History panel and unrelated
drafts. Refresh invalidates commit selection, so select a commit again for another
action. The input is shared with branch creation in Branches. This scoped history
action does not implement reset, revert, cherry-pick or full upstream dialog parity.

## Rename a local branch

In Branches, enter a new name in New local branch name and choose
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

To rename another local branch, select it in the branch list, enter its new name
and choose Rename selected branch. This does not switch branches: current HEAD,
staged/unstaged contents and untracked files remain intact. The observed target
name, full object ID and current flag must still match. A branch used by any other
worktree is refused, even when Git could update that worktree's branch name.
The same destination/configuration and active-operation guards apply. The UI
requires the Branches panel, an observed non-current branch and no write in flight;
a failed action refreshes state and retains the rename draft.

The rename field is separate from branch creation. Failures retain it, successful
renames clear only that field, and changing repositories clears it. Commit and
branch-creation drafts survive the rename. Integration tests cover tracking,
reflog, index/file preservation, stale observations, destination collisions,
sequencer state, linked worktrees and native non-UTF-8 source names on Unix.

This is a prototype under the existing `branches` feature in the upstream ledger.
Current and selected-branch rename are implemented locally. Case-only renames
requiring a forced retry remain unsupported. Safe deletion of merged local branches is described below. The reference implementation in
`app/src/lib/git/branch.ts` supports a wider scope; this increment does not claim
complete branch parity or advance release checkpoints. The input accepts a UTF-8
new name, while an existing source name retains its native filesystem encoding.
The updated resting Branches panel has been captured and visually inspected on
Linux with the software renderer at 960×640 and 1200×800. Interactive keyboard
and end-to-end UI rename checks remain outstanding.

## Delete a merged local branch

In Branches, select a non-current local branch, then Delete selected branch.
Review its name and full commit ID and confirm Delete reviewed local branch.
This removes the local ref, reflog and local branch settings. It does not contact
a server or delete remote branches or remote-tracking refs. Current HEAD,
commit drafts, staged/unstaged changes and untracked files are preserved.

The selected tip must be an ancestor of the observed current HEAD. Merely being
published to its upstream is insufficient. The operation uses `git branch --delete`
without force, so Git may also refuse when the branch has commits missing from
its configured upstream even if the current branch contains them. There is no
force-delete fallback. Branches checked out in linked worktrees are rejected by Git.

Deletion requires a committed local current branch, an unchanged index/HEAD/branch
observation, and the same selected branch name and object ID. Active merge,
rebase, cherry-pick and revert operations block it. Selection changes, panel or
repository navigation, refresh and write completion revoke the confirmation.
Failures refresh the actual repository state and require a new review; unrelated
creation, rename and commit drafts are retained.

Preflight checks and the process mutex do not lock out external Git clients.
Another client can still race after the checks and before Git acquires its ref
locks. This feature does not claim atomic compare-and-delete against the reviewed
object ID or protection against concurrent external config changes. The original
TypeScript adapter supports forced deletion; that scope remains deferred under
the existing `branches` feature. Release analysis/parity checkpoints are unchanged.

Git integration tests cover merged/unmerged history, tracked upstream checks,
stale observations, worktrees, operation metadata, dirty-file preservation and
non-UTF-8 target names. UI tests cover confirmation binding, cancellation, busy
controls, invalidation and failure handling. The smoke fixture also captures a
read-only deletion review using `--panel branch-delete-review`; this developer
scenario requires `--repository` and `--capture`, selects the first non-current
branch and never sends the deletion action. The updated resting panel and
confirmation have been captured and visually inspected on Linux using the
software renderer at 960×640 and 1200×800. Interactive keyboard and complete
end-to-end UI deletion checks remain outstanding.

## Complete state and freshness scans

Core snapshot/freshness reads reject incomplete output instead of parsing a prefix.
Their raw stdout limits are:

| Read | Limit |
| --- | --- |
| Human/porcelain status (each command) | 8 MiB |
| Complete staged-index entries | 16 MiB |
| Local branch listing | 4 MiB |
| HEAD/symbolic ref, repository root and merge/rebase metadata paths | 64 KiB each |
| Remote names, tracking metadata and each fetch/push URL list | 64 KiB per read |
| Stash list and each tracked/untracked stash path stream | 4 MiB each |
| Complete tracked-path scan for stash collision checks | 16 MiB |
| Branch rename local-config and worktree guards | 4 MiB each |
| Conflict attribute / rebase branch-tip / common ancestor | 64 KiB each |
| Rebase plan and each collision path stream | 4 MiB each |
| Rebase collision paths across all streams | 8 MiB total |
| Complete tracked-path scan for rebase collisions | 16 MiB |

On Unix each migrated subprocess has a 30-second deadline and a 16 KiB stderr cap,
using managed process-group cleanup and joined readers. Windows validates these
limits after whole-output buffering and still lacks equivalent memory/deadline
bounds. Limits cover captured bytes, not Git's internal memory, parsed-model
allocations or one overall snapshot deadline.

HEAD and branch reads propagate failures. Only a quiet, empty exit 1 can denote
absence for the selected Git commands. A missing HEAD commit is considered unborn
only when a symbolic HEAD exists and its target ref is absent; malformed refs,
missing commit objects and detached HEAD without a valid commit fail. Full native
ref names and full commit IDs remain part of the freshness observation. These
sequential reads do not fence concurrent external clients.

Regression fixtures exceed the index/status and branch-list caps, verify refused
commit/staging with unchanged index/HEAD/working files, and exercise recovery after
restoring only the disposable fixture. The remote model accepts at most 64 remotes and 64 URLs per endpoint. New remote
names/URLs are checked against the known metadata budgets before a write; external
configuration and Git URL rewriting can still cause later reads to be rejected.
Stash restore reviews at most 10,000 path records across the two path streams and
loads the full tracked-path set once. Third-parent lookup failures propagate;
only the selected quiet absence result can mean no saved untracked tree.

Auxiliary regressions exercise excessive remote/URL counts, oversized endpoint,
stash and local-config output, and stash collision review above 10,000 paths.
They verify rejected writes, retained config/index/working data/stashes and recovery.
These are per-subprocess byte/deadline limits and a bounded review count, not one
operation-wide deadline; filesystem collision checks have no separate deadline.
Conflict attributes and rebase preflight queries now use bounded complete reads.
Shared ancestry queries require empty stdout and accept only a quiet exit 1 for
non-ancestry; common-base output must contain a full object ID. The generic
whole-output reader has been removed. Merge-tree now retains complete raw output
within 4 MiB stdout and 64 KiB stderr; overflow fails after draining and joining,
without parsing a prefix or interrupting a configured driver. This object-writing
preview has no automatic deadline. Rebase uses marked 64 KiB per-stream diagnostic
tails, preserving final exit status and paused-state detection. Windows retains
explicit unbounded-capture fallbacks for read-only commands and the runtime probe.

## Git pipe-reader lifecycle

Transfers, the shared local-write runner and bounded Unix reads share a reader-pair
owner. Both pipe threads
start through fallible builders; failure to start either reader stops/reaps the
managed child and waits for any started reader. At completion both workers are
joined before a reader panic or I/O error is returned. Transfers retain their
64 KiB per-stream diagnostic tails and 16 KiB live progress limit. Linux tests
cover inherited pipes after Git exits, timeout/cancellation and a subsequent retry.
Every Unix Git subprocess now uses the shared child/reader lifecycle. Windows
still needs native process-tree/inherited-pipe cleanup and bounded read/probe
fallbacks (PLAT-01). Remaining stalled-write policy is tracked as R-06 in
[the backlog](rust/backlog.md).

The shared local-write runner starts both output readers before feeding a commit
message to stdin, then closes stdin, waits for Git and joins both readers. Each
stream retains only its last 64 KiB of raw bytes with a marker for discarded
output; UTF-8 replacement may enlarge the displayed string. Successful calls keep
their stdout-only diagnostic contract. Git failure status and both diagnostic
tails take precedence over a secondary stdin failure; an incomplete input write
still fails even if the child exits successfully. Real Git redirects hook stdout
to stderr, so those messages appear in the failure diagnostics.

No automatic timeout or cancellation is imposed on local-write hooks/signing.
A stalled child or inherited pipe can still keep the operation busy; interruption
and recovery policy remains pending. I/O/setup errors do not promise rollback of
an already started Git command. Existing freshness checks and repository refresh
remain necessary. Merge-tree and rebase now share the same lifecycle with
complete-protocol and diagnostic-tail policies respectively. Linux tests use real noisy hooks, large
messages and staged/unstaged files, and verify retry after a rejected hook. The
Linux pipe-deadlock regression uses coreutils `timeout` to bound the test itself.
Additional regressions verify complete raw protocol bytes at exact limits, final
exit status, overflow on either stream only after child completion, and successful
retry. A noisy real merge driver makes preview fail without changing HEAD, index
or working files; a noisy rebase driver still produces a supported paused state
that can be aborted back to the original branch and contents.

## Git runtime diagnostics

The Git panel shows the version reported by the system Git resolved through PATH.
The app checks it on startup using a worker thread; Recheck Git repeats the
read-only probe. It works without an open repository. A missing Git executable
produces an installation/PATH message instead of hiding the engine failure.
Version vendor/development suffixes are retained. Runtime results do not unlock,
finish or redirect repository writes and do not discard drafts or snapshots.

The preview uses externally installed Git; it does not package or update Git and
does not infer compatibility or security from the reported version. Later commands
resolve the executable again, so a PATH/binary change can invalidate the observation.
The diagnostic is not a prerequisite or capability gate for repository commands.
On Unix the probe has a five-second deadline with bounded 512-byte stdout and
1024-byte stderr capture. Oversized output fails instead of being accepted as a
truncated version; raw bytes are validated before display. A timeout terminates
the isolated process group, waits for the direct child and joins both readers
before retry becomes available. Cleanup includes a short termination grace period.
This Unix implementation is tested on Linux; the Windows fallback still buffers
output and requires native process-tree/pipe handling before equivalent bounds
can be enabled there.
See [the first baseline triage](upstream/release-3.6.11.md) for the distinction
between upstream's Git 2.56.1 bundle and the current Rust engine policy.

Capture the diagnostic with `--panel git`. The offline smoke fixture includes that
panel and no-repository captures with Git absent from PATH, plus Unix stalled/noisy
Git shims, in addition to the existing panels and read-only deletion review. The
timeout and overflow states were also visually inspected at both window sizes.
The available/missing diagnostic states
were captured and visually inspected on Linux/software rendering at 960×640 and
1200×800 on 2026-11-24; the fixture files, refs, index and config were unchanged.
Interactive keyboard and accessibility checks remain outstanding.

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
committed diff previews to 256 KiB of raw output, plus a truncation marker; UTF-8
replacement during display can increase the rendered byte size.

On Unix, each historical Git child has a 30-second deadline and a 16 KiB stderr
cap. Complete metadata is limited to 1 MiB per page/detail read; changed-file lists
to 4 MiB and 10,000 files. Oversized structured results fail rather than returning
an incomplete list. Message/patch readers retain only the prefix and drain/discard
the rest until EOF, checking the final exit status before displaying a preview.
Selecting a diff does not reread the commit body. These bounds cover captured
output and command lifetime, not Git's own internal memory. Process-group cleanup
also covers inherited pipes; this path is tested on Linux.

Windows still buffers whole output before checking metadata/display limits and
has no equivalent per-read deadline (PLAT-01). Large-repository virtualization
remains planned. Commit controls are hidden in History; switch to Changes to
prepare a new commit. Resting layouts have been visually checked; interactive
history navigation QA remains outstanding.

## UI transition modules

`crates/ui/src/update/mod.rs` exhaustively routes the application's 91 message
variants to typed events in ten domain handlers: application/navigation,
repository setup, history, changes/conflicts, branches, merge, rebase, remotes,
stashes and completion/refresh. Guard failures explicitly match the relevant
events; adding an unhandled variant fails compilation. Worker callbacks continue
to return application messages. Successful large response payloads are boxed at
the internal routing boundary without cloning their data.

Domain helpers live with their handlers. Completion/refresh handling centralizes
draft updates, review invalidation and snapshot refresh. The existing 82 state
tests cover busy guards, drafts, confirmations, failures and transfer freshness.
The refactor's offline smoke run produced eleven captures at 960×640 with unchanged
fixture data; this does not prove full interactive QA. The branch deletion-review
capture exposes right-edge button clipping tracked as UI-03.

## Development checks

The Rust app uses Cargo. The inherited source is also checked with the repository's
Prettier and ESLint configuration. Install those tools from the existing lockfile:

```sh
npm ci --ignore-scripts --no-audit --no-fund
npm run lint:fix
npm run lint
python3 script/check_rust.py
# With dependencies already cached:
python3 script/check_rust.py --offline
```

The `--ignore-scripts` install is sufficient for lint and avoids Electron downloads,
application dependency installation and native builds. It does not set up the
original Electron app; follow its development guide for that. `target/` and generated
`sync-output/` reports are excluded from Prettier. The fork CI checks npm lint in a
separate job using the same install command.

## Rebase linear local history

Replay review accepts at most 1,000 candidate commits. Plans above that limit fail
before execution; existing sequencer metadata above the supported count must be
managed with Git. Collision checks allow at most 10,000 path records and 8 MiB of
combined raw path output, with 4 MiB per subprocess stream. Repeated paths count
again across streams; complete target trees are included, so the review limits
also apply to fast-forward and recovery checks. Individually valid streams cannot
silently exceed the combined budget. These bounds do not add a deadline to the
mutating rebase process or one overall deadline to filesystem checks.


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
preserves messages for replayed commits without launching a terminal editor. If
a conflicted commit is resolved entirely to rebased-onto contents, no patch remains
and Git can omit that commit. This differs from preserving clean replay commits
that start empty or become empty without conflict. Abort rebase
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

Preview ours / Preview theirs reads the immutable stage-2/stage-3 blob for a
still-unmerged regular-file path. A missing side is shown as deletion; blobs with
NUL bytes show size only, while other contents use a lossy UTF-8 text prefix up to
64 KiB of raw bytes. Complete blob reads are capped at 8 MiB and index-side
metadata at 64 KiB, using the shared Unix read deadline/output policy. Symlink,
submodule and ambiguous renamed conflict sides remain unsupported.

Git stage semantics are retained: during rebase, ours is the rebased-onto side and
theirs is the replayed commit. Previews show the full blob ID and executable flag,
recheck index/HEAD/status after reading, and clear the old resolution review.
They are read-only, do not bind the current working-file contents, and cannot be
used to apply a choice or enable Mark resolved.

Choose ours / Choose theirs prepares a separate review of both the current working
file and the selected index blob or deletion. Apply choice explicitly confirms
replacement/deletion and staging of that path; Cancel choice discards the review.
The review binds complete contents and executable mode (up to 8 MiB), index/HEAD,
sequencer state, effective attributes and relevant checkout/staging settings.
Changes to these inputs require a new review. Selection, navigation and refresh
clear the pending choice; busy controls cannot start another write. Draft commit
messages remain intact.

Supported write statuses are UU, AA, UD and DU: regular text with unresolved
markers, NUL-containing binary blobs, add/add and modify/delete sides. AU, UA and
DD require external Git for side writes; an absent stage alone cannot authorize
deletion in those more complex states. Read-only side previews remain available. Marker-free UU/AA text is treated
as externally resolved and must use the existing review/stage workflow below.
Filters, text/EOL, ident, working-tree encoding and automatic line-ending
conversions are conservatively refused. Symlinks, submodules, ambiguous renames
and larger blobs require external Git. This is a scoped implementation of G-05;
the complete upstream status/caller/test audit remains open.

Applying uses Git checkout with the selected side followed by add, or Git rm for
a deleted side, with literal native paths and no force flag. Configured checkout
hooks remain active. The final index entry must match the reviewed blob/mode or
absence. These operations are not atomic against external clients. A checkout,
hook or staging failure can leave the working file changed: the app reports the
failure and refreshes; inspect the file/index before recovery. Errors from later
policy/metadata reads also warn that the working file/index may have changed. It does not roll
back over subsequent edits or automatically continue a merge/rebase.

Mark reviewed file resolved stages only that file after rechecking its contents,
executable mode, index, HEAD and marker settings. Locally deleted files can be
reviewed and staged as a deletion. The action does not rewrite files, choose ours
or theirs, or create a commit. Review again after any relevant change. Once all
conflicts are resolved, use the existing commit form; a merge started outside the
app keeps its merge parents. A stash conflict can be resolved and committed while
the saved stash remains available.

External binary editing, symlinks, submodules and files above 8 MiB still need an
external Git tool; supported binary side choices use the separate confirmation above. The UI does not yet edit conflict blocks or offer interactive rebase.
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
branch name. An explicit name wins; when the field is blank, the worker reads
global Git `init.defaultBranch` and uses `main` only when the setting is absent.
During preference resolution, malformed/invalid/empty/non-UTF-8/oversized settings
and lookup failures are reported before creating a destination. Values are validated as literal branch names,
without trimming an invalid value into a fallback. Global configuration is read
only. The parent must already exist. The app can create a
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
configuration. Before reserving metadata, initialization scans destination
ancestors for `.git` entries and conservative bare/separate Git layouts (HEAD,
objects and refs/reftable/packed-refs markers), without interpreting Git failures
as repository absence. Broken/symlinked markers are protected and filesystem
errors propagate. An ordinary project file named HEAD alone remains valid.
These checks do not fence concurrent external filesystem changes.
Initialization is tested end-to-end through the first staged commit;
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

Toolbar controls, Changes/History tabs and footer links separate the seven panels.
File lists and diffs occupy the main workspace; commit fields appear only in Changes. A merge/rebase banner keeps operation controls reachable
while resolving files. Navigation preserves drafts and revokes hidden previews
and abort/remove confirmations. Busy operations keep their current panel.

The default window is 1200×800 with a minimum of 960×640. A renderer layer clips
file rows and toolbar content, including long paths with the software renderer.
Views live in `crates/ui/src/layout.rs`, shared chrome in `layout/shell.rs`, colors
in `appearance.rs` and bounded read-only patch rendering in `diff_view.rs`. Git
operations and their freshness checks are unchanged.

The light workspace now follows GitHub Desktop's main arrangement: repository,
branch and remote controls in a dark toolbar; Changes/History lists; commit inputs
under the file column; selected-file actions and a flexible diff pane. Open or
refresh a repository from Create / Clone (also available through the repository
toolbar), or the welcome view. Saved changes and Git diagnostics remain in the
footer. See [the UI direction and captures](rust/ui-design.md) for scope and next
steps. The first pass retains explicit stage/unstage and existing confirmations;
upstream's commit-inclusion checkboxes and dropdown/dialog flows are not ported.

Tab and Shift+Tab cycle forward/backward through the current text inputs and
enabled panel navigation controls, changed-file buttons and read-only review controls,
in widget-tree order. Disabled navigation controls
are skipped. Only otherwise unhandled Tab presses are mapped; Ctrl/Alt/Super
combinations and captured keys retain their existing behavior. The sole focused
input retains its cursor on a full cycle.

A focused navigation button has a visible outline. Press and release Enter or Space
to open its panel; holding/repeating the key does not repeatedly navigate. Losing
focus, changing modifiers, disabling or changing the button target cancels its
armed activation. Navigation uses the existing busy guard and draft/review rules.
Pointer/touch clicks retain their existing behavior. Changed-file buttons open a
read-only diff; hunk-review and conflict-preview buttons use the same Enter/Space
release behavior. Their identity includes the native repository path, current and
original file paths, observed index/worktree status and read action. Changing that
identity cancels focus/armed activation; ordinary redraws retain it. These controls
reuse existing busy guards and invalidate obsolete reviews while preserving drafts.
Stage/unstage, commit and conflict-resolution write buttons keep separate controls.

Hunk-review checkboxes participate in Tab/Shift+Tab navigation and focus reveal.
Press and release Space to include/exclude a hunk or changed line; Enter does not
toggle them. A held Space toggles once on release. Focus loss, modifiers, another
key, disabling, or changing the exact review/coordinate cancels the gesture.
Ordinary redraws retain focus for the same review coordinate. Selection only
updates the review: use the separate Stage/Unstage action to apply it.

The Close hunk review and Cancel choice buttons also participate in Tab traversal;
Enter/Space dismisses only the review associated with that exact control. Changing
the review instance cancels focus and any armed dismissal, even when its content
is equal. Plain, otherwise unhandled Escape dismisses a hunk review or side-choice
confirmation only on an idle Changes panel. Other modifiers, captured key events,
other panels and busy operations are ignored. With no applicable review it is a
no-op. Escape never requests transfer cancellation or merge/rebase abort. Closing
a hunk review clears its hunk/line selection while preserving commit drafts.

After a focus move, the focused field or navigation control is brought into view
through its own scroll containers with the minimum clamped adjustment. Nested
containers are handled from the inside out. Other panes and already visible
controls keep their scroll position; an oversized control aligns its leading edge.
This covers the default top-left alignment used by the current views. Focus still
works for future controls without bounds metadata, but those controls need an
annotation to be revealed automatically.

Normal redraws of the same field preserve focus, cursor and selection. If a form
slot becomes a different input purpose/context, its native edit state starts fresh;
use Tab or click to select the new field before typing. This leaves stored draft
text intact. Reordering/removing fields may clear their focus rather than transfer
it to another input; common unchanged fields retain their identity.

Native keyboard delivery, restoration across removed/relocated fields,
non-default scroll anchors, IME/HiDPI and complete accessibility/manual acceptance
remain pending. Automated
checks exercise real Iced widgets/events with a headless renderer.

Developer launch options accept a repository, panel and window size:

```sh
cargo run --locked -p desktop-rust -- --repository /path/to/repo --panel history
cargo run --locked -p desktop-rust -- --repository /path/to/repo --size 960x640 --capture /tmp/fresh-window.png
```

`--capture` saves only this application's window as PNG and exits. It refuses
existing output files and reports failure if the window closes before capture.
Launch-time repository inspection is read-only and happens before window creation.
Interactive repository operations continue to use worker threads.

For a read-only focus-ring capture on a disposable repository, use
`--panel keyboard-nav-review` with both `--repository` and a fresh `--capture`
path. The first two frames move focus twice through the current tree (repository
and branch toolbar controls in the new shell); the fourth frame captures the window. No activation key
is injected, no panel navigation is requested and no Git data is written. This
checks rendered focus, not native keyboard delivery.

`--panel keyboard-scroll-review` similarly requires a repository and fresh capture
path. It opens Git diagnostics and applies one backward focus move to the last
focusable control, normally the Git footer link in the new shell, then captures
the window. Long repository/branch labels now exercise toolbar clipping; an
already visible target needs no scroll adjustment. It performs no key activation
or Git write.

A repeatable offline fixture captures all seven panels, a read-only branch
deletion review, missing-Git diagnostics and Unix timeout/output-overflow cases
without touching a real project or contacting a remote:

```sh
cargo build --locked -p desktop-rust
python3 script/rust-ui-smoke.py --output /tmp/fresh-ui-1200 --size 1200x800
python3 script/rust-ui-smoke.py --output /tmp/fresh-ui-960 --size 960x640
```

Each output directory must be new. The script requires a graphical session,
uses the software renderer unless `ICED_BACKEND` is set, creates and removes a
temporary repository, and checks that its working files, config, status, refs and
index remain unchanged. Raw PNGs and `report.json` are local review artifacts.
A small curated set may be committed as milestone review evidence, never as
automated pixel baselines. Compare the images manually: capture success alone does not
establish visual correctness. Automated CI currently runs state tests, not captures.

On 2026-11-24, all six basic panels were captured and visually inspected on Linux
at 1200×800 and 960×640 using the software renderer. This covers the resting
layouts with sample changes, history, remote configuration and a stash. Keyboard
navigation, focus, IME, HiDPI, accessibility, other renderers/platforms, long-list
scrolling and complete interactive write/conflict flows remain unverified.

For complete manual flows, use [Linux alpha acceptance](rust/alpha-acceptance.md).
`script/rust-ui-fixture.py` prepares four persistent offline repositories and can
launch each with isolated Git configuration. Preparation and read-only inspection
are verified; actual interactive acceptance remains pending. The [foundation milestone](rust/consolidation.md) merged through PR #9;
`quality/linux-alpha-usability` carries the next acceptance increments.

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

Port behaviors as independently reviewable commits in coherent milestone batches,
with one PR per batch. Keep the TypeScript reference source separate from Rust ports.
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
Discovery and the watchdog have completed successful manual runs on the default
branch; see [activation evidence](upstream-maintenance.md#activation-and-initial-detection). Scheduled
workflows cannot detect their own complete suspension; monitor Actions externally
if unattended availability is required.
The preview now supports local initialization, stage/unstage, commit, and branch
creation/switching.
Clone, fetch, fast-forward pull, push and upstream setup are implemented as a
prototype. Current/selected local branch rename, creation at a history commit and
deleting merged local branches with confirmation are supported. Force deletion
and in-app authentication remain planned. Diff rendering is monospaced text with
scoped whole-hunk and individual-line staging/unstaging; syntax highlighting,
large-diff virtualization and full interactive conflict editing remain planned.
Automated Git integration tests cover staged/unstaged changes, unborn branches,
rename, deletion, binary files, conflicts, literal pathspecs and non-UTF-8 paths.
Git tests also cover branch creation/switching, detached and unborn HEAD, invalid
names, stale source/target refs, linked worktrees and preservation of dirty files.
UI state tests cover draft retention, branch operation results, busy controls,
write-target invalidation and failed refresh.
The six basic Iced panels have had Linux visual layout checks at two window sizes.
Interactive workflow QA and accessibility testing remain outstanding; see the UI checks above.
