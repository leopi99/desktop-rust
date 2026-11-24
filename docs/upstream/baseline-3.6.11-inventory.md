# Initial full-baseline inventory: release-3.6.11

Source commit: `0102f49aa6c8c554de558299f31b6129b502a2fe`. Rust observation: `49a9de727eebdb487654a63d846872d2912822f5`.
Updated: 2026-11-24. This is a partial source inventory for U-01. The complete
baseline remains pending-analysis; analyzed/parity checkpoints stay unset.
The separate [release interval triage](release-3.6.11.md) remains valid.

## Scope of this pass

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

## Full-baseline coverage grid

All rows remain open. Partial helper inspection is not a completed feature audit.
Feature identifiers match `upstream/features.json`.

| Feature IDs | Inventory state | Next evidence |
| --- | --- | --- |
| repository-init | Partial helper/test inspection; G-04 locally ported | UI callers, preference errors/settings, directory validation, platform behavior. |
| conflict-resolution | Partial router/enum inspection | Delegated helpers, binary/deletion/add-add matrix, callers, original tests. |
| repository-read, diff, stage-commit, commit-history | Pending | Status/diff/index contracts, hunk selection, parsing, limits and original tests. |
| branches, stash | Pending | Supported operations, worktrees, tracking, recovery, original tests. |
| remote-settings, remotes, remotes-auth | Pending | URLs/config, transports, accounts, credentials, cancellation and tests. |
| local-merge, linear-rebase, merge-rebase-conflicts | Pending | Preview/execute/recovery contracts, sequencer state, UI integration and tests. |
| lfs-submodules, enterprise-pull-requests, advanced-integrations | Pending | Feature surface, dependencies, platform and authentication requirements. |
| git-runtime, git-distribution, release-packaging | Pending full inventory | Engine capabilities, native integrations, updates/signing; interval triage covers only the release delta. |

For each slice, collect immutable source/test references, caller-shaped contracts,
Git argument/status/config semantics, platform dependencies and corresponding Rust
implementation/test evidence. Record gaps in the backlog; classify ledger records
with the required source and Rust review evidence before advancing checkpoints.
