# Rust foundations consolidation

Updated: 2026-11-26. Target: `quality/rust-foundations` into `development`.
This milestone consolidated the accumulated engineering work in
[PR #9](https://github.com/leopi99/desktop-rust/pull/9), merged on 2026-11-26 at
[e496be487e](https://github.com/leopi99/desktop-rust/commit/e496be487e164bcc5be61654dbe514ccfa0e4a99). The merge tree matches the final published foundation branch.
The record below retains the review scope and evidence; merge status does not
close native acceptance tasks or attest upstream parity. Follow-up usability work
is on `quality/linux-alpha-usability`. This is a prototype milestone, not an alpha
release.

## Scope and review order

The existing independent commits are retained for review and selective reversion.
Review the complete diff against the actual PR base, in this order:

| Area | Review focus | Main implementation/evidence |
| --- | --- | --- |
| Rust policy | Workspace inheritance, pinned compiler, negative policy checks, CI installing the same compiler; no weakened lint exceptions. | Cargo manifests, `script/check_rust*.py`, `docs/rust/quality.md`. |
| Unix process ownership | Child groups, kill/reap, inherited pipes, fallible reader startup/joining and narrowly scoped unsafe invariants. | `crates/git/src/process*`, runtime/history/read-output adapters and process regressions. |
| Reads and local writes | Complete raw limits versus display prefixes, late failures, concurrent stdin/stdout/stderr, refusal to authorize writes from clipped data. | History, snapshots, remotes/stash/rebase preflight, merge-tree and working-file readers. |
| UI routing | Exhaustive domain events, busy guards, refresh/error recovery, draft preservation and invalidation of reviews. | `crates/ui/src/update/`, existing state tests and selected own-window captures. |
| Guarded Git behavior | Native paths/OIDs, selected branch operations, confirmed conflict choices, line/hunk patches and no unintended working writes. | Branch/conflict/hunk APIs and real-Git regressions; pinned upstream slices in the inventory. |
| Operational acceptance | Isolated offline fixtures, exact expectations and unverified manual cases. | [Alpha acceptance](alpha-acceptance.md), fixture helper and consolidation evidence. |

## Required human review

This batch requires `needs-deep-review`: index/ref writes, conflict replacement and
recovery can lose user data if their guards or postconditions are wrong. Review
process-group unsafe adapters and compiler/CI policy as trust/supply-chain
boundaries as well. A qualified human must examine the invariants, failure
windows and tests; an author's assessment and automated checks do not replace
that review. No automatic merge is part of this milestone.

## Explicit remaining boundaries

- Unix bounds are verified on Linux; Windows capture/process-tree parity is open.
- Local hooks/signing/drivers have bounded retained diagnostics but no automatic
  write deadline/cancellation. External clients can race preflight; no global
  atomicity or rollback-over-user-work guarantee is claimed.
- Working-file ancestor checks do not fence component-renaming races; regular
  filesystem I/O has no deadline, and Windows leaf handling remains best effort.
- Partial staging supports scoped regular text, not all filters/encodings,
  special files, coordinated renames/copies or every conflict form.
- Native keyboard/focus/IME/HiDPI/accessibility and complete interactive recovery
  are still pending. Fixture creation and captures do not close UI-01/UI-02.
- Accounts, secure credential adapters, packaged delivery, signing/updates and
  advanced integrations remain later milestones.
- Full upstream baseline analysis is pending. Detected stays at `release-3.6.11`
  / `0102f49aa6c8c554de558299f31b6129b502a2fe`; analyzed/parity stay unset.
- The owner reported Actions quota limits. PR #9's hosted Rust/lint/CodeQL later
  succeeded, while macOS jobs could not start and Windows remained queued at
  inspection. Scheduled discovery/watchdog and all-platform CI remain unverified.

## Evidence discipline

Record the exact tested implementation/tooling SHA and actual outcomes. A later
commit containing only these notes/screenshots does not change the tested Rust
code. Keep complete local logs and fixture manifests available; commit only a
small curated set of own-window screenshots as PR review evidence, not automated
pixel baselines. Never publish credentials or real-project data.

The PR preserved the repository template and its `Team-authored review` section.
The author's assessment was received before marking it ready; GitHub now records
it as merged. The review requirements above describe the batch's risk class;
merge status is recorded separately from verification. Open acceptance tasks
remain visible rather than being marked done by merging an engineering milestone.

[Persisted local verification evidence](evidence/quality-foundations/README.md) binds
the tested revision to command outcomes, capture integrity and selected images.
