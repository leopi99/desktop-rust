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
| Engine | External Git version/error reports; Unix probe deadline and stream caps, tested on Linux | Windows probe/process-tree bounds; historical-output bounds; bundled Git/update/capability policy. |
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

The batch is engineering groundwork, with no additional feature PR opened for
each item. Keep commits independently reviewable and group compatible follow-up
work before the next milestone PR. The active branch stays `quality/rust-foundations`
for this reliability batch; continue from it rather than dropping its unmerged
policy by starting again from `development`. Runtime/process hardening is in
progress; historical-output bounds and broader child-lifecycle work are the next
coherent increments. Default-branch CI activation follows the batch merge.

## Evidence and confidence

The last merged runtime batch passed 233 Rust tests and 32 upstream tests, and
its available/missing-Git layouts were inspected at 960×640 and 1200×800 on Linux.
The current reliability batch passed 242 Rust tests, 32 upstream tests and 10 policy
tests, including deliberately rejected compiler examples and inheritance/opt-out
checks for future crates. Clippy, formatting, warning-free rustdoc and npm lint
also passed. Record fresh results for later behavior changes; these counts are
dated observations, not a permanent coverage guarantee.

The offline smoke fixture verifies its files, refs, index and config remain
unchanged. In addition to normal/missing Git it now captures a stalled executable
and an endlessly noisy executable on Unix. Both error layouts were inspected at
960×640 and 1200×800 on Linux/software rendering. It does not automate button clicks
or establish accessibility parity. Native Windows deadline/tree cleanup remains
pending.
No successful test count substitutes for the required source and human review.

## Upstream checkpoint

Detected: `release-3.6.11`, commit `0102f49aa6c8c554de558299f31b6129b502a2fe`.
The 3.6.10-to-3.6.11 interval is triaged in
[the baseline note](../upstream/release-3.6.11.md): three ledger records distinguish
Git distribution, original hotfix runners and upstream release metadata.
The initial **full baseline** remains pending-analysis; analyzed/parity are unset.
Discovery and watchdog manual success do not prove future scheduled execution.

## Keeping this index useful

Update this file when a batch changes implementation status, evidence or scope.
Move remaining work through backlog task IDs and link the closing commit/PR.
Keep precise source references/checkpoints in the ledger instead of duplicating
release classifications here. Do not mark a task complete because its plan exists.
