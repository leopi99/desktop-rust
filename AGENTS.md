# Working on GitHub Desktop

Follow the repository's [contribution guidance](.github/CONTRIBUTING.md)
and [development instructions](.github/copilot-instructions.md).
These instructions supplement those guides; they do not change contribution
eligibility or merge requirements.

## Preparing team-authored pull requests

Before preparing or updating a team-authored PR, use the
[prepare-team-pr skill](.agents/skills/prepare-team-pr/SKILL.md).
Follow its authorization checks, evidence collection, review assessment,
and ownership requirements.

Read the current [PR template](.github/pull_request_template.md).
Preserve its sections and generate or update the skill's
`Team-authored review` section in the PR body, not in the template.

## Rust rewrite batches

For Rust work, also follow [the Rust agent rules](crates/AGENTS.md) and
[the Rust quality standard](docs/rust/quality.md). All first-party Rust crates
belong to the Cargo workspace and inherit its lint and package policy.
Run `python3 script/check_rust.py` before completing a Rust batch.
Keep [status](docs/rust/status.md) and [backlog](docs/rust/backlog.md) current.
Group coherent changes into milestone batches; prepare one PR for a completed
batch instead of automatically opening a PR per feature. Keep independent commits
within that branch so review and selective reversion remain possible.
