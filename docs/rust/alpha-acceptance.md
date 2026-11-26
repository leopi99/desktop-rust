# Linux alpha acceptance

Updated: 2026-11-26. This is an executable manual test plan, not a record of
completed interaction testing. PR #9 merged the engineering foundations; the next
batch is `quality/linux-alpha-usability`. Neither declares a distributable Linux
alpha or upstream parity.

## Prepare disposable repositories

From the checkout, build the pinned Rust preview and choose a fresh directory
whose parent already exists:

```sh
cargo build --locked --offline -p desktop-rust
python3 script/rust-ui-fixture.py --output /tmp/desktop-rust-alpha-session
python3 script/rust-ui-fixture.py --output /tmp/desktop-rust-alpha-session --launch changes
```

The helper creates four independent repositories with repository-local test
identity, hooks/signing disabled, no remotes and isolated Git configuration.
It ignores inherited `GIT_*` overrides during preparation, inspection and launch.
It refuses an existing output root, including symlinks, before running Git there.
It never cleans up or resets a repository. A failed preparation leaves its partial
fixture for inspection; use a fresh root to retry. Launch keeps the app open until
you close it, without an automatic verdict or interaction timeout.

`manifest.json` records the builder hash, initial HEAD/refs/status/unmerged entries
and SHA-256 digests of index, config and working files. Conflict fixtures also
record the state before starting their merge/rebase. `interactive_checks` remains
`not-run`: preparing fixtures does not complete any of the checks below.

Inspect a fixture before/after each action without changing it:

```sh
python3 script/rust-ui-fixture.py --output /tmp/desktop-rust-alpha-session --inspect changes
```

Replace `changes` with `branches-stash`, `merge-conflict` or `rebase-conflict` to
inspect or launch another case. Use a newly prepared root for each alternative
continue/abort path; do not reset a fixture containing edits you want to retain.
The helper does not constrain paths selected later in the UI: keep tests within
these disposable repositories.

## Acceptance cases

Record the source SHA, OS/session/renderer, scale, size, action sequence, observed
result, preserved content and evidence path. Mark pass only after actual keyboard
or pointer interaction. Automated state/Git tests and resting captures are
separate evidence.

| ID | Fixture | Actions | Required observation |
| --- | --- | --- | --- |
| A-01 | changes | Enter summary/description; navigate every panel; return and refresh. Try Tab/Shift+Tab, Enter/Space on sidebar buttons, selection, copy/paste and non-ASCII input. Scroll away from a field and focus it again; repeat with long repository/branch labels. Check focus/selection when changing forms and when controls appear/disappear. | Drafts survive navigation/refresh; focus is visible and editable controls are reachable. Record navigation/IME failures explicitly. |
| A-02 | changes | Review `tracked.txt`; select one replacement pair; stage; commit with a unique message. Inspect before/after. | Only the selected block and existing staged rename enter the commit. The other edit, new file, deleted file and `keep-untracked.txt` stay available. Commit includes the entire index, not only the selected file. |
| A-03 | changes | On a fresh fixture, stage/unstage selected rows of `new.txt` and `delete.txt`; edit and partially stage `renamed.txt`. | Index bytes match the selection; working bytes/absence stay intact; the old rename source is not recreated. Wrong-side actions are unavailable. |
| A-04 | changes | Open a review; modify its file with an editor; apply the old selection. Retry after fresh review. | Stale selection is refused; new edits and draft survive; fresh review recovers. |
| A-05 | branches-stash | Create/switch a branch; rename a selected non-current branch; cancel a deletion review. | Ref/selection labels refresh correctly; cancellation deletes nothing; `keep-untracked.txt` survives. |
| A-06 | branches-stash | Review the saved stash; apply it while retaining the entry. Modify tracked content and save a second stash. | Expected tracked bytes return; retained stash is still listed; new save refreshes the list; untracked preservation is explicit. |
| A-07 | merge-conflict | Review both sides; cancel one choice; choose a side explicitly; continue the merge. | Cancellation writes nothing. Confirmed choice stages only that file; continue creates a two-parent merge commit; `keep.txt` and `keep-untracked.txt` survive. |
| A-08 | merge-conflict | On a fresh root, inspect then confirm Abort merge. | Original branch/HEAD and tracked baseline return; unrelated untracked content survives; obsolete reviews disappear. |
| A-09 | rebase-conflict | Read side labels; choose an explicit side; continue. Repeat Abort on a separate fresh root. | Rebase side meanings are clear; continue completes replay; abort restores the original branch/HEAD. Unrelated user files and drafts survive both paths. |
| A-10 | all | Repeat relevant flows at 960×640, 1200×800 and a real HiDPI scale; scroll long lists; close/reopen. | Actions remain reachable, labels readable, focus predictable; record renderer/session limitations and recovery failures. |

For successful continue operations, inspect history/refs as well as status.
For refused/failed operations, compare index/files with the immediately preceding
inspection, allowing only your deliberate external edits. Do not treat a clean
status alone as proof that the right content was committed.

## Current results

| Evidence | Status |
| --- | --- |
| Four repositories prepared with expected staged/unstaged and conflict states | Verified locally on 2026-11-26. |
| Read-only inspection and refusal to overwrite an existing root | Verified locally; initial manifest and every fixture signature unchanged. |
| Automated Rust/Git/policy checks and own-window smoke | Recorded separately in [status](status.md) and the consolidation evidence. |
| Text-input Tab/Shift+Tab traversal | Implemented in the usability branch; automated Iced subscription/widget/state checks pass. Native key input, other controls and accessibility remain pending. |
| A-01 through A-10 actual interactive execution | Not run; UI-01/UI-02 remain open. |
| Sidebar focus/activation | Implemented; mixed traversal, release/repeat/interruption guards, disabled/retargeted controls, mouse/touch and late busy delivery have automated widget coverage. Programmatic focus-ring captures visually checked at both sizes; native keyboard session pending. |
| Focus reveal in scroll views | Implemented for annotated inputs/sidebar controls with default top-left anchors. Six real-widget regressions and before/after own-window captures pass; native keyboard and non-default anchor behavior remain unverified. |
| Field identity/handover | Current inputs have typed purpose/context IDs. Automated widget/event checks cover redraw preservation, retarget/reorder reset and blocked typing into a replacement field. Native panel transitions and restoration across removed fields remain pending. |
| Hunk/line checkbox keyboard selection | Implemented; five automated widget/event regressions cover Tab traversal, Space release/repeats, exact-review/coordinate invalidation, interruption and selection-only delivery. Enter does not apply or toggle. Actual native keyboard/accessibility execution remains pending. |
| Changed-file/read-review keyboard navigation | Implemented; five new real-widget/event regressions cover read-only actions, identity handover, native paths, pointer/touch, busy delivery and draft/review rules. File diff, hunk review and conflict previews are reachable through Tab and Enter/Space; write actions remain separate. Actual native A-01–A-03 execution remains pending. |
| Native Windows/macOS, full accessibility/IME/HiDPI verification | Not run. |

## Alpha exit conditions

Before declaring a Linux alpha, complete A-01 through A-10 with reproducible
results, fix data-preservation/focus/recovery blockers, decide the supported Git
runtime and installation path, and document unsupported operations. Review R-06
(stalled local writes), G-01 (external-client races), G-02/G-05 (format/conflict
boundaries), AUTH-01 and DIST-01/DIST-02 in [backlog](backlog.md).
In-app accounts remain a later remote-alpha milestone. Packaging/signing and
cross-platform support are not established by this local engineering batch.
