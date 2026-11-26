# Rust rewrite status

Updated: 2026-11-26. This is the implementation/milestone index, not an upstream
parity attestation. Use [backlog](backlog.md) for remaining task IDs,
[quality](quality.md) for engineering rules and `upstream/state.json` for release
checkpoints. Detailed usage/limitations remain in [the rewrite guide](../rust-rewrite.md).

## Planning estimate

Estimates recorded on 2026-11-24, retained after PR #9 merged on 2026-11-26:

| Target | Indicative completion | Scope |
| --- | --- | --- |
| First local-first Linux alpha | 60–70% | Routine local Git workflows, verified recovery/interaction, minimum installation and documented limits. In-app account features are a later remote-alpha milestone. |
| Complete rewrite | 30–40% | Stable upstream feature coverage, authentication/integrations, native Windows/macOS support, delivery/updates and cross-platform verification. |

These are coarse engineering planning estimates over broad work areas, with low
confidence while the full upstream inventory is incomplete. They are not computed
from test counts, lines of code or equal-sized backlog items. Reassess after
baseline analysis and real interaction QA; they do not advance parity checkpoints
or predict a release date. PR #9 is now merged; the next usability batch is on
`quality/linux-alpha-usability`. [The consolidation note](consolidation.md) records
the merged milestone; [alpha acceptance](alpha-acceptance.md) provides disposable
manual test fixtures and explicit pending outcomes.

The largest remaining areas are upstream contract inventory/ports, interactive
UI/accessibility QA, stalled-write cancellation, hunk/history/integration features,
accounts/credentials, engine distribution and packaged cross-platform delivery.

## Consolidated work

PRs #1–#5 introduced the foundation, history, UI panels and branch rename/deletion.
PR #6 consolidated the stack onto `development`; #7 persisted the detected
upstream baseline; #8 added external Git runtime diagnostics and interval triage.
PR #9 consolidated the Rust quality/runtime/workflow batch on 2026-11-26 at
[e496be487e](https://github.com/leopi99/desktop-rust/commit/e496be487e164bcc5be61654dbe514ccfa0e4a99). All nine are merged; independent prototype features do not imply release parity.

| Area | Implemented and checked | Remaining boundary |
| --- | --- | --- |
| Workspace | core/Git/Iced split; Cargo lockfile; strict shared Rust policy and Rust/npm CI merged through PR #9 | Follow-up changes still need their own batch validation/review. |
| Local repositories | init, status, staged/unstaged diffs, full-file stage/unstage, scoped whole-hunk and individual-line staging/unstaging for modifications, additions and deletions; staging working edits at staged rename destinations; commits | Coordinated rename/copy operations, broader formats, implicit EOF joining/editing, large-repository virtualization and richer rendering. |
| Local history/branches | Fixed-tip history paging, committed diffs, branch creation at a selected commit, create/switch, current/selected rename, reviewed merged-branch deletion | Stronger concurrent-client guarantees, advanced history editing, native platform/interaction QA. |
| Git workflows | Stash save/restore, conflict review/mark-resolved, merge, linear rebase/continue/abort | Interactive editing/sequences, further recovery flows, complete interaction QA. |
| Remotes | Clone/fetch/fast-forward pull/push; remote settings; transfer output/cancellation | In-app accounts/OAuth and secure credentials; Windows process-tree termination. |
| UI | Seven panels, own-window captures, read-only deletion review and missing-Git scenario; Tab/Shift+Tab traversal and focus reveal for text inputs/enabled sidebar navigation on the follow-up branch | Native focus/keyboard activation for other controls, IME/HiDPI/accessibility and complete interactive flows. |
| Engine | External Git version/error reports; Unix probe/history/patch deadlines and capture caps, tested on Linux | Windows probe/history/patch/process-tree bounds; stalled-write deadline/cancellation policy; bundled Git/update/capability policy. |
| Upstream | Weekly discovery, ledger validator, daily watchdog; successful manual runs | Observe scheduled runs, complete initial baseline inventory and port applicable behavior. |
| Delivery | Linux prototype builds and isolated fixture coverage | Packaging, signing, native account adapters and Windows/macOS release verification. |

## Current batch: Linux alpha usability

Branch: `quality/linux-alpha-usability`, based on verified PR #9 merge
`e496be487e164bcc5be61654dbe514ccfa0e4a99`. The merge contains the full published
foundation tree; the checkout was fast-forwarded before branching.

First increment [d6024034ca](https://github.com/leopi99/desktop-rust/commit/d6024034caa0086b97d204c0e60916041e44f66d) adds unhandled Tab/Shift+Tab navigation among
text inputs in the current widget tree, with cyclic forward/backward focus and
no cached field IDs. Other modifier combinations, captured events and text-edit
keys are left to their existing handlers. Empty/one-input trees are stable;
revisiting the sole focused input preserves its cursor. The operation never
activates buttons, writes Git data or invalidates drafts/hunk selections.

Five new regressions use the actual Iced event subscription and text-input widgets
with a headless renderer, plus application state preservation while busy. They
are automated widget checks, not a native keyboard session. At that increment,
button activation was outside the focus traversal. The sidebar follow-up below
adds navigation controls; Git action buttons/checkboxes, focus scrolling/panel
handover and full A-01–A-10 acceptance remain open. This branch groups compatible usability work
before one milestone PR; no per-feature PR is opened automatically.

This increment passed 413 Rust tests (112 UI), 10 compiler/policy tests, 32 upstream
tests, formatter, all-target/all-feature Clippy, warning-free rustdoc and npm lint.
The minimum-size own-window smoke passed eight panels and three Git failures with
unchanged fixture state (`/tmp/desktop-rust-linux-usability-smoke-960`). The root UI
protocol is now 92 variants, with focus routed exhaustively through application
events. No native keyboard session or complete accessibility QA is claimed.

Sidebar follow-up [7085d9701e](https://github.com/leopi99/desktop-rust/commit/7085d9701eb5c8980e9fc07d97d521a23288da22) makes the seven enabled panel buttons focusable
in the same traversal, with a theme-text-color focus ring. Enter/Space arm on key
press and emit the existing PanelSelected event once on matching unmodified key
release. Repeats do not emit extra actions. Focus/window loss, another key,
modifier changes, disabling or changing the bound panel cancel the armed state.
A benign redraw of the same target preserves focus. Mouse/touch activation still
uses the original Iced button path. Disabled sidebar buttons are skipped, and
queued navigation still passes through the application's busy guard.

Six new widget/event regressions cover mixed input/navigation traversal, disabled
controls, release/repeat semantics, interrupted/stale presses, modifiers and
unfocused keys, mouse/touch compatibility, redraw and late busy delivery. One
capture-option regression checks explicit repository/destination guards. The
complete local gate passes 420 Rust tests (119 UI), 10 compiler/policy tests and
32 upstream tests, plus formatter/Clippy/rustdoc/npm lint.

A read-only `keyboard-nav-review` developer capture applies two focus moves during
the first frames, then records the existing window capture. It neither activates
a panel nor writes Git data. Focus rings were captured and visually inspected at
960×640 and 1200×800 with unchanged fixture signatures
(`/tmp/desktop-rust-nav-focus-801a9we3`). The minimum-size eight-panel/three-failure
smoke also passed (`/tmp/desktop-rust-nav-smoke-960`). Programmatic focus/captures
and headless widget events do not establish native keyboard/IME/accessibility QA.
The root protocol stays at 92 messages; Git action buttons and checkboxes are not
added to keyboard activation. UI-01/UI-04 and baseline analysis remain open.

Focus visibility follow-up [5c8bb1b3b7](https://github.com/leopi99/desktop-rust/commit/5c8bb1b3b7a678edfda68d9d90d5fe933c3b7154) adds bounds metadata around the current
text inputs and sidebar buttons without changing their state, layout, pointer
handling or overlays. A focus move measures the current widget tree and reveals
the selected region only through its ancestor scroll containers. Nested containers
are planned from the inside out; adjustments are clamped to content bounds.
Already visible targets and unrelated scroll regions keep their offsets. Oversized
targets align their leading edge without oscillation. Unannotated future controls
retain ordinary focus behavior, and non-finite geometry refuses a scroll plan.
This scope covers the application's default top-left scroll alignment.

Six new real-Iced-widget regressions cover forward/backward/wrap traversal, nested
and unrelated containers, stable visible/empty trees, both top-left axes,
oversized targets and unannotated controls. The read-only `keyboard-scroll-review`
capture guard is included in the existing capture-context regression. It opens
Git diagnostics and applies one previous-focus move, without an activation key.
Before/after own-window captures with long generated repository/branch names show
the previously offscreen Git navigation button revealed and focused at 960×640
and 1200×800; fixture signatures remain unchanged
(`/tmp/desktop-rust-focus-reveal-gok0j7jo/captures`). Native keyboard delivery,
non-default scroll anchors, panel focus handover, Git action buttons/checkboxes,
virtualized-list semantics and accessibility acceptance remain open.
The complete increment passed 426 Rust tests (125 UI), 10 compiler/policy tests,
32 upstream tests, formatter, all-target/all-feature Clippy, warning-free rustdoc
and npm lint. The minimum-size eight-panel/three-failure smoke passed with
unchanged fixture data (`/tmp/desktop-rust-focus-reveal-smoke-960`). The root UI
protocol remains 92 messages, reusing the existing focus operation route.

Semantic field identity follow-up [19ad69e3da](https://github.com/leopi99/desktop-rust/commit/19ad69e3da6c20203298b4daea93b4421b70c947) prevents a reused widget slot
from carrying focus/cursor/selection into an input with another purpose. Current
form inputs use explicit purpose/context IDs; the shared branch-name draft has
different IDs in History and Branches. A changed ID recreates only that field's
native edit state, while same-ID redraws retain focus and selection. Draft values
stay in the application model. A common repository-path field keeps its identity
when the panel body changes. Affected reordered inputs lose focus safely rather
than transferring it to a different purpose; this does not restore focus across
removed/recreated widgets or arbitrary relocations.

Five new real-widget/event regressions verify blocked typing after retargeting,
explicit refocus before editing the new field, same-field selection retention,
reordered fields, shared-draft contexts, common fields and stable parent scroll
offsets/reveal. The complete local gate passes 431 Rust tests (130 UI), 10 policy
tests and 32 upstream tests, plus formatter/Clippy/rustdoc/npm lint. The minimum-size
eight-panel/three-failure smoke passed with unchanged fixture data
(`/tmp/desktop-rust-field-identity-smoke-960`). Programmatic navigation/scroll-focus
captures were visually inspected at 960×640 and 1200×800 with unchanged fixture
signatures (`/tmp/desktop-rust-field-identity-focus-vx4h8iz0`). The root protocol
remains 92 messages. Native keyboard/IME/accessibility and complete panel-focus
restoration remain open in UI-01/UI-06; no upstream checkpoint advances.

## Merged batch: Rust quality and process hardening

Branch: `quality/rust-foundations`, merged through PR #9. The batch included:

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

The foundation batch is merged engineering groundwork, not an alpha release.
Its independent commits remain reviewable in PR #9. Continue usability work from
`quality/linux-alpha-usability`, preserving the merged policy. Shared Unix capture
migration and UI handler separation are locally complete; stalled-write policy
remains a follow-up. R-05 now covers core snapshot/freshness scans and the
remote/stash/branch-config/worktree adapters and rebase/conflict preflight reads.
Merge-tree structured output and rebase diagnostics now use bounded capture;
R-03 and R-05 are locally complete on Unix, verified on Linux. Windows still has
explicit unbounded-capture fallbacks. The migrated local-write runner (R-06)
has no automatic deadline/cancellation for hooks, signing or inherited pipes.
Status and patch scans each have their own limits; this is not one overall
operation deadline or a bound on every Git command.
Rust policy and CI definitions are now on `development`; scheduled upstream
discovery/watchdog execution still needs observation.

## Consolidation and alpha acceptance

The foundation implementation was merged in [PR #9](https://github.com/leopi99/desktop-rust/pull/9)
on 2026-11-26; see [review order and boundaries](consolidation.md). The
`script/rust-ui-fixture.py` prepares four isolated offline repositories for
changes/staging, branches/stash, merge conflict and paused rebase. Initial Git
states, read-only inspection and refusal to overwrite existing roots are verified.
It leaves fixtures available and never labels preparation as interactive success.
[Manual acceptance A-01–A-10](alpha-acceptance.md) remains pending; UI-01/UI-02 and
the alpha completion estimate are unchanged. Review/merge and release readiness
are separate outcomes. [Persisted evidence](evidence/quality-foundations/README.md)
records the frozen revision, fresh local gates and four curated own-window images.

## Evidence and confidence

The last merged runtime batch passed 233 Rust tests and 32 upstream tests, and
its available/missing-Git layouts were inspected at 960×640 and 1200×800 on Linux.
The merged foundation batch passed 408 Rust tests, 32 upstream tests and 10 policy
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
Line controls are bounded to 64 KiB/1,024 changed rows. The later EOF follow-up
below enables safe partial choices with independent old/new stream checks.
Broader formats, implicit editing/joining and full interactive/platform QA remain
open; no parity checkpoint is advanced.
Validated partial EOF selection ([7b8d969153](https://github.com/leopi99/desktop-rust/commit/7b8d9691535b25a5ffed79c111f616cfef73ab11), 2026-11-26) passed 377 Rust tests
(including 107 UI state/capture-option tests), 32 upstream and 10 compiler/policy
tests, plus formatter/Clippy/rustdoc/npm lint. Four new Git regressions cover all
replacement subsets for terminated/unterminated old/new sides in both directions,
partial edits before unchanged EOF context with LF/CRLF, cross-hunk offsets and
newline-only/empty-file transitions. The matrix checks exact index/working bytes
for accepted cases and an unchanged basis for refused implicit newline insertion.
Existing EOF UI regression now verifies selectable changed rows while marker and
oversized-review controls remain unavailable. Partial selections in no-newline
hunks were captured and visually inspected in both directions at 960×640 and
1200×800 with unchanged fixture state during each phase (`/tmp/desktop-rust-eof-selection-cgv3ur9f`). The general
minimum-size smoke also passed (`/tmp/desktop-rust-eof-smoke-960`). Captures do not
replace interaction/accessibility or native platform QA. Ambiguous line joining
is deliberately refused; broader contracts and full baseline analysis remain open.
Working-file reader hardening ([d3d72cc76f](https://github.com/leopi99/desktop-rust/commit/d3d72cc76fa2731cc814511254782dc8cdf551d7), 2026-11-26) passed 386 Rust tests
(including 107 UI state/capture-option tests), 32 upstream and 10 compiler/policy
tests plus formatter/Clippy/rustdoc/npm lint. Four unit regressions verify exact
read/probe budgets even for an endless growing stream, late I/O failure, invalid
relative paths, symlink ancestor/leaf refusal and non-regular file handling. Five
new Git regressions cover empty/text/binary/exact/oversized previews, full staging
after a refused preview, symlink target labels, root/subdirectory equivalence,
native names and stale parent replacement with unchanged user data. The initial
metadata check no longer authorizes an unbounded filesystem read. Existing
conflict/hunk regression coverage runs through the same reader, retaining 8 MiB.
The general minimum-size panel smoke passed with unchanged fixtures
(`/tmp/desktop-rust-working-files-smoke-960`); no UI layout changed in this increment.
Ancestor checks remain preflight; no descriptor-relative fencing, regular/network
filesystem deadline or native Windows leaf guarantee is claimed. R-07 is locally
complete in this scope; G-01 and platform verification remain open.
Partial regular additions ([0ee72609c1](https://github.com/leopi99/desktop-rust/commit/0ee72609c1cfb8b3e94f787c1f04a3a00b7a2bbd), 2026-11-26) passed 394 Rust tests
(including the unchanged 107 UI state/capture-option tests), 32 upstream tests and
10 compiler/policy tests, plus formatter, Clippy, warning-free rustdoc and npm lint.
Seven new Git regressions cover index-byte-preserving review without intent entries,
unborn first commits with selected content, partial/all-row unstaging, retained
executable mode, quoted/native/space paths, CRLF/UTF-8/EOF bytes, stale inputs,
unsupported forms, limits and preservation of other staged paths/divergent work.
One subprocess regression distinguishes quiet no-index differences from failures.
Follow-up `0dc9d3c5fe` explicitly selects the untracked record of a recreated HEAD
path; its focused regression and all-target/all-feature Clippy also pass.
Untracked paths must be absent from HEAD/index. Empty additions, explicit intent
entries and recreated tracked paths retain full-file staging. Deleted/renamed,
conversion and special-file contracts remain outside this scope.
Both selection directions were captured and visually inspected at 960×640 and
1200×800, with exact fixture signatures unchanged during each phase
(`/tmp/desktop-rust-new-file-ui-bnjgrc9m`). The eight-panel/three-failure minimum-size
smoke also passed with unchanged data (`/tmp/desktop-rust-new-file-smoke-960`).
The root UI protocol remains 91 variants; existing selection, draft and freshness
invalidation behavior is reused. Captures do not replace interaction/accessibility
or native platform QA. G-02 and full-baseline analysis remain open; no upstream
checkpoint advances.
Partial regular deletions ([e462bf5c48](https://github.com/leopi99/desktop-rust/commit/e462bf5c481400b456498da80aa6370dc341a7e8), 2026-11-26) passed 401 Rust tests
(including 107 existing UI state/capture-option tests), 32 upstream and 10
compiler/policy tests, plus formatting, Clippy, warning-free rustdoc and npm lint.
Seven new real-Git regressions cover partial staging/commit and continued
unstaging, missing working files, native/quoted/space names, exact CRLF/EOF bytes,
executable modes, stale inputs, source/patch limits, unsupported formats and
conversions, index lock failure/retry and removed staged additions. The unsupported
source regression was extended with deleted symlinks and rerun separately; final
all-target/all-feature Clippy and all 107 UI tests also pass.
Missing working files stay absent through both index directions. Complete source
reads use the index, or fixed HEAD for a staged deletion, with an 8 MiB limit and
UTF-8/NUL validation. Complete patches remain capped at 512 KiB/128 hunks and line
controls at 64 KiB/1,024 changed rows. Empty deletions and recreated staged-deletion
paths use full-file/external-tool workflows; broader conversion/special-file
contracts and interactive/platform QA remain open.
Both deletion selection directions were captured and visually inspected at
960×640 and 1200×800, with unchanged fixture signatures and the file still absent
(`/tmp/desktop-rust-deletion-ui-qodc_4_g`). The eight-panel/three-failure smoke
passed at 960×640 with unchanged data (`/tmp/desktop-rust-deletion-smoke-960`).
No root UI variant was added; selection invalidation and draft preservation reuse
the existing protocol. G-02 and full baseline analysis remain open; no upstream
analysis/parity checkpoint advances.
Working edits at staged rename destinations ([8dd2e70301](https://github.com/leopi99/desktop-rust/commit/8dd2e70301f2d6c172808593e25578f254d65e46), 2026-11-26) passed
408 Rust tests (including 107 existing UI state/capture-option tests), 32 upstream
and 10 compiler/policy tests, plus formatter, Clippy, warning-free rustdoc and npm
lint. Seven regressions cover single/multiple hunks and independent added/deleted
rows, prior staged changes, commits/other index paths, recreated working sources,
missing destinations, native/quoted paths, CRLF/EOF and executable mode, stale
inputs/policy, unsupported content, forged origins and index lock failure/retry.
The existing pure-rename regression (`ef88102f1f`) now explicitly checks index-R
and unchanged working status before refusing an unstaged hunk review.

Only observed index-R / working-M-or-D paths enter this extension. The existing
index-to-working diff and cached patch target the destination entry. The source
entry remains absent, and any independent working file recreated at its old name
stays intact. Full destination deletion removes its index entry as well, leaving
the original staged deletion. Staged-side rename review, partial unstaging of the
move/content, copies and working-tree renames remain outside this scope. Existing
conversion/mode, byte/row, freshness, selection invalidation and draft rules apply.

Modified and missing rename destinations were captured and visually inspected at
960×640 and 1200×800 with unchanged fixture signatures and the original path still
absent (`/tmp/desktop-rust-staged-rename-ui-0a6jonyd`). The eight-panel/three-failure
minimum-size smoke passed with unchanged data
(`/tmp/desktop-rust-staged-rename-smoke-960`). No root UI variant was added. These
captures do not close interaction/accessibility or native platform QA. G-02 and
the full upstream baseline remain open; no checkpoint advances.
No successful test count substitutes for the required source and human review.

On 2026-11-24 the owner reported Actions minutes exhausted until December 2026.
Later checks on PR #9's final head showed hosted Rust, npm lint, CI Lint and CodeQL
success; macOS jobs did not start due runner/account limits and Windows was queued.
This does not establish all-platform CI or scheduled discovery/watchdog execution.
Use the complete local Rust gate, npm lint and ledger/tests for follow-up evidence,
and recheck hosted execution on the relevant revision.

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
