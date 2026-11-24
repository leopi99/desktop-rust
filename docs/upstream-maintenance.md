# Maintaining the Rust fork against stable upstream releases

## Weekly discovery

The weekly `Upstream stable discovery` workflow validates the committed ledger,
runs the Python tests and builds an artifact containing a proposed `state.json`,
`report.md`, `check.json` and per-release source patches and commit lists.
Discovery never commits changes or advances the analysis/parity checkpoints.
Reports are published atomically only after all requests, diffs and checks succeed.
Git objects are fetched into an isolated temporary bare database. The development
checkout, its index, refs and FETCH_HEAD are not used for upstream evidence.

To run locally, use a new output directory:

```sh
python3 script/upstream/registry.py
python3 script/upstream/sync.py --output /tmp/desktop-rust-weekly-new
```

The initial baseline is the last published non-draft, non-prerelease numeric
stable release. Ties are ordered by version. Historical releases before this
baseline are not imported as new work. Later publications are tracked even when
their version is lower, so maintenance backports are not lost. Backports compare
to the previous version on the same major/minor line when one exists. New release
lines compare to the greatest lower stable version. If no predecessor exists,
the release is marked as needing an initial inventory.

Registered releases must remain present in the upstream API and retain the same
commit SHA. A missing release, moved tag, empty stable response, API failure or Git
failure stops the run without publishing new state. Fix the cause and rerun; do
not move checkpoints to conceal a failed check. The discovery report is evidence
for review, not a completed classification or a claim of Rust parity.

## Ledger and review gates

`upstream/state.json` uses schema version 1 and fixes the source repository to
`desktop/desktop`. Each release contains `tag`, 40-character commit `sha`, official
release `url`, timezone-qualified `published_at`, and `status` (`pending-analysis`
or `analyzed`). An analyzed release additionally requires a nonempty `assessment`
describing the source/test/dependency review, including when no porting is needed.
An optional `base` records the comparison tag and commit SHA.

The `detected`, `analyzed` and `parity` checkpoints are null or objects containing
`tag` and `sha` of a registered release. Detection identifies the last published
registered release. Analysis cannot skip an unclassified registered release or
change. Checkpoint order follows publication time, with version as a tiebreaker,
so a later backport does not rewrite the history of an earlier assessment.

Each entry in `changes` requires these fields:

| Field | Meaning |
| --- | --- |
| `id` | Unique, stable identifier within the ledger |
| `release` | Registered release tag containing the change |
| `feature` | ID from `upstream/features.json` |
| `upstream` | Nonempty list of upstream commit SHAs or GitHub commit/PR URLs |
| `status` | `pending-analysis`, `pending-implementation`, `deferred`, `implemented`, or `not-applicable` |
| `rust_pr` | Required fork PR URL when implemented |
| `evidence` | Nonempty test evidence descriptions/links when implemented |
| `reason` | Required explanation for deferred or non-applicable work |

A parity checkpoint requires analysis through the same publication and no
unresolved work for `prototype` or `supported` features. Deferred work for a
`planned` feature must stay in the ledger with a reason; this checkpoint only
covers active features, not full application parity. The validator checks evidence
presence and checkpoint consistency; humans must assess whether the evidence is
sufficient and whether all relevant changes have been inventoried.

For each discovery artifact, inspect its source diffs, original tests, release
notes and dependency changes. Classify behavior changes and carry the proposed
release records into a reviewed ledger update. Port each behavior with regression
tests, then add the Rust PR and actual validation evidence. Validate before review:

```sh
python3 script/upstream/registry.py
python3 -m unittest discover -s script/upstream -v
```

Follow AGENTS.md and the prepare-team-pr skill when preparing team-authored PRs.
No PR is created automatically yet. Security and data-loss fixes can be handled
outside the weekly cadence without bypassing review or losing their source links.

## Daily watchdog and activation

The daily `Upstream discovery watchdog` workflow uses read-only Actions API access.
It fails if discovery is disabled, has never succeeded on the default branch, or
its most recent successful run started more than eight days ago. Recent failed
runs never count as successful checks. API failures also fail the watchdog.
The signal is a failed Actions run; it does not post issues or messages.

Deploy both workflows to the fork's default branch, enable scheduled Actions and
manually dispatch discovery once. A manual successful discovery resets the same
freshness check. Both workflows are currently repository files, not remotely
activated infrastructure. GitHub can disable scheduled workflows in forks or after
inactivity, so a watchdog inside Actions cannot detect the suspension of all Actions.
See [GitHub workflow activation rules](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disable-and-enable-workflows)
and [the workflow runs API](https://docs.github.com/en/rest/actions/workflow-runs).

Local watchdog inspection uses `GH_TOKEN` if provided:

```sh
python3 script/upstream/watchdog.py --repository leopi99/desktop-rust --branch development
```

Use the actual default branch if it changes. The watchdog does not create schedules,
change repository settings, publish PRs, or modify the ledger.
