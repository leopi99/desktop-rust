# Rust quality standard

This standard applies to the Rust implementation under `crates/`, all workspace
targets and future first-party crates. The Electron sources retain their own
TypeScript contribution rules. It supplements the root contribution and PR policy.

## Required gate

Install Rust through rustup and Python 3.11 or newer, then run from any directory:

```sh
python3 /path/to/desktop-rust/script/check_rust.py
```

From the repository root, `python3 script/check_rust.py --offline` uses cached
Cargo dependencies. Initial toolchain/dependency installation needs network access.
The script finds Cargo on PATH or in the rustup installation and selects the
pinned compiler explicitly. `cargo test` alone is not the quality gate.

The required sequence is workspace policy validation, policy regression tests,
rustfmt, all-target/all-feature Clippy, workspace tests and warning-free rustdoc.
The same entry point runs in the Rust CI workflow. npm lint and upstream ledger
checks remain separate required checks for a batch, per the root instructions.
CI configuration on a branch becomes the default branch's policy when merged;
required-check/branch-protection settings are separate GitHub configuration.

## Automatically enforced rules

| Rule | Enforcement |
| --- | --- |
| Reproducible compiler | `rust-toolchain.toml` pins 1.100.0 with rustfmt and Clippy; check commands select that channel explicitly. |
| Tested minimum version | Workspace `rust-version` matches the pinned compiler; package settings are inherited by every member. |
| Every crate participates | The manifest guard uses Cargo's actual member list and rejects unregistered manifests under `crates/` and missing/local lint overrides. |
| Formatting | `cargo fmt --all -- --check`, with stable options in `rustfmt.toml`. |
| No lint warnings | Workspace Clippy `all = deny` and the required `-D warnings`, including tests and all features. |
| No explicit production panic shortcuts | `unwrap_used`, `expect_used`, `panic`, `todo` and `unimplemented` forbidden, including local attribute overrides. Return/propagate recoverable errors instead. |
| No stray diagnostics | `dbg_macro` forbidden; `print_stdout` and `print_stderr` denied, with narrow documented CLI exceptions. |
| Safe code by default | `unsafe_code` and `unsafe_op_in_unsafe_fn` denied; unavoidable OS FFI needs a scoped expectation and SAFETY comments. |
| Public API contracts | Missing rustdoc, missing public Debug implementations and missing `# Errors`/`# Panics` documentation denied; rustdoc warnings fail the gate. |
| Respect results/visibility | `unused_must_use` and `unreachable_pub` denied. |
| Accountable exceptions | Allow attributes denied; expectations need a reason and obsolete/unfulfilled expectations fail compilation. |

These are engineering checks, not proof of correctness, security or parity. A
contributor who can modify the policy can also weaken it; changes to lint tables,
toolchains, exceptions and CI require explicit review. Do not bypass the gate with
compiler cap-lints, alternate wrappers or changed checks.

## Exceptions and tests

The forbid-level panic/unwrap/expect/todo/unimplemented/debug rules cannot be
overridden by an expectation. For other deny-level rules, fix the violation
before considering an exception. Use the smallest item/block
with `#[expect(lint_name, reason = "specific invariant or boundary")]`; never
relax the workspace or a whole production crate to make a check pass. An
expectation that stops matching a violation must be removed, not silenced.

The existing production exceptions are Unix process-group signalling in
`crates/git/src/process.rs`, explicit developer CLI help, and capture-failure
stderr reporting. Unsafe blocks explain the memory/FFI safety contract separately
from broader process-lifecycle behavior. Unix-only expectations are conditional
so they do not become unfulfilled on other platforms.

`clippy.toml` permits fail-fast assertions only in explicit test contexts. Unit
modules use `#[cfg(test)]`; integration test crates declare `#![cfg(test)]`.
This covers their fixture helpers without enabling unwrap/panic in production.
Tests still obey formatting, all other applicable lints and useful documentation.

Policy regression tests compile deliberately nonconforming programs and require
specific compiler/Clippy diagnostics. They also check permitted test assertions
and justified CLI output, and reject new crates that omit lint inheritance.

## Rules requiring code review

- Keep `desktop-core` independent of UI/process execution, `desktop-git` responsible
  for Git adapters, and Iced state/view logic separate. Do not grow large handlers
  as the default design; extract cohesive operations with explicit state transitions.
- Prefer explicit data models and ownership, borrowed inputs where appropriate,
  exhaustive enums and small functions. Use Result for recoverable failures and
  preserve useful error kinds/context; do not use error-message matching as logic.
- Document observable contracts and failure cases, not restatements of syntax.
  Add examples when useful; keep bounded-preview and concurrency limitations clear.
- Keep native paths/ref names for writes, full object IDs and literal Git argument
  boundaries. No shell construction from repository/user data, forced operations,
  silent overwrites, helper execution or lossy write-path conversion.
- Preserve user work on failure. Validate state before writes, invalidate stale
  reviews, keep drafts, and await child termination before releasing write state.
  The process mutex and Git locks do not by themselves fence external clients.
- Keep blocking processes/filesystem work out of UI event handling. Bound process
  lifetime/output where the operation requires it and test cancellation/recovery.
  Known legacy gaps are tracked in the backlog rather than called compliant.
- Dependencies need a purpose, platform/maintenance assessment and lockfile review.
  Do not add a dependency to avoid a small, clear standard-library implementation.
- Tests assert behavior and data preservation, especially negative/stale/native-path
  cases. Use isolated fixtures; never perform destructive tests on the working repo.
  Visual captures supplement state tests and do not prove keyboard/accessibility QA.

Review destructive Git operations, authentication/trust boundaries, unsafe adapters
and release/supply-chain changes in depth as required by the PR skill. Automated
checks do not waive that review or advance upstream parity checkpoints.

UI transitions live in `crates/ui/src/update/`: keep the top-level dispatcher
exhaustive and domain event matches exhaustive, with explicit ignored variants
for failed guards. Do not add a catch-all that silently absorbs new events. Keep
completion/refresh draft and review invalidation together, and move cohesive
helpers with their domain. Box large successful response payloads when required
by enum-size checks; do not clone snapshots/reviews just to route a message.
Exercise existing state regressions for routing changes and add behavioral tests
when changing a guard or transition, rather than tests of file organization.
Form inputs in reused view slots use `focus_region::field` with a typed `FieldId`
for their purpose/panel context. Keep identities independent of editable draft
text; shared draft storage does not imply the same input context. Preserve native
edit state on ordinary redraws and reset it when the slot changes purpose.

## Updating the policy

Change the pinned compiler, workspace minimum and CI installer together, then run
the complete gate and inspect the new lint output. Raise the minimum only when
that compiler has been tested. Prefer curated rules with clear failure modes over
blanket activation of unstable formatter options or every pedantic/restriction lint.
All-feature testing assumes features can coexist; introducing mutually exclusive
features requires a reviewed explicit test matrix, not removing coverage silently.

Keep [status](status.md) and [backlog](backlog.md) current in the same batch.
