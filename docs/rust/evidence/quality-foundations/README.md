# Consolidation review evidence

Captured and verified locally on 2026-11-26 against implementation/tooling commit
[`2ea9a7665d400e0b5e39c70e0c8ac40710ebbacf`](https://github.com/leopi99/desktop-rust/commit/2ea9a7665d400e0b5e39c70e0c8ac40710ebbacf). Later commits add
only documentation and curated review artifacts, with no Rust/tooling changes.

[Verification summary](validation.txt) and [machine-readable record](validation.json)
record observed results. Full local logs remain under
`.git/pr-preparation/quality-foundations-*` on the verification checkout; they are
not uploaded CI artifacts. Reproduce the Rust gate, npm lint, upstream checks and
own-window smoke commands in the summary using fresh output paths.

The helper [manual fixtures](../../alpha-acceptance.md) have verified initial
states, read-only inspection, existing-root/symlink refusal, missing-Git preflight
and isolated configuration. Separate Git abort checks validate the prepared merge
and rebase cases. They do not execute UI controls or close UI-01/UI-02.

Eight panels plus three Git failures were captured at both 960×640 and 1200×800.
Additional line-selection/conflict-choice captures at both sizes also preserved
the exact fixture state. The four curated images below were visually inspected;
other raw captures/reports stay local. Images contain only generated fixture data
and are review evidence, not automated pixel baselines or proof of accessibility.

| Own-window image | Observed layout |
| --- | --- |
| [Changes](changes-960.png) | Changes list, separate staged/working diff and commit fields. |
| [Branches](branches-960.png) | Branch actions and vertically arranged creation/rename forms. |
| [Selected lines](line-selection-960.png) | Changed-row controls and fixed action footer. |
| [Conflict choice](conflict-choice-960.png) | Explicit replace/stage confirmation, side semantics and cancel action. |

Hosted Actions minutes are unavailable until December per the owner. These are
local results, not a green hosted-CI claim. Native keyboard/focus/IME/HiDPI,
accessibility, complete interactive workflows and Windows/macOS remain pending.
See [scope and review risks](../../consolidation.md) and
[acceptance cases](../../alpha-acceptance.md). Full upstream analysis/parity
checkpoints remain unset.
