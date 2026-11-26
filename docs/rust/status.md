# Rust rewrite status

Updated: 2026-11-26. This is the implementation/milestone index, not an upstream
parity attestation. Use [backlog](backlog.md) for remaining task IDs,
[quality](quality.md) for engineering rules and `upstream/state.json` for release
checkpoints. Detailed usage/limitations remain in [the rewrite guide](../rust-rewrite.md).

## Planning estimate

As of 2026-11-24, including the published `quality/rust-foundations` branch:

| Target | Indicative completion | Scope |
| --- | --- | --- |
| First local-first Linux alpha | 60–70% | Routine local Git workflows, verified recovery/interaction, minimum installation and documented limits. In-app account features are a later remote-alpha milestone. |
| Complete rewrite | 30–40% | Stable upstream feature coverage, authentication/integrations, native Windows/macOS support, delivery/updates and cross-platform verification. |

These are coarse engineering planning estimates over broad work areas, with low
confidence while the full upstream inventory is incomplete. They are not computed
from test counts, lines of code or equal-sized backlog items. Reassess after
baseline analysis and real interaction QA; they do not advance parity checkpoints
or predict a release date. The current branch still needs consolidated review/merge.

The largest remaining areas are upstream contract inventory/ports, interactive
UI/accessibility QA, stalled-write cancellation, hunk/history/integration features,
accounts/credentials, engine distribution and packaged cross-platform delivery.

## Consolidated work

PRs #1–#5 introduced the foundation, history, UI panels and branch rename/deletion.
PR #6 consolidated the stack onto `development`; #7 persisted the detected
upstream baseline; #8 added external Git runtime diagnostics and interval triage.
All eight are merged. Independent prototype features do not imply release parity.

| Area | Implemented and checked | Remaining boundary |
| --- | --- | --- |
| Workspace | core/Git/Iced split; Cargo lockfile; Rust/npm CI | Quality policy in the current batch still needs its batch merge. |
| Local repositories | init, status, staged/unstaged diffs, full-file stage/unstage, scoped whole-hunk and individual-line staging/unstaging, commits | Broader format/partial-EOF contracts, large-repository virtualization and richer rendering. |
| Local history/branches | Fixed-tip history paging, committed diffs, branch creation at a selected commit, create/switch, current/selected rename, reviewed merged-branch deletion | Stronger concurrent-client guarantees, advanced history editing, native platform/interaction QA. |
| Git workflows | Stash save/restore, conflict review/mark-resolved, merge, linear rebase/continue/abort | Interactive editing/sequences, further recovery flows, complete interaction QA. |
| Remotes | Clone/fetch/fast-forward pull/push; remote settings; transfer output/cancellation | In-app accounts/OAuth and secure credentials; Windows process-tree termination. |
| UI | Seven panels, own-window captures, read-only deletion review and missing-Git scenario | Keyboard/focus/IME/HiDPI/accessibility and complete interactive flows. |
| Engine | External Git version/error reports; Unix probe/history/patch deadlines and capture caps, tested on Linux | Windows probe/history/patch/process-tree bounds; stalled-write deadline/cancellation policy; bundled Git/update/capability policy. |
| Upstream | Weekly discovery, ledger validator, daily watchdog; successful manual runs | Observe scheduled runs, complete initial baseline inventory and port applicable behavior. |
| Delivery | Linux prototype builds and isolated fixture coverage | Packaging, signing, native account adapters and Windows/macOS release verification. |

## Current batch: Rust quality and process hardening

Branch: `quality/rust-foundations`, based on the merged PR #8. The batch groups:

- Pinned Rust compiler/minimum and stable formatter settings.
- Workspace lint inheritance, public API contracts and scoped exceptions.
- Removal of production expect calls, with errors returned through existing APIs.
- One local/CI quality command and negative compiler/policy regression tests.
- Explicit agent rules, this progress index and an ordered backlog.
- Unix version-probe limits and shared managed-child setup/termination, with Linux process, output and retry regressions in `ca5687095f`.
- Unix historical-read deadlines, bounded complete metadata/file lists and streamed message/patch prefix capture in `5b24281183`, including late-error and multi-megabyte fixture coverage.
- Shared fallible reader-pair startup and joining in `3e574c3f75`: child cleanup on thread-start failure, both workers awaited even on reader failure, and transfer inherited-pipe/retry regressions.
- Tracked staged/unstaged patch limits in `52eb1e12b6`, reusing read-only capture and preserving full-file staging after a clipped preview.
- Concurrent local-write pipe draining in `bff5464a57`, with 64 KiB per-stream diagnostic tails and regressions for multi-megabyte input/output, early stdin closure and noisy real hooks.
- Core status/index/HEAD/path and branch-list capture limits in `e6ecdaa16d`, including explicit quiet absent-ref handling, commit-object validation and refusal of writes after oversized scans.
- Remote/stash metadata and branch config/worktree limits in `80fd340d1c`, including remote/URL counts, bounded stash collision review, one tracked-path scan and propagation of stash third-parent read errors.
- Rebase plan/collision/ref reads, shared ancestry queries and conflict attributes in `2e273b3ba9`, plus filesystem-based initialization guards. No production generic whole-output reader remains.
- Final merge-tree/rebase runner migration in `56594e0180`: complete raw protocol limits for merge and marked diagnostic tails with preserved status for rebase. Every Unix Git subprocess now uses shared child/reader ownership.
- UI update separation in `2f79f69b3d`: 74 messages routed exhaustively to ten typed domain handlers, nine support methods moved with them, and shared completion/refresh handling retained.
- Branch-form layout fix in `49a9de727e`, verified at the minimum and default window sizes.
- First scoped upstream behavior port in `aa524c1101` (G-04): configured initial branch for blank input, explicit override and fail-closed preference lookup before initialization.
- First G-05 phase in `c0fcf39b95`: bounded read-only previews of Git index stages 2/3 with raw blob IDs, text/binary/deletion display and native paths. These do not apply a resolution or provide a write token.
- Confirmed G-05 regular-file choices in [ba0696b3c5](https://github.com/leopi99/desktop-rust/commit/ba0696b3c57a19c34c5156a82a46f0ef509ac47f): a separate complete review, explicit replacement/deletion confirmation, stale-input checks, exact staged-blob postconditions and refresh after partial failure. Conversion/filter paths and special file kinds remain unsupported.

The batch is engineering groundwork, with no additional feature PR opened for
each item. Keep commits independently reviewable and group compatible follow-up
work before the next milestone PR. The active branch stays `quality/rust-foundations`
for this reliability batch; continue from it rather than dropping its unmerged
policy by starting again from `development`. Shared capture migration is locally
complete on Unix, and UI handler separation is locally complete. The current
implementation scope is ready for consolidated batch review; stalled-write policy
remains a follow-up. R-05 now covers core snapshot/freshness scans and the
remote/stash/branch-config/worktree adapters and rebase/conflict preflight reads.
Merge-tree structured output and rebase diagnostics now use bounded capture;
R-03 and R-05 are locally complete on Unix, verified on Linux. Windows still has
explicit unbounded-capture fallbacks. The migrated local-write runner (R-06)
has no automatic deadline/cancellation for hooks, signing or inherited pipes.
Status and patch scans each have their own limits; this is not one overall
operation deadline or a bound on every Git command.
Default-branch CI activation follows the batch merge.

## Evidence and confidence

The last merged runtime batch passed 233 Rust tests and 32 upstream tests, and
its available/missing-Git layouts were inspected at 960×640 and 1200×800 on Linux.
The current reliability batch passed 373 Rust tests, 32 upstream tests and 10 policy
tests, including deliberately rejected compiler examples and inheritance/opt-out
checks for future crates. Clippy, formatting, warning-free rustdoc and npm lint
also passed. History tests cover multi-megabyte bodies/patches, oversized metadata,
file-count rejection and unchanged HEAD/index/working files; reader tests cover
truncation boundaries, late failures, stderr overflow and deadlines. Working-diff
tests cover large staged/unstaged patches, unchanged data and complete staging
after clipping, plus unborn/native-path cases. Local-write tests verify bounded
tails, full input delivery, unchanged user data after hook rejection and retry;
real Git routes hook stdout through stderr. Scan regressions exercise oversized
index/status/branch data, refused writes and recovery; missing/corrupt HEAD objects
and non-absence process failures are rejected. Auxiliary regressions cover remote
count/URL overflow, oversized stash/config data, more than 10,000 collision paths,
refused writes and successful retry with saved data retained. Rebase regressions
cover the exact 1,000-commit boundary, large checkout reviews and aggregate path
budgets; initialization rejects damaged bare layouts while retaining ordinary
project files. Final-runner regressions verify raw protocol limits/status, child
completion before overflow errors, verbose merge-driver rejection without user
state changes and noisy rebase pause/abort. Record fresh results for later behavior changes; these counts are dated observations, not a
permanent coverage guarantee.

For the UI refactor, all 82 existing UI state tests pass within the 287-test suite;
no new tests merely mirror the file split. Extraction review compared all 74
original guards/bodies, accounting for payload unboxing and formatter changes.
The fresh Linux/software-rendered smoke run captured eight panels plus three Git
failure states at 960×640 and verified unchanged fixture files/config/index/refs.
Changes, History, deletion review and Git timeout captures were visually inspected.
The deletion-review capture exposed a clipped Create and switch button (UI-03),
subsequently fixed in `49a9de727e` with full-width vertical forms. Fresh smoke runs
passed at 960×640 and 1200×800 (eleven captures each, unchanged fixtures); normal
and deletion-review branch captures were visually inspected at both sizes.
Temporary artifacts are in `/tmp/desktop-rust-branch-layout-960` and
`/tmp/desktop-rust-branch-layout-1200`. They are local evidence and do not automate
interaction or accessibility testing.

The offline smoke fixture verifies its files, refs, index and config remain
unchanged. In addition to normal/missing Git it now captures a stalled executable
and an endlessly noisy executable on Unix. Both error layouts were inspected at
960×640 and 1200×800 on Linux/software rendering. It does not automate button clicks
or establish accessibility parity. Native Windows deadline/tree cleanup remains
pending.
The configured-default increment passed the complete local gate with 289 Rust
tests, including thirteen configuration scenarios across two isolated-process
regressions. Cases cover fallback, Unicode, duplicate values, explicit override,
invalid/malformed/non-UTF-8/oversized settings and missing Git, with no global
config writes or destination reservation in the exercised preflight failures. The updated Create/Clone
panel was captured and visually inspected at 960×640; the temporary image is
`/tmp/desktop-rust-preferred-branch-repository-960.png`.
The first conflict-side phase passed 296 Rust tests (including 84 UI state tests),
32 upstream tests and 10 policy tests, plus Clippy/rustfmt/rustdoc/npm lint.
New cases cover both immutable sides, binary metadata, clipped/oversized blobs,
modify/delete in both directions, literal/non-UTF-8 names, resolved status, busy
UI behavior and absence of write authority. A real-conflict Changes capture at
960×640 was visually inspected; refs/status/index/config/merge parent and working
file remained unchanged. The temporary image is
`/tmp/desktop-rust-conflict-side-controls-960.png`. Actual button-click and full
interactive conflict-resolution QA remain pending.
The confirmed-choice increment passed 306 Rust tests (87 UI state tests),
32 upstream tests and 10 compiler/policy tests, plus the complete Clippy, formatter,
rustdoc and npm lint gates. Seven new Git regressions cover both choices, binary
and add/add sides, both deletion directions, native/literal paths, changed working
files/index/attributes/settings/mode, external text resolutions and checkout followed
by a staging-lock failure with manual recovery. UI tests cover explicit authority,
cancellation/invalidation, busy writes, failure refresh and preserved drafts.
Read-only confirmation captures for both sides at 960×640 and 1200×800 were
visually inspected; fixture files/config/index/refs remained unchanged. Temporary
artifacts: `/tmp/desktop-rust-confirmed-side-final-g973og71`. These captures do not exercise Apply or replace
interactive conflict-resolution/accessibility QA. A fresh minimum-size offline
smoke run also captured eight panels and three Git failure layouts with the fixture
unchanged (`/tmp/desktop-rust-confirmed-side-smoke-960`).
The recovery/status increment ([a083d7f84f](https://github.com/leopi99/desktop-rust/commit/a083d7f84f890144afa6edeaa6cc1324de8ce5a9)) passed 311 Rust tests, including
87 UI state tests, plus 32 upstream and 10 compiler/policy tests and the complete
formatter/Clippy/rustdoc/npm lint gates. Five new behavioral regressions cover
conservative AU/UA/DD refusal with unchanged files/index, multi-conflict merge
through a two-parent commit, both rebase choices through continuation, abort after
a confirmed choice, and checkout hooks that change policy, fail or edit the file.
All errors after the first possible write now carry partial-state recovery context.
No UI layout changed in this increment; the preceding captures remain layout
observations only. Source audit now records the seven unmerged statuses and the
upstream empty-resolution continuation case; the full baseline remains open.
Selected-branch rename in [dcf0589562](https://github.com/leopi99/desktop-rust/commit/dcf0589562e9d68b3fce55a40e0e908f5093894c) passed 319 Rust tests (89 UI state tests),
32 upstream tests and 10 compiler/policy tests, plus the full formatter, Clippy,
rustdoc and npm lint gates. Six new Git regressions verify current state and
tracking/reflog preservation, stale/missing/forged targets, invalid/occupied/orphan
config destinations, other-worktree use, active operations and native non-UTF-8
source names. Two new UI regressions cover action guards, draft preservation and
busy-state behavior. Smoke runs at 960×640 and 1200×800 captured eight panels and
three Git failure cases each, with unchanged fixtures. Branches layouts were
visually inspected; temporary captures are in `/tmp/desktop-rust-selected-rename-960`
and `/tmp/desktop-rust-selected-rename-1200`. These are layout checks, not button-click,
keyboard or accessibility QA. The pinned upstream rename helper/dialog and reflog
test excerpts were inspected; forced case-only retry and account rulesets remain
outside this local scope. No parity checkpoint changed.
History branch creation ([f676e0f9f8](https://github.com/leopi99/desktop-rust/commit/f676e0f9f89aae53ae1026ec7f71de600eb55a6c)) passed 326 Rust tests (92 UI state
tests), 32 upstream tests and 10 compiler/policy tests, plus formatter, Clippy,
rustdoc and npm lint. Four new Git regressions exercise an older commit with dirty
index/worktree, disabled automatic tracking, immutable commit-only inputs, protected
destinations, stale/active operations and detached HEAD. Three UI regressions cover
observed-selection guards, busy redirection, completion/refresh and draft retention.
The full target ID remains visible beside the form. Fresh eight-panel/three-error
smoke runs at 960×640 and 1200×800 left their fixtures unchanged; History captures
were visually inspected in `/tmp/desktop-rust-history-branch-960` and
`/tmp/desktop-rust-history-branch-1200`. These do not automate interaction or prove
accessibility. Pinned history-menu/dialog/Git helper/store excerpts are recorded
in the upstream inventory; feature and release parity remain unset.
Single-hunk staging ([78115619bd](https://github.com/leopi99/desktop-rust/commit/78115619bd5830ce7e46b3cc636312f3c1f89fbe), 2026-11-26) passed 340 Rust tests, including
96 UI state/capture-option tests, plus 32 upstream and 10 compiler/policy tests and
the formatter/Clippy/rustdoc/npm lint gates. Ten new Git regressions verify either
of two blocks, subsequent fresh review, commit of only the selected block, prior
staged changes, later-hunk offsets after earlier insertions, stale/policy refusal,
quoted native paths, EOF/CRLF/empty-file bytes, unsupported file kinds, index-lock
failure and exact 128/129-hunk plus oversized-patch boundaries. Four UI/capture
regressions cover write authority, invalidation, busy actions, drafts and read-only
capture requirements. Two-block review and normal Changes captures at 960×640 and
1200×800 were visually inspected with unchanged fixture files/index/config/refs:
`/tmp/desktop-rust-hunks-review-dwk8i4xf`. The minimum-size general panel smoke also
passed with unchanged fixtures (`/tmp/desktop-rust-hunks-smoke-960`). No button-click/accessibility or
cross-platform parity is claimed. Line selection and hunk unstaging remain open.
Single-hunk unstaging ([4d69c67a06](https://github.com/leopi99/desktop-rust/commit/4d69c67a06905b8fc5daa421d7a3f6756c649f08), 2026-11-26) passed 350 Rust tests,
including 99 UI state/capture-option tests, plus 32 upstream and 10 compiler/policy
tests and formatter/Clippy/rustdoc/npm lint. Seven additional Git regressions cover
either staged block then commit, divergent working edits and other staged files,
reverse coordinates after insertions, direction/forgery refusal, stale input and
lock failure, hard bounds/unsupported types and exact native-path/EOF/CRLF/empty-file
preservation. Two UI regressions cover direction-specific authority, busy and
navigation guards, refresh and drafts; another checks staged capture requirements.
Both review directions and normal Changes were captured at 960×640 and 1200×800
with an unchanged mixed-index fixture (`/tmp/desktop-rust-unstage-hunk-review-1_oe0g3b`); staged reviews were
visually inspected at both sizes. The general minimum-size smoke passed with
unchanged fixtures (`/tmp/desktop-rust-unstage-hunk-smoke-960`). These are not
interactive/accessibility or native platform results. The source inventory records
the difference from upstream's selection-aware index rebuild; parity stays unset.
Multiple-hunk selection ([e04ae273d7](https://github.com/leopi99/desktop-rust/commit/e04ae273d79d3fe81a2c157b59f4ff4fc7155dd9), 2026-11-26) passed 360 Rust tests,
including 103 UI state/capture-option tests, plus 32 upstream and 10 compiler/policy
tests and formatter/Clippy/rustdoc/npm lint. Six new Git regressions cover
nonadjacent blocks and commit of the chosen index, mixed insertion/deletion offsets,
invalid or opposite-direction choices, stale/locked writes without partial prefixes,
all 128 hunks at the accepted boundary and native-path/CRLF round trips. Three UI
regressions cover checkbox uniqueness, empty/wrong-side/busy guards, draft retention,
review invalidation and single-action selection reset; another covers read-only
selected-review captures. Stage/unstage selection captures at 960×640 and 1200×800
were visually inspected with unchanged fixture state during each capture phase
(`/tmp/desktop-rust-multi-hunk-final-u550zg8w`). The fixed action footer stays visible while the blocks scroll;
a width issue found in the first capture was corrected and rechecked. The general
minimum-size panel smoke passed with unchanged fixtures (`/tmp/desktop-rust-multi-hunk-final-smoke-960`). These
captures do not automate interaction/accessibility QA or advance parity.
Individual-line selection ([c1548ebf19](https://github.com/leopi99/desktop-rust/commit/c1548ebf19c96cf556349781ed153936d32918f4), 2026-11-26) passed 373 Rust tests,
including 107 UI state/capture-option tests, plus 32 upstream and 10 compiler/policy
tests and formatter/Clippy/rustdoc/npm lint. Nine new Git regressions cover separate
addition/deletion semantics, one replacement inside a shared hunk then commit,
adjusted cross-hunk counts, invalid/duplicate/context/opposite-side choices,
conservative no-newline handling, row/byte bounds, stale/locked refusal and native
paths/CRLF. One regression exhausts all 15 nonempty subsets of a two-line
replacement in both directions. Three UI regressions cover mixed block/row
selection, refining a whole hunk, direction/busy/invalidation guards and drafts;
another checks read-only line-capture requirements. Partial replacement captures
in both directions were visually inspected at 960×640 and 1200×800 with unchanged
fixture state during each capture phase (`/tmp/desktop-rust-line-selection-wjj6lu1q`).
The general minimum-size smoke also passed with unchanged fixtures (`/tmp/desktop-rust-lines-smoke-960`).
Line controls are bounded to 64 KiB/1,024 changed rows; no-newline hunks require all
changed rows or whole-hunk actions. Broader formats, partial EOF semantics and full
interactive/platform QA remain open; no parity checkpoint is advanced.
No successful test count substitutes for the required source and human review.

On 2026-11-24 the owner reported Actions minutes exhausted until December 2026.
Use the complete local Rust quality command, npm lint and upstream ledger/tests
for current evidence; no GitHub CI result is claimed for this increment. Keep
workflow definitions and quality requirements in place. Recheck hosted CI and
scheduled discovery/watchdog execution when the quota becomes available.

## Upstream checkpoint

Detected: `release-3.6.11`, commit `0102f49aa6c8c554de558299f31b6129b502a2fe`.
The 3.6.10-to-3.6.11 interval is triaged in
[the baseline note](../upstream/release-3.6.11.md): three ledger records distinguish
Git distribution, original hotfix runners and upstream release metadata.
The initial **full baseline** remains pending-analysis; analyzed/parity are unset.
[The partial pinned inventory](../upstream/baseline-3.6.11-inventory.md) inspects five
complete source/test files plus selected config functions and identifies configured-default-branch (G-04) and explicit
manual-conflict-choice (G-05) gaps. G-04 is now locally addressed in `aa524c1101`;
G-05 and every full-feature inventory row remain open. No checkpoint is advanced.
Discovery and watchdog manual success do not prove future scheduled execution.
Local discovery on 2026-11-24 at 16:32 UTC succeeded with no new stable releases;
the baseline SHA remained unchanged. Temporary artifacts are in
`/tmp/desktop-rust-upstream-local-20261124-readers` on the verification machine;
these are local evidence, not uploaded Actions artifacts or a parity assessment.
Follow [the local discovery procedure](../upstream-maintenance.md) weekly while
hosted execution is unavailable.

## Keeping this index useful

Update this file when a batch changes implementation status, evidence or scope.
Move remaining work through backlog task IDs and link the closing commit/PR.
Keep precise source references/checkpoints in the ledger instead of duplicating
release classifications here. Do not mark a task complete because its plan exists.
