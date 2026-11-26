# Rust implementation rules

Read [the quality standard](../docs/rust/quality.md) before changing Rust code.
It supplements the root contribution/PR instructions and applies to every crate
and target here. TypeScript-specific conventions are not Rust style rules.

- Inherit workspace package values and `[lints] workspace = true`; register new
  crates as workspace members. Keep external dependencies explicit and justified.
- Production code must pass the pinned toolchain, rustfmt, all-target/all-feature
  Clippy with warnings denied, tests and warning-free rustdoc through
  `python3 script/check_rust.py`.
- Use typed errors/results and checked external data. Do not use unwrap, expect,
  panic, todo, unimplemented or debug/terminal prints in production control flow.
  Forbid-level shortcuts cannot be overridden by local lint attributes.
- Keep unsafe forbidden by default. A required OS adapter needs a narrow
  `#[expect(unsafe_code, reason = "...")]`, a SAFETY comment on each unsafe block,
  regression coverage and human review of the invariant. Never relax workspace lints.
- Prefer removing violations. Any unavoidable lint exception uses the smallest
  item/expression, `#[expect(..., reason = "...")]`, and a concrete invariant.
  Do not add blanket allow attributes or suppress unfulfilled expectations.
- Give public APIs, fields and variants useful rustdoc. Fallible public functions
  document their failure conditions under `# Errors`; do not invent guarantees.
- Keep repository paths and ref names in native encoding. Use typed Git arguments,
  literal pathspecs, full object IDs, explicit write targets and freshness checks.
  Never introduce shell interpolation, force/overwrite behavior or lossy write paths.
- Keep blocking Git/filesystem work out of Iced event handlers. Preserve drafts,
  invalidate stale reviews, handle cancellation and bound displayed diagnostics.
- Unit tests belong in `#[cfg(test)]` contexts; integration test crates declare
  `#![cfg(test)]`. Fail-fast assertions in those contexts are permitted by Clippy
  configuration, not by relaxing production lints.
- Test observable behavior and preservation of user data, including stale/negative
  cases. Do not add tests that only mirror internal implementation details.
- Update the status/backlog and upstream evidence for the batch. A prototype,
  a green test, a captured layout and reviewed parity are distinct states.
