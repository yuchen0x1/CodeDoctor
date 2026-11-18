//! Clone matching over normalized line windows. Full hashes locate exact or
//! renamed copies. Two positional bands provide bounded candidates for small
//! edits, with matching normalized lines verified in their original order.

use super::CloneConfig;
use super::token_normalize::normalize_line;
use crate::parse::ParsedFile;
use std::collections::{HashMap, VecDeque};

const WINDOW_HASH_BASE: u64 = 0x9e37_79b1_85eb_ca87;
const CHECKPOINT_INTERVAL: usize = 1024;
const NEAR_CANDIDATES_PER_BAND: usize = 16;

#[derive(Debug, Clone, PartialEq)]
pub struct CloneMember {
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloneKind {
    Exact,
    Similar,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloneGroup {
    pub kind: CloneKind,
    pub similarity: f32,
    pub line_count: u32,
    pub members: Vec<CloneMember>,
}

struct SourceLines<'a> {
    path: &'a str,
    language: &'a str,
    raw: Vec<&'a str>,
    normalized: Vec<u64>,
}

#[derive(Clone, Copy)]
struct Window {
    file: usize,
    start: usize,
    hash: u64,
    bands: [u64; 2],
    near_eligible: bool,
}

impl Window {
    fn position(self) -> (usize, usize) {
        (self.file, self.start)
    }
}

const CONTROL_SIGNALS: &[&str] = &[
    "for", "while", "if(", "if<", "switch", "try", "catch", "except", "elif", "case", "match", "do",
];

fn span_hash(prefix: &[u64], start: usize, end: usize, power: u64) -> u64 {
    prefix[end].wrapping_sub(prefix[start].wrapping_mul(power))
}

fn checkpoint<E>(index: usize, check: &mut impl FnMut() -> Result<(), E>) -> Result<(), E> {
    if index % CHECKPOINT_INTERVAL == 0 {
        check()?;
    }
    Ok(())
}

fn similarity<T: PartialEq>(left: &[T], right: &[T]) -> f32 {
    let equal = left.iter().zip(right).filter(|(a, b)| a == b).count();
    equal as f32 / left.len() as f32
}

fn overlaps(left: Window, right: Window, size: usize) -> bool {
    left.file == right.file && left.start.abs_diff(right.start) < size
}

fn find_windows<'a, E>(
    files: &'a [ParsedFile],
    size: usize,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<(Vec<SourceLines<'a>>, Vec<Window>), E> {
    let mut files: Vec<_> = files.iter().collect();
    files.sort_unstable_by(|a, b| a.file.relative_path.cmp(&b.file.relative_path));
    let mut lines_by_file = Vec::new();
    let mut windows = Vec::new();
    let mut token_ids = HashMap::new();
    let half = size / 2;
    let first_power = WINDOW_HASH_BASE.wrapping_pow(half as u32);
    let second_power = WINDOW_HASH_BASE.wrapping_pow((size - half) as u32);
    let full_power = first_power.wrapping_mul(second_power);

    for parsed in files {
        check()?;
        let raw: Vec<&str> = parsed.text.lines().map(str::trim).collect();
        if raw.len() < size {
            continue;
        }
        let mut normalized = Vec::with_capacity(raw.len());
        let mut rolling_hash = vec![0u64];
        let mut control_lines = vec![0usize];
        let mut substantive_lines = vec![0usize];
        for (index, line) in raw.iter().enumerate() {
            checkpoint(index, check)?;
            let tokens = normalize_line(line);
            let control = CONTROL_SIGNALS.iter().any(|signal| tokens.contains(signal));
            control_lines.push(control_lines[index] + usize::from(control));
            substantive_lines.push(substantive_lines[index] + usize::from(tokens.len() >= 3));
            let next_id = token_ids.len() as u64 + 1;
            let id = *token_ids.entry(tokens).or_insert(next_id);
            normalized.push(id);
            rolling_hash.push(
                rolling_hash[index]
                    .wrapping_mul(WINDOW_HASH_BASE)
                    .wrapping_add(id),
            );
        }

        let file = lines_by_file.len();
        let first_window = windows.len();
        for start in 0..=(raw.len() - size) {
            checkpoint(start, check)?;
            let end = start + size;
            let substantive = substantive_lines[end] - substantive_lines[start];
            if control_lines[end] == control_lines[start] || substantive == 0 {
                continue;
            }
            let middle = start + half;
            windows.push(Window {
                file,
                start,
                hash: span_hash(&rolling_hash, start, end, full_power),
                bands: [
                    span_hash(&rolling_hash, start, middle, first_power),
                    span_hash(&rolling_hash, middle, end, second_power),
                ],
                near_eligible: substantive >= 4,
            });
        }
        if windows.len() > first_window {
            lines_by_file.push(SourceLines {
                path: &parsed.file.relative_path,
                language: &parsed.file.language,
                raw,
                normalized,
            });
        }
    }
    Ok((lines_by_file, windows))
}

fn normalized<'a>(files: &'a [SourceLines<'_>], window: Window, size: usize) -> &'a [u64] {
    &files[window.file].normalized[window.start..window.start + size]
}

fn same_occurrences(existing: &[Window], candidate: &[Window], size: usize) -> bool {
    if existing.len() < candidate.len() {
        return false;
    }
    let anchor = candidate[0];
    let lower_bound = (anchor.file, anchor.start.saturating_sub(size - 1));
    let first = existing.partition_point(|member| member.position() < lower_bound);
    existing[first..]
        .iter()
        .take_while(|member| member.file == anchor.file && member.start < anchor.start + size)
        .any(|member| {
            let offset = member.start as isize - anchor.start as isize;
            candidate[1..].iter().all(|other| {
                other.start.checked_add_signed(offset).is_some_and(|start| {
                    existing
                        .binary_search_by_key(&(other.file, start), |w| w.position())
                        .is_ok()
                })
            })
        })
}

pub(super) fn match_clones<E>(
    files: &[ParsedFile],
    config: &CloneConfig,
    mut check: impl FnMut() -> Result<(), E>,
) -> Result<Vec<CloneGroup>, E> {
    check()?;
    if config.max_groups == 0 {
        return Ok(Vec::new());
    }
    let size = config.min_lines.max(2);
    let (files, mut windows) = find_windows(files, size, &mut check)?;
    check()?;
    windows.sort_unstable_by_key(|window| (window.hash, window.position()));
    let mut exact_groups = Vec::new();
    for (index, bucket) in windows.chunk_by(|a, b| a.hash == b.hash).enumerate() {
        checkpoint(index, &mut check)?;
        if bucket.len() < 2 {
            continue;
        }
        let anchor = bucket[0];
        let tokens = normalized(&files, anchor, size);
        let mut members = vec![anchor];
        for (index, &candidate) in bucket[1..].iter().enumerate() {
            checkpoint(index, &mut check)?;
            if !overlaps(*members.last().unwrap(), candidate, size)
                && tokens == normalized(&files, candidate, size)
            {
                members.push(candidate);
            }
        }
        if members.len() > 1 {
            exact_groups.push(members);
        }
    }

    check()?;
    exact_groups
        .sort_unstable_by_key(|group| (std::cmp::Reverse(group.len()), group[0].position()));
    let mut groups: Vec<Vec<Window>> = Vec::new();
    for (index, candidate) in exact_groups.into_iter().enumerate() {
        checkpoint(index, &mut check)?;
        if !groups
            .iter()
            .any(|group| same_occurrences(group, &candidate, size))
        {
            groups.push(candidate);
        }
        if groups.len() >= config.max_groups {
            break;
        }
    }

    // A small edit confined to one half still shares the other positional band.
    // Cap comparisons for common bands and keep exact/renamed results ahead of
    // the best-effort near-match pass.
    if config.similarity_threshold < 1.0 && groups.len() < config.max_groups {
        collect_near_matches(&files, &mut windows, size, config, &mut groups, &mut check)?;
    }

    groups.sort_unstable_by_key(|group| group[0].position());
    groups
        .into_iter()
        .map(|members| {
            check()?;
            let anchor = members[0];
            let original = &files[anchor.file].raw[anchor.start..anchor.start + size];
            let tokens = normalized(&files, anchor, size);
            let mut exact = true;
            let mut agreement = 1.0f32;
            let mut locations = Vec::with_capacity(members.len());
            for (index, window) in members.into_iter().enumerate() {
                checkpoint(index, &mut check)?;
                let file = &files[window.file];
                exact &= original == &file.raw[window.start..window.start + size];
                agreement = agreement.min(similarity(tokens, normalized(&files, window, size)));
                locations.push(CloneMember {
                    file_path: file.path.to_owned(),
                    start_line: window.start as u32 + 1,
                    end_line: (window.start + size) as u32,
                });
            }
            Ok(CloneGroup {
                kind: if exact {
                    CloneKind::Exact
                } else {
                    CloneKind::Similar
                },
                similarity: agreement,
                line_count: size as u32,
                members: locations,
            })
        })
        .collect()
}

fn collect_near_matches<E>(
    files: &[SourceLines<'_>],
    windows: &mut [Window],
    size: usize,
    config: &CloneConfig,
    groups: &mut Vec<Vec<Window>>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), E> {
    let mut recent: VecDeque<Window> = VecDeque::with_capacity(NEAR_CANDIDATES_PER_BAND);
    for band in 0..2 {
        check()?;
        windows.sort_unstable_by_key(|window| (window.bands[band], window.position()));
        for (index, bucket) in windows
            .chunk_by(|a, b| a.bands[band] == b.bands[band])
            .enumerate()
        {
            checkpoint(index, check)?;
            if bucket.len() < 2 {
                continue;
            }
            recent.clear();
            for (index, &window) in bucket.iter().enumerate() {
                checkpoint(index, check)?;
                if !window.near_eligible {
                    continue;
                }
                for &previous in &recent {
                    if previous.hash == window.hash
                        || files[previous.file].language != files[window.file].language
                        || overlaps(previous, window, size)
                        || similarity(
                            normalized(files, previous, size),
                            normalized(files, window, size),
                        ) < config.similarity_threshold
                    {
                        continue;
                    }
                    let pair = [previous, window];
                    if !groups
                        .iter()
                        .any(|group| same_occurrences(group, &pair, size))
                    {
                        groups.push(pair.to_vec());
                        if groups.len() >= config.max_groups {
                            return Ok(());
                        }
                    }
                }
                if recent.len() == NEAR_CANDIDATES_PER_BAND {
                    recent.pop_front();
                }
                recent.push_back(window);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sim_of(a: &str, b: &str) -> f32 {
        let ta: Vec<String> = a.lines().map(normalize_line).collect();
        let tb: Vec<String> = b.lines().map(normalize_line).collect();
        similarity(&ta, &tb)
    }

    #[test]
    fn identical_beyond_renaming_is_full_similarity() {
        let a = "for item in items:\n    if item > limit:\n        total += item\n";
        let b = "for entry in entries:\n    if entry > cap:\n        sum += entry\n";
        assert!((sim_of(a, b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn different_logic_scores_below_threshold() {
        let a = "for item in items:\n    if item > limit:\n        total += item\n";
        let b = "while running:\n    process_queue()\n    notify_all()\n";
        assert!(sim_of(a, b) < 0.8);
    }
}
