# Initial full-baseline inventory: release-3.6.11

Source commit: `0102f49aa6c8c554de558299f31b6129b502a2fe`. Rust observation: `49a9de727eebdb487654a63d846872d2912822f5`.
Updated: 2026-11-24. This is a partial source inventory for U-01. The complete
baseline remains pending-analysis; analyzed/parity checkpoints stay unset.
The separate [release interval triage](release-3.6.11.md) remains valid.

## Scope of the initial pass

Five upstream files were read in full at the pinned commit: initialization helper,
default-branch helper, initialization unit tests, manual conflict router and its
resolution enum. Selected global-config wrappers and the complete configuration
lookup function were also inspected in `app/src/lib/git/config.ts`. The initialization tests were inspected, not executed as upstream
tests. Delegated conflict helpers, UI callers and the wider upstream test suite
still require review. Rust behavior was compared against the source/test paths
below; this pass does not establish parity for either complete feature.

## Initialization and initial branch

Sources: [init helper](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/init.ts),
[default-branch helper](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/helpers/default-branch.ts),
[init unit tests](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/git/init-test.ts),
[config lookup](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/config.ts).

- Upstream initializes with `git -c init.defaultBranch=<resolved name> init`.
- The name resolver reads global `init.defaultBranch`, falling back to `main`
  when the setting is absent. The lookup uses `git config -z --global`, accepts
  exits 0/1, maps exit 1 to absence and takes the first NUL-delimited value. The
  default-branch helper also exposes a setter for the global setting.
- Inspected tests check metadata creation and that symbolic HEAD matches the
  resolver. This inspection alone does not verify every default-setting error case.
- At the initial Rust observation above, [initialization](https://github.com/leopi99/desktop-rust/blob/49a9de727eebdb487654a63d846872d2912822f5/crates/git/src/init.rs) takes an explicit initial
  branch and guards existing Git metadata. The [UI handler](https://github.com/leopi99/desktop-rust/blob/49a9de727eebdb487654a63d846872d2912822f5/crates/ui/src/update/repository.rs)
  substitutes `main` when the field is blank; it does not resolve the global Git
  preference. [Rust initialization tests](https://github.com/leopi99/desktop-rust/blob/49a9de727eebdb487654a63d846872d2912822f5/crates/git/tests/initialization.rs)
  cover explicit branch creation, preserved files, metadata rejection and first commit.

Initial gap **G-04**, now addressed locally in [aa524c1101](https://github.com/leopi99/desktop-rust/commit/aa524c11013d4bdd6cef71509823f33be73df662): the new
`init_repository_with_default_branch` API accepts an explicit override or resolves
the global setting. The UI passes no override for a blank field. Lookup is bounded
to 64 KiB stdout / 16 KiB stderr with a 30-second Unix deadline; only quiet absent
exit 1 enables fallback to main. Configured values are decoded strictly and
validated before metadata reservation; malformed/invalid data never silently
becomes the fallback. The original explicit-branch initializer remains available.

Thirteen isolated-process scenarios exercise unset/configured/Unicode/duplicate
values, explicit override (including an oversized unused preference), malformed,
empty, invalid, non-UTF-8 and oversized config values, missing Git and an explicitly
empty override. Tests call the public initializer, check actual symbolic HEAD,
verify no destination on failure and compare temporary global-config bytes.
These are Rust regressions, not execution of the original upstream test suite.

The global-setting setter/settings UI and upstream caller/location validation
still require their own scope. This local port does not complete the whole
initialization audit or advance analyzed/parity checkpoints. Ledger feature status
remains prototype pending the required source, PR and substantive review evidence.

## Explicit manual conflict choices

Sources: [manual conflict router](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/stage.ts),
[resolution enum](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/models/manual-conflict-resolution.ts).

- Upstream represents explicit `ours` and `theirs` choices. The router selects the
  corresponding status entry, checks out a side for unmerged/both-added cases,
  then delegates add/delete handling according to the selected entry.
- For marker-bearing text-conflict status with zero remaining markers, the router
  leaves externally resolved content alone instead of applying the side choice.
- Checkout/add/remove helpers and their callers/tests were not inspected in this
  pass; overwrite, recovery and binary behavior must be reviewed there.
- Rust [conflict review](https://github.com/leopi99/desktop-rust/blob/49a9de727eebdb487654a63d846872d2912822f5/crates/git/src/conflicts.rs) reads externally
  edited regular text/deletion, verifies freshness/markers and stages the reviewed
  result. Binary, oversized, symlink and submodule conflicts require external Git.
  [Rust conflict tests](https://github.com/leopi99/desktop-rust/blob/49a9de727eebdb487654a63d846872d2912822f5/crates/git/tests/conflicts.rs) cover these guards
  and preservation cases. There is no explicit ours/theirs selection in the UI.

Observed gap **G-05**: complete the delegated-helper/caller/test review, then add
reviewed manual side selection with stale-state and user-data protections.
This is a behavior gap to design and test, not permission for implicit overwrites.

## G-05 follow-up: delegated behavior and read-only preview phase

Additional source excerpts inspected at the same pinned upstream commit:
[checkoutConflictedFile](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/checkout.ts),
[addConflictedFile](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/add.ts),
[removeConflictedFile](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/rm.ts),
[merge-commit caller](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/commit.ts),
[rebase continuation caller](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/rebase.ts),
and relevant [resolution-diff test excerpts](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/git/diff-stage-test.ts).
These are focused helper/caller/test inspections, not whole-file audits of the
larger checkout, commit, rebase or app-store modules.

The helpers use literal `checkout --ours/--theirs -- path`, `add -- path` and
`rm -- path`. Manual choices are applied by merge-commit/rebase-continuation
callers before staging other files. The inspected diff tests exercise both index
sides and missing/deleted sides. This does not yet establish the complete binary,
add/add, rename, recovery or external-edit contract.

Rust [c0fcf39b95](https://github.com/leopi99/desktop-rust/commit/c0fcf39b9565013f925603f604d614cfe4dabca1) introduces only the read-only preview foundation:
validated native unmerged index records, full blob IDs/modes, bounded blob reads,
text/NUL-binary/deletion display, post-read freshness and a UI without write
authority. Tests verify preserved user/index state, resource limits and stale/busy
cases. It is a raw side-content preview, not upstream's working-file-to-side diff
renderer. At that preview-only phase, applying explicit choices still needed a separate review binding the
working contents, index, attributes/conversion policy and confirmation, plus the
remaining status matrix and failure/recovery tests. G-05 remains in progress;
feature and release parity checkpoints remain unchanged.

## G-05 follow-up: confirmed regular-file subset

Rust [ba0696b3c5](https://github.com/leopi99/desktop-rust/commit/ba0696b3c57a19c34c5156a82a46f0ef509ac47f) adds a distinct complete review and explicit confirmation for
regular-file replacement/deletion. It uses the inspected checkout/add/remove
sequence, preserves native paths and checks the staged blob/mode against the
reviewed side. Working bytes/mode, index/HEAD/sequencer, attributes and relevant
settings are revalidated. Marker-free external text resolutions are preserved;
conversion/filter attributes and automatic line-ending conversion are refused.
Tests cover text/binary/add-add choices, both missing-side directions, stale data
and a real post-checkout hook causing staging failure, with manual recovery.
The UI refreshes after errors and retains draft messages. Checkout and staging
are separate operations; neither the process mutex nor preflight fences external
clients. Special file kinds, conversions, the full caller/status/test matrix and
interactive/platform QA remain open. This scoped port does not advance feature
or release parity checkpoints.

## G-05 follow-up: status matrix and operation recovery

Focused excerpts inspected at the pinned source commit:
[status types](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/models/status.ts),
[status conflict classification](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/status.ts),
[merge/manual-resolution tests](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/git/commit-test.ts)
and [rebase progress/continuation tests](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/git/rebase/progress-test.ts).
Upstream distinguishes marker-bearing UU/AA text from manual conflicts using its
binary classification. Its merge tests cover preserved/deleted modify-delete
files, both add/add sides and binary choices through commit. Its rebase tests
repeatedly select theirs and continue through successive conflicts. These focused
excerpts do not complete whole-file, UI, platform or original fixture audits.

The standard unmerged index matrix guides the scoped Rust write boundary:

| Porcelain status | Index stages | Rust side-write scope |
| --- | --- | --- |
| UU | 1, 2, 3 | Explicit regular-file side confirmation; marker-free external text is preserved. |
| AA | 2, 3 | Explicit regular-file side confirmation; marker-free external text is preserved. |
| UD | 1, 2 | Confirm ours contents or theirs deletion. |
| DU | 1, 3 | Confirm ours deletion or theirs contents. |
| AU | 2 | Read-only preview; side writes require external Git. |
| UA | 3 | Read-only preview; side writes require external Git. |
| DD | 1 | Read-only preview; side writes require external Git. |

An absent stage by itself cannot establish an intended deletion in AU/UA/DD or
coordinate related rename paths. [a083d7f84f](https://github.com/leopi99/desktop-rust/commit/a083d7f84f890144afa6edeaa6cc1324de8ce5a9) explicitly rejects those write
reviews, while retaining read-only previews. This is a conservative scope boundary,
not a completed port of every upstream status. Conversion and special-file limits
from the previous phase still apply. Rename handling across related paths remains
unimplemented; the status guard does not detect every possible rename relationship.

New Rust operation regressions resolve UU plus AA conflicts separately, require
fresh review after staging another file, then verify both merge parents and
untracked-file preservation. Rebase tests confirm stage semantics through continue
for both sides, original branch restoration and abort after a chosen resolution.
Choosing all rebased-onto contents leaves no patch: Git omits that replayed commit.
The inspected upstream [continuation](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/rebase.ts)
likewise explicitly skips when no tracked changes remain. Originally empty or
already-applied clean replay commits remain a separate policy from this conflicted
continuation case.

All errors after entering checkout/remove/add now carry partial-state recovery
context, including failed post-checkout policy/metadata reads. Real checkout hooks
exercise conversion-policy changes, hook rejection and subsequent working edits;
these paths preserve the unmerged index and do not stage a hook-edited file.
No automatic rollback, transaction against external clients or parity checkpoint
advance is claimed.

## Local branch rename subset

Focused source inspected at the pinned commit:
[renameBranch helper](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/branch.ts),
[dialog submission/rule checks](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/ui/rename-branch/rename-branch-dialog.tsx)
and [reflog rename test](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/git/reflog-test.ts).
The helper receives an explicit branch and uses Git branch move. Its error path
can retry a case-only collision with a forced move after checking branch names.
The dialog forwards its selected branch and new name and performs separate
account/repository rule checks. The inspected reflog test renames the current
branch and checks that recent branches use the updated name. This does not audit
all dispatcher callers, rulesets, branch tests or platform behavior.

Rust [dcf0589562](https://github.com/leopi99/desktop-rust/commit/dcf0589562e9d68b3fce55a40e0e908f5093894c) extends the existing current-branch command with explicit
non-current local branch rename. It checks the observed target name/full OID/current
flag, source index/HEAD/branch, idle operation state, destination refs/config and
worktree use, then invokes literal `branch --move -- source destination`. No
forced retry is attempted. Git owns ref/config/reflog changes; no checkout or
remote operation is requested. Current and selected rename share destination and
worktree checks. Current HEAD, index/files and the target's original remote tracking
destination remain intact in the exercised regressions; local config and reflog
follow the new name. Six additional Git tests cover preservation and refusal cases,
including native source names and a branch open in another worktree. UI guards and
shared failure/refresh behavior are covered separately.

Concurrent external ref/config changes can still race preflight. Account/ruleset
validation, forced case-only rename policy, other platforms and full interactive
QA remain outside this scoped port. G-03 still includes further history operations;
no feature/release parity checkpoint advances.

## Branch creation from an observed commit

Focused pinned excerpts inspected:
[history context menu](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/ui/history/commit-list.tsx),
[history callback](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/ui/history/compare.tsx),
[creation dialog](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/ui/create-branch/create-branch-dialog.tsx),
[Git helper](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/branch.ts)
and [Git store creation](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/stores/git-store.ts).
The menu exposes creation from a commit, the callback opens a dialog with
`targetCommit`, and the dialog uses its SHA as the start point. The Git helper
uses `branch` with an explicit destination/start point and optional no-track flag;
the store refreshes its local branch list. These excerpts do not audit all
higher-level dispatcher effects, ruleset/account checks, tests or platforms.

Rust [f676e0f9f8](https://github.com/leopi99/desktop-rust/commit/f676e0f9f89aae53ae1026ec7f71de600eb55a6c) adds a scoped History action: full commit-only object ID,
explicit name and visible target ID, `branch --no-track -- name oid`, no checkout.
It always disables automatic tracking; UI creation and refresh keep History open.
Success clears the creation draft; failure retains it; other drafts remain intact.
The source basis, idle sequencer state, destination ref and orphan local config
are checked through bounded readers before writing. Destination checks are shared
with current/selected branch rename. Regressions cover creation at an older commit
with staged/unstaged/untracked data, automatic-tracking settings, detached HEAD,
non-commit/missing/symbolic/abbreviated IDs and stale/occupied/active-operation
refusals. UI state tests verify observed selection and busy/completion behavior.
Full interaction/platform QA, the complete upstream creation flow and further
history editing remain open. No parity checkpoint is advanced.

## Partial staging: first complete-hunk subset

Focused pinned source inspected:
[index patch application](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/apply.ts),
[patch-header/hunk formatting excerpts](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/patch-formatter.ts)
and [first/second-hunk formatter tests](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/patch-formatter-test.ts).
The upstream helper builds a selection-aware patch and applies it to the index
with cached apply, no whitespace correction and support for zero-context hunks.
Its rename path also reconstructs the staged rename. The formatter handles
individual selected/unselected lines and computes adjusted row counts/coordinates;
the inspected tests exercise choosing the first or second block. These focused
excerpts do not audit the complete selection model, UI, all original fixtures,
conversion policies or platform behavior.

Rust [78115619bd](https://github.com/leopi99/desktop-rust/commit/78115619bd5830ce7e46b3cc636312f3c1f89fbe) stages a single complete context-bearing hunk of an existing
unrenamed regular UTF-8 text modification. It retains Git's original quoted patch
header and hunk bytes; validates row counts and refuses non-regular-mode headers,
new/deleted/renamed/special/binary data and conversion policies. Full patch input
is bounded to 512 KiB/128 hunks and working content to 8 MiB. A distinct review
binds status, index/HEAD/operation state, full working bytes/mode, conversion
attributes/settings and all patch blocks. Staging reruns the complete review,
checks then applies only header plus selected block using cached apply, without
zero-context, three-way, reject or force modes. The ordinary clipped diff is not
a write token. Git owns index locking; external clients/hooks are not fenced by
preflight or the process mutex.

Regression evidence covers partial-index/working preservation, both blocks and
shifted coordinates, commit of one chosen block, EOF/CRLF/empty-file cases, native
paths, unsupported and stale input, lock failure and hard size/hunk limits. The UI
exposes each block's action, revokes reviews on navigation/refresh and preserves
drafts. Hunk unstaging, line/multiple selection, conversion/special-file support,
full upstream formatter/test audit and interactive/platform QA remain open. This
is a scoped G-02 increment, not feature/release parity.

## Partial staging: reverse index-only hunk operation

Additional pinned excerpts inspected:
[ordinary commit index reset/rebuild](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/commit.ts),
[partial selection pass in stageFiles](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/update-index.ts)
and [whole-index unstage helper](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/reset.ts).
Upstream ordinary commit clears the index and stages its selected files/lines;
stageFiles routes partial selections to the previously inspected patch helper.
Its merge-commit path explicitly preserves existing staging. These observations
are not evidence of a dedicated upstream reverse-hunk UI/API, nor a complete audit
of all commit/selection callers. Rust's separate explicit index/working views
remain a scoped design difference: the Rust commit action preserves and commits
the staged index instead of reconstructing it from the upstream selection model.

Rust [4d69c67a06](https://github.com/leopi99/desktop-rust/commit/4d69c67a06905b8fc5daa421d7a3f6756c649f08) adds a staged-side review of HEAD-to-index changes and uses
reverse cached apply for exactly one context-bearing block. The review binds the
comparison direction; the stage and unstage APIs and UI reject the opposite side.
Both directions share parsing, bounds, complete freshness and index-only execution.
Only existing regular text modifications with a present supported working file
are included; staged adds/deletes/renames/mode changes, special/binary files and
conversion policies remain excluded. No checkout, global index reset, force,
three-way fallback or automatic rollback is requested.

New regressions verify both block choices and commit of the remainder, shifted
reverse coordinates, extra unstaged edits and other staged files, opposite/forged
reviews, stale inputs, index locks, patch/hunk bounds and native/EOF/CRLF/empty-file
bytes. UI/capture tests verify directional authority, guarded actions and invalidation.
Line/multiple selection, complete upstream selection/commit semantics, broader
formats and interactive/platform QA remain open; no parity checkpoint changes.

## Partial staging: multiple complete hunks in one file

Additional focused pinned evidence:
[first-and-third-hunk selection test](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/patch-formatter-test.ts)
and [selection-aware formatter tail](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/patch-formatter.ts).
The inspected test omits the second hunk while retaining the first and third;
the formatter appends selected changes across hunks and rejects an empty patch.
Its line selection and coordinate reconstruction support a broader surface than
this Rust subset; these excerpts are not a full original fixture/UI audit.

Rust [e04ae273d7](https://github.com/leopi99/desktop-rust/commit/e04ae273d79d3fe81a2c157b59f4ff4fc7155dd9) accepts several complete hunks from the same direction-bound
review. It rejects empty/duplicate/invalid index choices before applying any block,
orders valid input by original patch position, revalidates the full review once,
then checks and writes one cached patch, reversed only for unstaging. The index is
not updated by a per-hunk loop. Git owns its index lock; preflight and the separate
check/write calls do not fence external clients or configured hook effects.

Checkbox selection starts empty, is tied to the current file/review, and clears on
review replacement, navigation, refresh, cancellation and any hunk write. The count
and matching Stage/Unstage selected action remain outside the scrolling blocks.
Regressions cover first/third selection, mixed insertions/deletions and reverse
coordinates, remaining staged/working data, invalid/stale/locked refusal,
128-hunk selection and native-path/CRLF preservation, plus UI guards and capture
requirements. Line selection, multi-file batches, broader formats and complete
upstream selection/commit/platform QA remain outside this increment. Existing
512 KiB/128-hunk/8 MiB limits and conversion exclusions are retained. No feature
or release parity checkpoint advances.

## Partial staging: individual changed rows

Focused pinned excerpts inspected again for this slice:
[selection-aware formatter](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/patch-formatter.ts)
and [addition-only/preceding-deletions formatter test](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/patch-formatter-test.ts).
The upstream formatter retains context, drops unselected additions, converts
unselected deletions to context, adjusts counts and emits no-newline markers.
Its inspected test selects an addition while preceding deletions remain excluded.
These excerpts are not a full selection-model, original-fixture or UI audit; the
upstream ordinary commit index-rebuild difference remains as recorded above.

Rust [c1548ebf19](https://github.com/leopi99/desktop-rust/commit/c1548ebf19c96cf556349781ed153936d32918f4) adds forward and reverse selection of changed rows in a
complete review. Stable coordinates identify the hunk and physical row; headers,
context and markers cannot be selected. For reverse application the retention
rules are dual: unselected added rows become context and unselected deleted rows
are omitted, so the current index remains the source of the reverse patch. Start
coordinates and counts are reconstructed with checked arithmetic and accumulated
selected deltas. Empty, duplicate, invalid and stale choices fail before writing.
The same shared cached check/apply runner executes one patch, preserving working
files and unrelated index changes in the exercised regressions.

Line controls require the whole review to fit 64 KiB and 1,024 changed rows;
whole-hunk limits stay unchanged. Partial choices in no-newline hunks are refused;
all changed rows or the whole hunk remain available. The UI can combine full
blocks with rows, refine a full block by unchecking a row, and invalidates both
selection sets together. Regressions include all 15 nonempty subsets of a two-line
replacement in both directions, replacement-only commits, adjusted offsets, bounds,
EOF/CRLF/native names, stale/locked refusal and guarded UI/capture behavior.
Broader formats/conversions, partial EOF contracts, the complete upstream test/UI
surface and interactive/platform QA remain open. No parity checkpoint advances.

## Partial selection with missing final newlines

The pinned [no-newline formatter test](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/test/unit/patch-formatter-test.ts)
constructs a synthetic diff, selects an added line and asserts that the formatted
patch includes its addition and a no-newline marker. The previously inspected
[formatter](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/patch-formatter.ts)
retains markers associated with included rows. Those assertions do not establish
actual cached application or exact byte preservation for every partial EOF choice.
No conclusion about implicit joining behavior is inferred from string presence.

Rust [7b8d969153](https://github.com/leopi99/desktop-rust/commit/7b8d9691535b25a5ffed79c111f616cfef73ab11) replaces the blanket all-changed-rows requirement with independent
old/new stream validation after selection transformation. A no-newline marker marks
EOF only for the streams to which its preceding retained row belongs. Any later
row belonging to an stream already marked at EOF is refused before applying the patch;
markers for omitted rows stay omitted. Partial selections before unchanged EOF
context and valid deletions are accepted; silently adding a separator or joining
lines is not introduced. The UI exposes changed-row controls in no-newline hunks,
keeps markers nonselectable and explains that adjoining EOF changes may need to be
selected together. Existing direction, freshness, byte/row and whole-hunk bounds
remain unchanged.

Four regressions execute real Git operations across old/new newline combinations
and selection subsets in both directions, verify LF/CRLF context preservation,
cross-hunk count changes and newline-only/empty-content transitions. Accepted
choices must produce the exact expected index bytes with the working file intact;
refused ambiguous choices must preserve the basis. Complete upstream fixture/UI,
implicit editing/joining semantics and platform QA remain outside this increment.
No feature or release parity checkpoint advances.

## Working-file preview bounds and repository-root binding

Focused pinned [working-directory diff excerpts](https://github.com/desktop/desktop/blob/0102f49aa6c8c554de558299f31b6129b502a2fe/app/src/lib/git/diff.ts)
were inspected: added/untracked non-submodule text diff uses Git's no-index route
against the null path and accepts its changes-found exit status. The separate
working-directory image helper reads filesystem bytes for image presentation.
Those are different workflows; this inspection does not establish their full
caller/budget/platform contracts or implement image support in Rust.

The [prior Rust plain-content preview](https://github.com/leopi99/desktop-rust/blob/3fa13a1a58aa0d8c15a677fe292e5fd02c05fcf5/crates/git/src/lib.rs)
checked untracked file size and then used an unbounded filesystem read. A file
changing after that metadata check could bypass the intended capture bound.
Rust [d3d72cc76f](https://github.com/leopi99/desktop-rust/commit/d3d72cc76fa2731cc814511254782dc8cdf551d7) replaces that path with a shared regular-file reader bounded
to the requested bytes plus one overflow probe, rejecting an incomplete prefix.
Untracked preview uses 1 MiB; conflict/hunk working-content capture uses 8 MiB.
Unix leaf opens use no-follow/nonblocking flags; type checks reject non-regular
descriptors before reading. Ancestors are checked before and after opening, but
external component renames are not fenced. Symlink preview displays target spelling
without reading target contents. Windows retains best-effort checks, and regular
filesystem I/O has no automatic deadline.

Snapshot and preview share bounded Git-root resolution. A preview requested from
a subdirectory uses root-relative selected paths consistently for tracked patches
and untracked files, without lossy native-path conversion. Regression evidence
covers read budgets/growing streams, late errors, symlink/type guards, native root
names, bounds, unchanged index/files/config and full staging after overflow refusal.
This is Rust hardening of its plain-content preview, not an upstream no-index diff
or image port and not a parity checkpoint advance. Complete diff/image/platform
contracts and descriptor-relative ancestor handling remain outside this scope.

## Full-baseline coverage grid

All rows remain open. Partial helper inspection is not a completed feature audit.
Feature identifiers match `upstream/features.json`.

| Feature IDs | Inventory state | Next evidence |
| --- | --- | --- |
| repository-init | Partial helper/test inspection; G-04 locally ported | UI callers, preference errors/settings, directory validation, platform behavior. |
| conflict-resolution | Partial router/helper/caller/status/test inspection; scoped confirmed writes and merge/rebase recovery tests implemented | Coordinated rename and conversion/special-file contracts, original fixture/UI audit and interactive/platform QA. |
| stage-commit | Partial complete-hunk apply/formatter/test inspection | Broader formats/implicit EOF editing, full index/commit selection contracts and original tests. |
| repository-read, diff, commit-history | Pending full inventory | Status/diff/history contracts, parsing, limits and original tests. |
| branches, stash | Partial rename and commit-based creation slices; rest pending | Full branch/stash operations, worktrees, tracking, recovery, original tests and platforms. |
| remote-settings, remotes, remotes-auth | Pending | URLs/config, transports, accounts, credentials, cancellation and tests. |
| local-merge, linear-rebase, merge-rebase-conflicts | Pending | Preview/execute/recovery contracts, sequencer state, UI integration and tests. |
| lfs-submodules, enterprise-pull-requests, advanced-integrations | Pending | Feature surface, dependencies, platform and authentication requirements. |
| git-runtime, git-distribution, release-packaging | Pending full inventory | Engine capabilities, native integrations, updates/signing; interval triage covers only the release delta. |

For each slice, collect immutable source/test references, caller-shaped contracts,
Git argument/status/config semantics, platform dependencies and corresponding Rust
implementation/test evidence. Record gaps in the backlog; classify ledger records
with the required source and Rust review evidence before advancing checkpoints.
