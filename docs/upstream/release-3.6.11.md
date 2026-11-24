# Initial triage of release-3.6.11

Source: [release-3.6.11](https://github.com/desktop/desktop/releases/tag/release-3.6.11),
commit `0102f49aa6c8c554de558299f31b6129b502a2fe`.
This triage compares `release-3.6.10..release-3.6.11`. It does **not** complete
the inventory of the full initial baseline source tree. The release remains
`pending-analysis`, with `analyzed` and `parity` unset.

## Inspected interval

The complete interval contains four commits and changes only four files:
`app/package.json`, `app/yarn.lock`, `changelog.json` and `.github/workflows/ci.yml`.
No application Git logic or tests changed in this interval. Release notes identify
Git 2.56.1 as the improvement; dependency manifests update dugite 3.3.2 to 3.3.3.
The original macOS hotfix jobs move from macos-14-xlarge to macos-15-xlarge.

| Item | Source | Rust disposition |
| --- | --- | --- |
| Bundled Git | [dependency update](https://github.com/desktop/desktop/commit/ee3e0297e109a560d9ae1fefef14f008b112457b) | Deferred against planned `git-distribution`; system Git is external to the current Rust package. |
| Hotfix runner migration | [workflow update](https://github.com/desktop/desktop/commit/0102f49aa6c8c554de558299f31b6129b502a2fe) | Deferred against planned `release-packaging`; Rust macOS distribution is not implemented. |
| Version and release-note metadata | Commits recorded in the ledger | Not applicable to the independent Rust preview version; retain original Electron reference files. |

`upstream/state.json` contains the corresponding source links and reasons. These
records classify the small release interval, not every behavior inherited before
it. The planned statuses must not be used to infer full application parity.

## Runtime visibility

The Rust Git panel reports the version returned by the executable resolved through
PATH and an actionable error if it cannot be started. The probe uses `git --version`
without opening a repository, runs on a worker thread, and can be retried. This
provides visibility into the external-engine policy; it does not install Git,
pin a binary, enforce an approved version, or prove behavior/security compatibility.
Vendor and development version suffixes are retained instead of guessed away.
Later operations resolve Git again, so PATH or executable changes can invalidate
the observation. On Unix, the diagnostic now enforces a five-second deadline and
512/1024-byte stdout/stderr caps with process-group cleanup; it is tested on Linux.
Windows equivalent bounds remain pending native process-tree/pipe cancellation.
These resource safeguards do not change the deferred bundled-engine disposition.
Packaging and a supported-version/capability policy need separate work.

## Still required for the initial baseline

Inventory supported workflows in the full stable source, its test coverage and
dependencies; compare those contracts with the Rust implementation and record
remaining behavioral differences, including authentication and distribution.
Then assess whether the analysis checkpoint can advance. No such advancement is
part of this increment.
