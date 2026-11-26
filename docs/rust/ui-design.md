# Rust desktop UI direction

Updated: 2026-11-26. The reference is GitHub Desktop's workspace structure and
interaction language. The fork keeps its own name and supported behavior; visual
similarity does not establish upstream parity or alpha readiness.

## First visual pass

Implemented at [095959c636](https://github.com/leopi99/desktop-rust/commit/095959c636e86c15666d518b5b51042e9e52437e).

The light workspace uses a dark repository/branch/remote toolbar, compact
Changes/History tabs, a fixed 292 px file/commit column and a flexible diff area.
Commit inputs stay below the file list. Actions belong to the selected file rather
than repeating beneath every list entry. History uses matching compact lists.
Repository opening moves to Create / Clone and the empty-repository welcome view;
the toolbar navigates to repository, branch and remote management. Footer links
keep saved changes, repository setup and Git diagnostics reachable.

The common shell, palette and read-only diff renderer are separate modules.
Normal diffs have old/new line gutters and addition/deletion backgrounds; Routine file/index header lines and empty sections are omitted. Metadata-like code inside
hunks stays visible. Display is bounded to 2,000 source lines and 4,096 Unicode
characters per line with explicit clipping notices. These previews cannot
exercise partial-write authority: hunk/line application still requires the
existing complete immutable review and explicit action.

Selected-file state is presentation only. Existing read requests/results set it;
refresh, repository changes, panel navigation and review dismissal revoke it.
Native paths and original rename paths stay in FileChange values. Existing busy,
stale-review, draft and confirmation rules remain in place. Header/row controls
reuse the focusable navigation widget; contrast adapts to dark/selected surfaces.
There is no new keyboard shortcut for Git writes.

## Visual evidence

Own-window captures on Linux/tiny-skia, visually inspected at 960×640 and
1200×800, include all eight smoke panels at minimum size, Changes/History at both
sizes, hunk/conflict reviews, clean state and long toolbar labels. Fixtures are
isolated; file/index/config/status/ref signatures remain unchanged. No actual
native keyboard/accessibility session is claimed.

- [Changes at 960×640](evidence/desktop-layout/changes-960.png)
- [Changes at 1200×800](evidence/desktop-layout/changes-1200.png)
- [History at 1200×800](evidence/desktop-layout/history-1200.png)
- [Line review at 960×640](evidence/desktop-layout/line-review-960.png)

## Next visual milestones

1. Match remaining Desktop details: icons, tab indicators, spacing, resizable panes,
   hover/path affordances and context menus. Keep labels honest about actions.
2. Add a genuine multiline commit description and improve commit/history detail
   presentation. Port file-inclusion semantics deliberately; do not disguise
   stage/unstage as upstream's commit-selection checkbox.
3. Replace the remaining management forms with focused dialogs/dropdowns and
   a proper repository chooser. Preserve confirmations and recovery diagnostics.
4. Add dark/system theme selection and verify long/localized labels, HiDPI, IME,
   accessibility and complete keyboard/pointer flows on native sessions.

The current toolbar opens management panels; it does not yet implement upstream's
dropdown menus. Fetch/pull/push remain explicit in Remotes. Commit still includes
the entire index; the description remains a single-line input. History's branch
creation form, specialized hunk controls and settings screens remain prototype
flows. Side-by-side diff, syntax highlighting, image previews and full Desktop
menu/dialog behavior are future work, not implied by these captures.
