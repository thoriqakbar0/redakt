use crate::Span;
use std::collections::{HashMap, HashSet};

// A boundary correction only needs a short neighborhood around a detected value.
const MAX_LENGTH: usize = 128;

type Seed<'a> = (&'a str, &'a str);

struct Seeds<'a> {
    values: HashSet<Seed<'a>>,
    lengths: HashMap<(&'a str, char), Vec<usize>>,
}

impl<'a> Seeds<'a> {
    fn new(text: &'a str, offsets: &[usize], spans: &'a [Span]) -> Self {
        let mut values = HashSet::new();
        let mut lengths: HashMap<_, Vec<_>> = HashMap::new();
        for span in spans {
            let width = span.end - span.start;
            if !(3..=MAX_LENGTH).contains(&width) {
                continue;
            }
            let value = &text[offsets[span.start]..offsets[span.end]];
            let first = value.chars().next().unwrap();
            let label = span.label.as_str();
            if eligible(value) && values.insert((label, value)) {
                lengths.entry((label, first)).or_default().push(width);
            }
        }
        for widths in lengths.values_mut() {
            widths.sort_unstable_by(|a, b| b.cmp(a));
            widths.dedup();
        }
        Self { values, lengths }
    }
}

fn eligible(value: &str) -> bool {
    let bounded =
        value.starts_with(char::is_alphanumeric) && value.ends_with(char::is_alphanumeric);
    if !bounded {
        return false;
    }
    let letters = value.chars().filter(|c| c.is_alphabetic()).count();
    let digits = value.chars().filter(|c| c.is_numeric()).count();
    digits >= 6
        || letters >= 3
            && (value.contains(char::is_uppercase)
                || value.contains(char::is_whitespace)
                || value.contains(['@', '.', '-', '_']))
}

fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

struct Document<'a> {
    text: &'a str,
    chars: Vec<char>,
    offsets: Vec<usize>,
    seeds: Seeds<'a>,
}

impl<'a> Document<'a> {
    fn candidate(&self, start: usize, first: &Span) -> Option<usize> {
        if start > 0 && word(self.chars[start - 1]) {
            return None;
        }
        let label = first.label.as_str();
        let lengths = self.seeds.lengths.get(&(label, self.chars[start]))?;
        for &length in lengths {
            let end = start + length;
            if end > self.chars.len() || end < first.end || (start, end) == (first.start, first.end)
            {
                continue;
            }
            if self.chars.get(end).is_some_and(|c| word(*c)) {
                continue;
            }
            let value = &self.text[self.offsets[start]..self.offsets[end]];
            if self.seeds.values.contains(&(label, value)) {
                return Some(end);
            }
        }
        None
    }

    fn extension(&self, spans: &[Span], previous_end: usize) -> Option<(Span, usize)> {
        let first = &spans[0];
        let earliest = previous_end.max(first.end.saturating_sub(MAX_LENGTH));
        let mut best = None;
        let mut best_length = first.end - first.start;
        for start in earliest..=first.start {
            let Some(end) = self.candidate(start, first) else {
                continue;
            };
            let count = spans.partition_point(|span| span.start < end);
            if end - start <= best_length
                || spans[..count]
                    .iter()
                    .any(|s| s.end > end || s.label != first.label)
            {
                continue;
            }
            best_length = end - start;
            best = Some((
                Span {
                    start,
                    end,
                    label: first.label.clone(),
                },
                count,
            ));
        }
        best
    }
}

/// Extend a partial detection only if the same document already labels the full
/// surface with the same type. Input spans are in ascending start order.
pub(super) fn repair(text: &str, spans: Vec<Span>) -> Vec<Span> {
    if spans.len() < 2 {
        return spans;
    }
    let chars: Vec<_> = text.chars().collect();
    let offsets: Vec<_> = text
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .collect();
    let seeds = Seeds::new(text, &offsets, &spans);
    let document = Document {
        text,
        chars,
        offsets,
        seeds,
    };
    let mut output = Vec::with_capacity(spans.len());
    let mut index = 0;
    let mut previous_end = 0;
    while index < spans.len() {
        let (span, consumed) = document
            .extension(&spans[index..], previous_end)
            .unwrap_or_else(|| (spans[index].clone(), 1));
        previous_end = previous_end.max(span.end);
        output.push(span);
        index += consumed;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize, label: &str) -> Span {
        Span {
            start,
            end,
            label: label.to_string(),
        }
    }

    #[test]
    fn repeated_names() {
        let text = "🦊 Nora Chen met Nora Chen and Nora Chen.";
        let input = vec![span(2, 11, "name"), span(16, 20, "name")];
        assert_eq!(
            repair(text, input),
            [span(2, 11, "name"), span(16, 25, "name")]
        );
    }

    #[test]
    fn conflicting_label() {
        let text = "Nora Chen met Nora Chen.";
        let input = vec![
            span(0, 9, "name"),
            span(14, 18, "name"),
            span(19, 23, "city"),
        ];
        assert_eq!(repair(text, input.clone()), input);
    }
}
