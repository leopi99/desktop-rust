# Rust rewrite backlog

Updated: 2026-11-26. Tasks describe outcomes and acceptance evidence; they are not
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

| ID | Priority | Status | Outcome and acceptance |
| --- | --- | --- | --- |
| R-01 | High | Done locally on Unix; verified on Linux | [ca5687095f](https://github.com/leopi99/desktop-rust/commit/ca5687095f945e896fb1d0d7f5c6d54078aa04ff): five-second probe deadline, 512/1024-byte stdout/stderr caps, fail-closed overflow and joined readers. Tests cover stalled/noisy commands, inherited pipes, process/helper termination and retry. Windows remains covered by PLAT-01. |
| R-02 | High | Done locally on Unix; verified on Linux | [5b24281183](https://github.com/leopi99/desktop-rust/commit/5b2428118380ab36e38237c72db729bfa76e550e): 30-second per-child deadlines, 16 KiB stderr cap, complete metadata/file-list caps of 1/4 MiB and 10,000 files. Message/patch capture retains 64/256 KiB prefixes while checking final exit status. Tests cover multi-megabyte commits, unchanged repository data, oversized subjects/lists, late errors and stalled/noisy readers. Git internal memory is outside these capture bounds; Windows remains PLAT-01. |
| R-03 | High | Done locally on Unix; verified on Linux | Shared child setup/termination and an opaque managed-child type now serve transfers, Unix probes and Unix historical reads. In [3e574c3f75](https://github.com/leopi99/desktop-rust/commit/3e574c3f755b0d4c16f4df1f2bdfa1813ad6a8b9), a shared reader pair adds fallible thread startup, child cleanup on either startup failure and unconditional joining of both readers, including panic/I/O errors. Transfer regressions cover inherited pipes, timeout/cancellation recovery and both diagnostic tails. Tracked patch reads now share this lifecycle in R-04. The final merge/rebase paths are migrated in [56594e0180](https://github.com/leopi99/desktop-rust/commit/56594e0180813563538a9ef204c2cf63df32d0d2); every Unix Git subprocess uses shared setup and reader ownership. Stalled-write deadline/cancellation policy remains R-06; Windows fallbacks/tree cleanup remain PLAT-01. |
| R-04 | High | Done locally on Unix; verified on Linux | [52eb1e12b6](https://github.com/leopi99/desktop-rust/commit/52eb1e12b66bd137a7280fa66a868513075c2623): tracked staged/unstaged patches reuse the bounded read-only capture with a 256 KiB prefix per side, 30-second per-patch deadline and 16 KiB stderr cap. Multi-megabyte tests verify unchanged index/worktree, full-file staging after clipping, unborn HEAD and native paths with helpers disabled. Selection/status reads are migrated in R-05; Windows capture remains PLAT-01. |
| R-05 | High | Done locally on Unix; verified on Linux | [e6ecdaa16d](https://github.com/leopi99/desktop-rust/commit/e6ecdaa16d54b9a6e7f25caf63ad14ad4d8b708e) bounds core snapshot/status, index freshness, HEAD/symbolic refs, metadata paths and local-branch listing. Raw caps are 8 MiB status, 16 MiB index, 4 MiB branches and 64 KiB refs/paths; Read-only Unix subprocesses have 30-second deadlines and 16 KiB stderr caps. Quiet empty exit 1 is the only accepted absent-ref result; missing commit objects, signals, other exits, overflow and timeout propagate. Tests cover oversized index/status/branch scans, refused commit/staging, unchanged data, recovery and malformed/missing HEAD objects. Additional migration in [80fd340d1c](https://github.com/leopi99/desktop-rust/commit/80fd340d1cb196074f15b693cc91d9f844238e36): remote metadata (64 KiB per read, 64 remotes and 64 URLs per endpoint), stash lists/path streams (4 MiB each), collision review (10,000 path records, one complete tracked-path scan), and branch config/worktree guards (4 MiB each). Stash third-parent read errors propagate. Tests verify rejected edits/transfers/restores/rename, preserved data and recovery. Remaining reads migrated in [2e273b3ba9](https://github.com/leopi99/desktop-rust/commit/2e273b3ba9012156d56231f1a417da52d68b7c3d): conflict attributes, shared ancestry/common-base queries, rebase plan/path/ref reads; the legacy generic reader is removed. Replay review is capped at 1,000 commits, collision reads at 4 MiB per stream / 8 MiB total / 10,000 records, tracked paths at 16 MiB and ref/attribute reads at 64 KiB. Initialization now inspects Git markers/layouts directly and propagates filesystem errors instead of trusting failed discovery. Tests cover exact replay boundary, oversized/aggregate checkout review and damaged bare layouts with unchanged data. The final object-writing merge-tree protocol is bounded in [56594e0180](https://github.com/leopi99/desktop-rust/commit/56594e0180813563538a9ef204c2cf63df32d0d2): 4 MiB stdout / 64 KiB stderr, drained to completion and rejected on overflow without accepting a prefix. This driver-capable command has no automatic deadline; stalled-write policy remains R-06. Windows read-only fallbacks still check limits after buffering and lack equivalent deadlines (PLAT-01). |
| R-06 | High | In progress | [bff5464a57](https://github.com/leopi99/desktop-rust/commit/bff5464a577538c8fe5c6f57c20019c71a2b1ff7) migrates `run_write` to the managed child/reader pair, starts both readers before feeding stdin, closes stdin before waiting, and retains only 64 KiB diagnostic tails per stream with explicit markers. Git failure diagnostics take precedence over a broken input pipe; success with incomplete input still fails. Tests cover multi-megabyte simultaneous input/output, empty input, early stdin closure, noisy real hooks, complete commit messages, preserved staged/unstaged data and retry after hook rejection. In [56594e0180](https://github.com/leopi99/desktop-rust/commit/56594e0180813563538a9ef204c2cf63df32d0d2), rebase also uses marked 64 KiB diagnostic tails with final status preserved; merge-tree uses complete bounded raw capture. Tests cover exact raw protocol limits/status, both overflow streams with child completion, verbose real merge drivers, paused rebase and abort. Runner migration is complete; automatic deadline/cancellation for hooks/signing/drivers and inherited pipes remains deliberately unimplemented. |
| A-01 | Medium | Done locally; verified on Linux | [2f79f69b3d](https://github.com/leopi99/desktop-rust/commit/2f79f69b3d307da920f7dbae0175a3d9711af3a1) replaces the large update match with exhaustive routing of 74 messages to ten typed domain handlers. Nine support methods move with their domains; large successful responses are boxed without cloning payloads. All 82 existing UI state tests pass, and extraction review confirms preserved guards/bodies. Offline smoke captures eight panels plus three Git diagnostic failures at 960×640 with unchanged fixture data; four captures visually inspected. This does not establish interactive/accessibility QA. |

Continue from the active `quality/rust-foundations` branch for the next compatible
hardening increments, keeping commits independently reviewable. Review process
changes and UI refactoring in depth. Set a concrete milestone scope before opening
the batch PR; do not combine the entire backlog into one PR.

## Upstream inventory and synchronization

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| U-01 | High | Inventory the full initial 3.6.11 stable source, supported workflow contracts, original tests and dependencies. [The inventory](../upstream/baseline-3.6.11-inventory.md) now contains pinned initialization/default-branch and manual-conflict slices (five complete files plus selected config functions), with G-04 now locally addressed and G-05 still open. The coverage grid remains open; interval triage and these partial slices do not advance analysis. |
| U-02 | High | Observe actual weekly discovery and daily watchdog runs on the default branch; review and persist each proposed detection record with immutable source links. Owner reports Actions minutes exhausted until December 2026: use local weekly discovery meanwhile and verify scheduled execution after quota returns. Local discovery on 2026-11-24 at 16:32 UTC found no new stable releases. |
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
| UI-03 | Medium | Done locally in [49a9de727e](https://github.com/leopi99/desktop-rust/commit/49a9de727eebdb487654a63d846872d2912822f5): branch create/rename forms use full-width vertical layout so action labels fit. Offline smoke passed at 960×640 and 1200×800 with unchanged fixtures; normal/deletion-review branch captures visually inspected at both sizes. Full interactive QA remains UI-01/UI-02. |
| G-02 | Medium | In progress: [78115619bd](https://github.com/leopi99/desktop-rust/commit/78115619bd5830ce7e46b3cc636312f3c1f89fbe) implements single-hunk staging for existing regular UTF-8 text modifications, with complete 512 KiB/128-hunk review, 8 MiB working-content bound, stale-input checks and index-only apply. Display prefixes cannot authorize writes; working bytes, prior staged changes and drafts are preserved in regressions. Conversions, new/deleted/renamed/special/binary files remain full-file/external-tool scope. Remaining: hunk unstaging, line and multiple-hunk selection, broader formats/conversion contracts, richer rendering and full interactive/platform QA. |
| G-03 | Medium | Selected non-current branch rename done locally in [dcf0589562](https://github.com/leopi99/desktop-rust/commit/dcf0589562e9d68b3fce55a40e0e908f5093894c), verified on Linux: observed name/OID/current flag, source basis, occupied/orphan configuration and worktree guards; native source names, moved tracking/reflog and preserved current HEAD/index/files. Shared checks also serve current-branch rename. UI retains drafts on failure and cannot redirect a busy action. Branch creation at an observed full commit ID is implemented in [f676e0f9f8](https://github.com/leopi99/desktop-rust/commit/f676e0f9f89aae53ae1026ec7f71de600eb55a6c), without checkout/tracking and with destination/configuration/operation/freshness guards. Remaining: advanced history editing, native platform/interactive QA and case-only rename policy. No forced rename/deletion fallback. |
| G-04 | Medium | Done locally in [aa524c1101](https://github.com/leopi99/desktop-rust/commit/aa524c11013d4bdd6cef71509823f33be73df662), verified on Linux: blank UI input resolves global Git `init.defaultBranch`, falling back to main only for a quiet absent setting. Explicit branch override skips the preference query. During preference resolution, invalid/empty/non-UTF-8/oversized values, malformed config and missing Git fail before destination reservation. Thirteen cases in isolated child processes verify branch creation and untouched temporary global configuration; the real user configuration is never modified by tests. Existing explicit API remains available; preference setter/settings UI stays separate scope. |
| G-05 | Medium | In progress: read-only side previews in [c0fcf39b95](https://github.com/leopi99/desktop-rust/commit/c0fcf39b9565013f925603f604d614cfe4dabca1), followed by confirmed regular-file choices in [ba0696b3c5](https://github.com/leopi99/desktop-rust/commit/ba0696b3c57a19c34c5156a82a46f0ef509ac47f). Separate write review binds full working/selected bytes and executable mode (8 MiB), index/HEAD/sequencer, effective attributes and relevant settings. Explicit Apply/Cancel, native paths, text/binary/add-add/deletion tests, stale-input refusal and checkout-success/staging-failure recovery are implemented locally. Conversions/filters, symlinks, submodules and ambiguous renames require external Git. Recovery/status follow-up in [a083d7f84f](https://github.com/leopi99/desktop-rust/commit/a083d7f84f890144afa6edeaa6cc1324de8ce5a9) adds conservative UU/AA/UD/DU write guards, partial-state context for all post-write errors and complete multi-conflict merge/rebase continue/abort regressions. Focused original status/merge/rebase test inspection is recorded in the inventory. Remaining: coordinated rename and conversion/special-file contracts, complete caller/fixture/UI audit, platform and interactive QA. |

## Remote alpha and delivery

| ID | Priority | Outcome and acceptance |
| --- | --- | --- |
| AUTH-01 | High before remote alpha | Specify in-app authentication/account flows, OS credential storage, SSH behavior and Enterprise scope; no embedded secrets or silent import of original app databases. |
| DIST-01 | High before packaging | Decide system-versus-bundled Git distribution, supported versions/capabilities and update/security policy. A displayed version does not implement upstream's Git bundle upgrade. |
| DIST-02 | Medium | Create Linux packages and a fork-specific release/update pipeline with provenance, signing policy and explicit artifact verification. |
| PLAT-01 | Before Windows support | Add/test native child-process-tree and inherited-pipe cancellation, then enable bounded runtime and historical capture on Windows. Transfers currently kill only the direct child; the version-probe and historical-read fallbacks still buffer output without their own deadlines. |
| PLAT-02 | Later | Implement native Windows/macOS adapters, signing, installer/update checks and platform UI QA before claiming delivery support. |
| ADV-01 | Later | Plan LFS, submodules, tags, cherry-pick, interactive rebase, Enterprise, pull requests and multiple accounts as scoped milestones. |

## Completion rule

Close a task only with implemented behavior and relevant evidence, naming the
commit/PR and remaining limitations. Upstream `implemented` records additionally
need the source links, Rust PR and regression evidence required by the ledger.
Keep blocked/deferred reasons visible. New discoveries belong here even when they
make the remaining scope larger; never hide them by changing a status label alone.
