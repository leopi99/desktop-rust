//! Read-only, bounded patch presentation. Display rows never authorize Git writes.
use crate::{appearance, Message};
use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length};

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Section,
    Added,
    Removed,
    Context,
    Hunk,
    Notice,
}

#[derive(Debug)]
struct DiffRow {
    old: Option<u64>,
    new: Option<u64>,
    text: String,
    kind: Kind,
}

pub(super) fn view(detail: &str) -> Element<'_, Message> {
    if !detail.starts_with("Staged changes\n") {
        return scrollable(
            container(text(detail).font(iced::Font::MONOSPACE).size(12)).padding(16),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .into();
    }
    let mut contents = column![].width(Length::Fill);
    for line in rows(detail) {
        if line.kind == Kind::Section || line.kind == Kind::Notice {
            contents = contents.push(
                container(text(line.text).size(12).color(appearance::MUTED))
                    .padding([10, 16])
                    .width(Length::Fill)
                    .style(appearance::sidebar),
            );
            continue;
        }
        let color = match line.kind {
            Kind::Added => appearance::ADDED,
            Kind::Removed => appearance::DELETED,
            Kind::Hunk => appearance::SIDEBAR_BG,
            Kind::Context | Kind::Section | Kind::Notice => iced::Color::WHITE,
        };
        let content = row![
            text(line.old.map(|n| n.to_string()).unwrap_or_default())
                .size(11)
                .color(appearance::MUTED)
                .width(42),
            text(line.new.map(|n| n.to_string()).unwrap_or_default())
                .size(11)
                .color(appearance::MUTED)
                .width(42),
            text(line.text)
                .font(iced::Font::MONOSPACE)
                .size(12)
                .width(Length::Fill),
        ]
        .spacing(8);
        contents = contents.push(
            container(content)
                .padding([3, 12])
                .width(Length::Fill)
                .style(move |_| appearance::surface(color)),
        );
    }
    scrollable(contents)
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn rows(detail: &str) -> Vec<DiffRow> {
    let mut output = Vec::new();
    let (mut old, mut new) = (None, None);
    let mut in_hunk = false;
    let mut skip = false;
    let mut lines = detail.lines().enumerate().peekable();
    while let Some((index, line)) = lines.next() {
        if index == 2000 {
            output.push(DiffRow { old: None, new: None, text: "Preview limited to 2,000 source lines. Use hunk review for supported partial selection.".into(), kind: Kind::Notice });
            break;
        }
        let (left, right, kind) = if line == "Staged changes" || line == "Working tree changes" {
            (old, new, in_hunk) = (None, None, false);
            skip = lines
                .peek()
                .is_some_and(|(_, next)| next.trim() == "(none)");
            if skip {
                continue;
            }
            (None, None, Kind::Section)
        } else {
            if skip || line.is_empty() {
                continue;
            }
            if line.starts_with("diff --git ") {
                (old, new, in_hunk) = (None, None, false);
                continue;
            }
            // A changed source line may itself begin with --- or +++. Only
            // hide file metadata outside a hunk, never patch content.
            if !in_hunk
                && (line.starts_with("index ")
                    || line.starts_with("--- ")
                    || line.starts_with("+++ "))
            {
                continue;
            }
            if line.starts_with("@@ ") {
                let mut parts = line.split_whitespace();
                let _ = parts.next();
                old = parts.next().and_then(|p| coordinate(p, '-'));
                new = parts.next().and_then(|p| coordinate(p, '+'));
                in_hunk = old.is_some() && new.is_some() && parts.next() == Some("@@");
                if !in_hunk {
                    (old, new) = (None, None);
                }
                (None, None, Kind::Hunk)
            } else if in_hunk && line.starts_with('+') {
                (None, take(&mut new), Kind::Added)
            } else if in_hunk && line.starts_with('-') {
                (take(&mut old), None, Kind::Removed)
            } else if in_hunk && line.starts_with(' ') {
                (take(&mut old), take(&mut new), Kind::Context)
            } else {
                (None, None, Kind::Context)
            }
        };
        let mut display: String = line.chars().take(4096).collect();
        if display.len() < line.len() {
            display.push_str(" … [display line shortened]");
        }
        output.push(DiffRow {
            old: left,
            new: right,
            text: display,
            kind,
        });
    }
    output
}

fn coordinate(value: &str, prefix: char) -> Option<u64> {
    value.strip_prefix(prefix)?.split(',').next()?.parse().ok()
}

fn take(counter: &mut Option<u64>) -> Option<u64> {
    let current = *counter;
    *counter = current.and_then(|n| n.checked_add(1));
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_like_source_lines_remain_visible_with_correct_separate_patch_coordinates() {
        let input = "Staged changes\n(none)\n\nWorking tree changes\ndiff --git a/file b/file\nindex abc..def 100644\n--- a/file\n+++ b/file\n@@ -10,2 +20,2 @@\n--- removed source\n+++ added source\n same\n\\ No newline at end of file\n@@ -40 +50 @@\n-old\n+new\n";
        let visible = rows(input);
        assert_eq!(
            visible.iter().filter(|r| r.kind == Kind::Section).count(),
            1
        );
        let removed = visible
            .iter()
            .find(|r| r.text == "--- removed source")
            .unwrap();
        assert_eq!(
            (removed.old, removed.new, &removed.kind),
            (Some(10), None, &Kind::Removed)
        );
        let added = visible
            .iter()
            .find(|r| r.text == "+++ added source")
            .unwrap();
        assert_eq!(
            (added.old, added.new, &added.kind),
            (None, Some(20), &Kind::Added)
        );
        let context = visible.iter().find(|r| r.text == " same").unwrap();
        assert_eq!((context.old, context.new), (Some(11), Some(21)));
        assert!(visible
            .iter()
            .any(|r| r.text == "\\ No newline at end of file"));
        assert_eq!(
            visible.iter().find(|r| r.text == "-old").unwrap().old,
            Some(40)
        );
        assert_eq!(
            visible.iter().find(|r| r.text == "+new").unwrap().new,
            Some(50)
        );
        assert!(!visible.iter().any(|r| r.text == "--- a/file"));
    }

    #[test]
    fn preview_bounds_keep_unicode_valid_and_announce_clipping_without_overflowing_numbers() {
        let input = format!(
            "Staged changes\n@@ -18446744073709551615 +0,0 @@\n-{}\n{}",
            "è".repeat(5000),
            " context\n".repeat(2500)
        );
        let visible = rows(&input);
        let long = visible.iter().find(|r| r.kind == Kind::Removed).unwrap();
        assert_eq!(long.old, Some(u64::MAX));
        assert!(long.text.ends_with("[display line shortened]"));
        assert!(long.text.starts_with("-èè"));
        assert!(visible
            .iter()
            .filter(|r| r.kind == Kind::Context)
            .all(|r| r.old.is_none()));
        assert_eq!(visible.last().unwrap().kind, Kind::Notice);
        assert!(visible.len() <= 2001);
    }
}
