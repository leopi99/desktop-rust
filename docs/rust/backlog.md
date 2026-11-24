# Rust rewrite backlog

Updated: 2026-11-24. Tasks describe outcomes and acceptance evidence; they are not
promises of current support. Update statuses within the working batch and keep
[status](status.md) and the upstream ledger consistent. `Done` below means locally
implemented/verified; deployment still depends on merging the batch.

## Batch policy

Use a milestone branch for coherent work, with independent commits and one PR for
the completed batch rather than one PR per feature. Target `development` directly
when possible. If a dependency genuinely requires stacked branches, retarget the
next PR after its parent merge and verify which commits actually reached the
default branch. Never infer consolidation just because all intermediate PRs say merged.

Security/data-loss fixes can use a small urgent PR. Keep unrelated changes out of
a batch. PR publication and merging still follow user authorization and the
repository PR skill; a batch is not permission to merge or waive review.

## Current batch: quality foundations

| ID | Status | Acceptance |
| --- | --- | --- |
| Q-01 | Done locally | Every workspace crate inherits shared package/lint values; a new/unchecked crate is rejected by the guard. |
| Q-02 | Done locally | Toolchain/minimum/formatter are fixed; production panic shortcuts and undocumented public contracts fail the gate. |
| Q-03 | Done locally | One command runs compiler-policy probes, all-target/all-feature Clippy, tests and warning-free rustdoc; CI uses it. |
| Q-04 | Done locally | Status, backlog, Rust-specific agent instructions and exception/review rules are documented. |

## Next increment in the active batch: runtime and process hardening

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| R-01 | High | Give the startup/version probe a finite deadline and bounded captured output; test a stalled and a noisy executable, retry and clean termination. The current probe runs off the UI thread but buffers output and has no separate deadline. |
| R-02 | High | Define resource bounds for historical Git output before preview truncation; test large messages/diffs and bounded memory behavior. |
| R-03 | High | Consolidate common child-process cleanup/error handling without weakening Git argument boundaries or transfer cancellation; preserve all existing fixture behavior. |
| A-01 | Medium | Split the large UI update handler into cohesive domain handlers while keeping write guards, draft retention and review invalidation covered by existing state tests. |

Continue from the active `quality/rust-foundations` branch for the next compatible
hardening increments, keeping commits independently reviewable. Review process
changes and UI refactoring in depth. Set a concrete milestone scope before opening
the batch PR; do not combine the entire backlog into one PR.

## Upstream inventory and synchronization

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| U-01 | High | Inventory the full initial 3.6.11 stable source, supported workflow contracts, original tests and dependencies. Record concrete differences/evidence per active feature before advancing analysis. Interval triage alone is insufficient. |
| U-02 | High | Observe actual weekly discovery and daily watchdog runs on the default branch; review and persist each proposed detection record with immutable source links. |
| U-03 | High | Port classified active-feature differences with regression tests and source/Rust PR links. Advance parity only when ledger gates and substantive review are satisfied. |
| U-04 | Medium | Decide whether discovery should publish a draft review proposal automatically; retain permission/ownership checks, atomic artifacts and separate analysis/parity gates. |
| U-05 | Medium | Add external monitoring for total Actions suspension; the in-Actions watchdog cannot detect its own global suspension. |

Reserve upstream capacity in each batch. Escalate security and user-data
preservation changes ahead of cosmetic work. Do not suppress missing/moved tags
or API failures to produce a healthy report.

## Local alpha and interaction verification

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| G-01 | High | Review and strengthen freshness/ref handling against concurrent external clients where feasible, preserving Git's own worktree/merge safeguards and the documented race boundary. |
| UI-01 | High | Exercise real keyboard navigation, focus, IME, HiDPI and accessibility on Linux; record failures and evidence. Existing resting-layout screenshots do not close this task. |
| UI-02 | High | Run complete interactive commit, branch, stash, merge and rebase recovery flows on disposable repositories; verify data preservation and meaningful failure states. |
| G-02 | Medium | Add hunk selection and richer diff rendering with exact staged/unstaged semantics and large-file limits. |
| G-03 | Medium | Add rename of other selected branches and further local-history operations in explicit, tested scopes. Force deletion remains unsupported until deliberately designed/reviewed. |

## Remote alpha and delivery

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| AUTH-01 | High before remote alpha | Specify in-app authentication/account flows, OS credential storage, SSH behavior and Enterprise scope; no embedded secrets or silent import of original app databases. |
| DIST-01 | High before packaging | Decide system-versus-bundled Git distribution, supported versions/capabilities and update/security policy. A displayed version does not implement upstream's Git bundle upgrade. |
| DIST-02 | Medium | Create Linux packages and a fork-specific release/update pipeline with provenance, signing policy and explicit artifact verification. |
| PLAT-01 | Before Windows support | Add and test native child-process-tree cancellation; current Windows behavior kills only the direct child. |
| PLAT-02 | Later | Implement native Windows/macOS adapters, signing, installer/update checks and platform UI QA before claiming delivery support. |
| ADV-01 | Later | Plan LFS, submodules, tags, cherry-pick, interactive rebase, Enterprise, pull requests and multiple accounts as scoped milestones. |

## Completion rule

Close a task only with implemented behavior and relevant evidence, naming the
commit/PR and remaining limitations. Upstream `implemented` records additionally
need the source links, Rust PR and regression evidence required by the ledger.
Keep blocked/deferred reasons visible. New discoveries belong here even when they
make the remaining scope larger; never hide them by changing a status label alone.
