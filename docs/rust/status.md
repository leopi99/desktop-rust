# Rust rewrite status

Updated: 2026-11-24. This is the implementation/milestone index, not an upstream
parity attestation. Use [backlog](backlog.md) for remaining task IDs,
[quality](quality.md) for engineering rules and `upstream/state.json` for release
checkpoints. Detailed usage/limitations remain in [the rewrite guide](../rust-rewrite.md).

## Consolidated work

PRs #1–#5 introduced the foundation, history, UI panels and branch rename/deletion.
PR #6 consolidated the stack onto `development`; #7 persisted the detected
upstream baseline; #8 added external Git runtime diagnostics and interval triage.
All eight are merged. Independent prototype features do not imply release parity.

| Area | Implemented and checked | Remaining boundary |
| --- | --- | --- |
| Workspace | core/Git/Iced split; Cargo lockfile; Rust/npm CI | Quality policy in the current batch still needs its batch merge. |
| Local repositories | init, status, staged/unstaged diffs, full-file stage/unstage, commits | Hunk selection, large-repository virtualization, richer diff rendering. |
| Local history/branches | Fixed-tip history paging, committed diffs, create/switch, current rename, reviewed merged-branch deletion | Rename other branches, stronger concurrent-client guarantees, advanced history editing. |
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
The current reliability batch passed 287 Rust tests, 32 upstream tests and 10 policy
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
The deletion-review capture shows a clipped Create and switch button at the right
edge (UI-03). Temporary captures/report are in `/tmp/desktop-rust-ui-domains-960`;
they are local evidence and do not automate interaction or accessibility testing.

The offline smoke fixture verifies its files, refs, index and config remain
unchanged. In addition to normal/missing Git it now captures a stalled executable
and an endlessly noisy executable on Unix. Both error layouts were inspected at
960×640 and 1200×800 on Linux/software rendering. It does not automate button clicks
or establish accessibility parity. Native Windows deadline/tree cleanup remains
pending.
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
