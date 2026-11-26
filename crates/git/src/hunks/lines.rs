//! Selection-aware context/count reconstruction; patch bytes remain lossless.
use desktop_core::{HunkLine, HunkReview, HunkSide};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn coordinate(value: &str) -> io::Result<(i64, usize)> {
    let (start, count) = value
        .get(1..)
        .ok_or_else(|| invalid("Invalid hunk range"))?
        .split_once(',')
        .unwrap_or((value.get(1..).unwrap_or_default(), "1"));
    let start = start.parse().map_err(|_| invalid("Invalid hunk range"))?;
    let count = count.parse().map_err(|_| invalid("Invalid hunk range"))?;
    Ok((start, count))
}

pub(super) fn patch(review: &HunkReview, selection: &[HunkLine]) -> io::Result<String> {
    if !review.line_selection_available() {
        return Err(invalid(
            "Line selection requires at most 64 KiB and 1,024 changed rows; use complete hunks",
        ));
    }
    if selection.is_empty() || selection.len() > desktop_core::MAX_LINE_SELECTION_ROWS {
        return Err(invalid(
            "Select unique changed lines before updating the index",
        ));
    }
    let mut grouped: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for row in selection {
        let hunk = review
            .hunks
            .get(row.hunk)
            .ok_or_else(|| invalid("Invalid selected hunk"))?;
        let line = hunk
            .split_inclusive('\n')
            .nth(row.line)
            .ok_or_else(|| invalid("Invalid selected line"))?;
        if !matches!(line.as_bytes().first(), Some(b'+' | b'-'))
            || !grouped.entry(row.hunk).or_default().insert(row.line)
        {
            return Err(invalid(
                "Select unique added or deleted lines, not headers, context or markers",
            ));
        }
    }
    let mut patch = review.header.clone();
    let mut delta = 0i64;
    for (hunk_index, selected) in grouped {
        let hunk = &review.hunks[hunk_index];
        super::validate_hunk(hunk)?;
        let rows: Vec<_> = hunk.split_inclusive('\n').collect();
        let fields: Vec<_> = rows[0].split(' ').collect();
        let (old_start, _) = coordinate(fields[1])?;
        let (new_start, _) = coordinate(fields[2])?;
        let mut body = String::new();
        let (mut old_count, mut new_count) = (0usize, 0usize);
        let mut previous_output = None;
        let (mut old_eof, mut new_eof) = (false, false);
        for (line_index, line) in rows.iter().enumerate().skip(1) {
            let prefix = line.as_bytes()[0];
            if prefix == b'\\' {
                if let Some(output) = previous_output {
                    if output != '+' {
                        old_eof = true;
                    }
                    if output != '-' {
                        new_eof = true;
                    }
                    body.push_str(line);
                }
                continue;
            }
            let chosen = selected.contains(&line_index);
            let output = match (review.side, prefix, chosen) {
                (_, b' ', _) => Some(' '),
                (_, b'+' | b'-', true) => Some(char::from(prefix)),
                (HunkSide::Unstaged, b'-', false) | (HunkSide::Staged, b'+', false) => Some(' '),
                (HunkSide::Unstaged, b'+', false) | (HunkSide::Staged, b'-', false) => None,
                _ => return Err(invalid("Invalid reviewed patch row")),
            };
            previous_output = output;
            if let Some(output) = output {
                if (output != '+' && old_eof) || (output != '-' && new_eof) {
                    return Err(invalid("Selection would append lines after a missing final newline; select the adjoining EOF changes together or use the whole hunk"));
                }
                body.push(output);
                body.push_str(&line[1..]);
                if output != '+' {
                    old_count += 1;
                }
                if output != '-' {
                    new_count += 1;
                }
            }
        }
        let (source_start, source_count, target_count) = match review.side {
            HunkSide::Unstaged => (old_start, old_count, new_count),
            HunkSide::Staged => (new_start, new_count, old_count),
        };
        let locus = source_start
            .checked_add(i64::from(source_count == 0))
            .and_then(|n| n.checked_add(delta))
            .ok_or_else(|| invalid("Hunk coordinate overflow"))?;
        let target_start = locus
            .checked_sub(i64::from(target_count == 0))
            .filter(|n| *n >= 0)
            .ok_or_else(|| invalid("Invalid selected hunk coordinate"))?;
        let (old, new) = match review.side {
            HunkSide::Unstaged => (source_start, target_start),
            HunkSide::Staged => (target_start, source_start),
        };
        patch.push_str(&format!("@@ -{old},{old_count} +{new},{new_count} @@\n"));
        patch.push_str(&body);
        delta = delta
            .checked_add(i64::try_from(target_count).map_err(|_| invalid("Hunk count overflow"))?)
            .and_then(|n| n.checked_sub(i64::try_from(source_count).ok()?))
            .ok_or_else(|| invalid("Hunk count overflow"))?;
    }
    Ok(patch)
}
