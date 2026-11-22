mod contextual;
mod embedded_model;
mod pii;
pub use pii::PiiTagger;

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt::Write;

const CODES: usize = 132;
const MAX_MODEL_BYTES: usize = 2_000_000;
const MAX_INSTRUCTIONS: usize = 10_000;
const MAX_ASSERTION_DEPTH: usize = 64;

type Instruction = [i32; 4];
type Capture = [Option<usize>; 2];
type Match = (usize, Capture);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    label: String,
    entry: usize,
    capture: bool,
    checksum: u8,
    required_masks: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Checksum {
    rows: Vec<Vec<[i32; 3]>>,
    accept: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Model {
    unicode_version: String,
    word_ranges: Vec<[u32; 2]>,
    space_ranges: Vec<[u32; 2]>,
    masks: Vec<Vec<u8>>,
    word_mask: usize,
    instructions: Vec<Instruction>,
    heads: Vec<Head>,
    luhn: Checksum,
    iban: Checksum,
    countries: Vec<[usize; 3]>,
}

pub struct Detector {
    model: Model,
    masks: Vec<Vec<i16>>,
    starts: Vec<[bool; CODES]>,
    luhn_accept: Vec<i16>,
    iban_accept: Vec<i16>,
    pub unicode_version: String,
}

fn trits(packed: &[u8], count: usize) -> Result<Vec<i16>, String> {
    if packed.len() != count.div_ceil(5) || packed.iter().any(|&value| value >= 243) {
        return Err("Invalid packed ternary vector".into());
    }
    let mut result = Vec::with_capacity(packed.len() * 5);
    for &byte in packed {
        let mut remaining = byte;
        for _ in 0..5 {
            let digit = remaining % 3;
            result.push(if digit == 2 { -1 } else { i16::from(digit) });
            remaining /= 3;
        }
    }
    if result[count..].iter().any(|&value| value != 0) {
        return Err("Invalid ternary padding".into());
    }
    result.truncate(count);
    Ok(result)
}

fn validate_ranges(ranges: &[[u32; 2]]) -> Result<(), String> {
    let mut previous = 127;
    for &[first, last] in ranges {
        if first <= previous
            || first > last
            || last > 0x10ffff
            || (first <= 0xdfff && last >= 0xd800)
        {
            return Err("Invalid Unicode ranges".into());
        }
        previous = last;
    }
    Ok(())
}

fn targets(instruction: Instruction) -> Vec<usize> {
    let [operation, first, second, _] = instruction;
    let mut result = Vec::new();
    if operation != 0 {
        result.push(second as usize);
    }
    if operation == 2 || operation == 4 {
        result.push(first as usize);
    }
    result
}

fn validate_instructions(instructions: &[Instruction], masks: usize) -> Result<(), String> {
    if instructions.is_empty() || instructions.len() > MAX_INSTRUCTIONS {
        return Err("Invalid instruction count".into());
    }
    for (position, &instruction) in instructions.iter().enumerate() {
        let [operation, first, _, detail] = instruction;
        if !(0..=5).contains(&operation)
            || targets(instruction)
                .iter()
                .any(|&target| target >= instructions.len())
            || (operation == 1 && (first < 0 || first as usize >= masks))
            || (operation == 3 && !(0..=1).contains(&first))
            || (operation == 4
                && (!(1..=1000).contains(&detail.unsigned_abs())
                    || first < 0
                    || first as usize >= position))
            || (operation == 5 && !(0..=1).contains(&first))
            || (operation != 4 && detail != 0)
        {
            return Err("Invalid instruction operands".into());
        }
    }
    validate_epsilon_graph(instructions)?;
    validate_assertions(instructions)
}

fn validate_epsilon_graph(instructions: &[Instruction]) -> Result<(), String> {
    let mut colors = vec![0_u8; instructions.len()];
    for root in 0..instructions.len() {
        let mut pending = vec![(root, false)];
        while let Some((position, leaving)) = pending.pop() {
            if leaving {
                colors[position] = 2;
                continue;
            }
            match colors[position] {
                1 => return Err("Cyclic zero-width program".into()),
                2 => continue,
                _ => {}
            }
            colors[position] = 1;
            pending.push((position, true));
            let instruction = instructions[position];
            if instruction[0] != 0 && instruction[0] != 1 {
                pending.extend(
                    targets(instruction)
                        .into_iter()
                        .map(|target| (target, false)),
                );
            }
        }
    }
    Ok(())
}

fn validate_assertions(instructions: &[Instruction]) -> Result<(), String> {
    let mut depths = vec![0_usize; instructions.len()];
    for (position, instruction) in instructions.iter().enumerate() {
        if instruction[0] != 4 {
            continue;
        }
        let mut pending = vec![instruction[1] as usize];
        let mut visited = vec![false; instructions.len()];
        let mut depth = 1;
        while let Some(target) = pending.pop() {
            if target >= position {
                return Err("Recursive assertion program".into());
            }
            if visited[target] {
                continue;
            }
            visited[target] = true;
            depth = depth.max(depths[target] + 1);
            pending.extend(targets(instructions[target]));
        }
        if depth > MAX_ASSERTION_DEPTH {
            return Err("Assertion nesting exceeds model limit".into());
        }
        depths[position] = depth;
    }
    Ok(())
}

fn validate_checksum(
    machine: &Checksum,
    states: usize,
    alphabet: usize,
    masks: usize,
) -> Result<Vec<i16>, String> {
    if machine.rows.len() != states {
        return Err("Invalid checksum dimensions".into());
    }
    for row in &machine.rows {
        if row.len() != alphabet {
            return Err("Invalid checksum alphabet".into());
        }
        for &[mask, target, weight] in row {
            if mask < 0
                || mask as usize >= masks
                || target < 0
                || target as usize >= states
                || !(-1..=1).contains(&weight)
            {
                return Err("Invalid checksum transition".into());
            }
        }
    }
    trits(&machine.accept, states)
}

fn starting_masks(instructions: &[Instruction], entry: usize) -> Vec<usize> {
    let mut pending = vec![entry];
    let mut visited = vec![false; instructions.len()];
    let mut masks = Vec::new();
    while let Some(position) = pending.pop() {
        if visited[position] {
            continue;
        }
        visited[position] = true;
        let [operation, first, second, _] = instructions[position];
        if operation == 1 {
            masks.push(first as usize);
        } else if operation != 0 {
            pending.push(second as usize);
            if operation == 2 {
                pending.push(first as usize);
            }
        }
    }
    masks.sort_unstable();
    masks.dedup();
    masks
}

impl Detector {
    pub fn embedded() -> Result<Self, String> {
        Self::from_bytes(embedded_model::EmbeddedModel::load()?.rules)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if !(10..=MAX_MODEL_BYTES).contains(&bytes.len())
            || &bytes[..4] != b"PHYT"
            || u16::from_le_bytes([bytes[4], bytes[5]]) != 2
        {
            return Err("Unsupported native model format".into());
        }
        let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        if length != bytes.len() - 10 {
            return Err("Truncated or trailing native model data".into());
        }
        let model: Model =
            serde_json::from_slice(&bytes[10..]).map_err(|_| "Malformed native model")?;
        Self::from_model(model)
    }

    fn from_model(model: Model) -> Result<Self, String> {
        if model.unicode_version.is_empty()
            || !model
                .unicode_version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return Err("Invalid Unicode version".into());
        }
        validate_ranges(&model.word_ranges)?;
        validate_ranges(&model.space_ranges)?;
        if model.masks.is_empty() || model.masks.len() > 512 || model.word_mask >= model.masks.len()
        {
            return Err("Invalid mask dimensions".into());
        }
        let masks = model
            .masks
            .iter()
            .map(|packed| trits(packed, CODES))
            .collect::<Result<Vec<_>, _>>()?;
        validate_instructions(&model.instructions, masks.len())?;
        let labels = ["email", "phone", "ssn", "card", "iban", "name", "address"];
        if model.heads.len() != labels.len() {
            return Err("Invalid head count".into());
        }
        for (head, label) in model.heads.iter().zip(labels) {
            if head.label != label
                || head.entry >= model.instructions.len()
                || head.checksum > 2
                || head.required_masks.iter().any(|&mask| mask >= masks.len())
            {
                return Err("Invalid head".into());
            }
        }
        if model.countries.is_empty() || model.countries.len() > 100 {
            return Err("Invalid country dimensions".into());
        }
        for &[first, second, length] in &model.countries {
            if first >= masks.len() || second >= masks.len() || !(4..=34).contains(&length) {
                return Err("Invalid country".into());
            }
        }
        let luhn_accept = validate_checksum(&model.luhn, 20, 10, masks.len())?;
        let iban_accept = validate_checksum(&model.iban, 97, 36, masks.len())?;
        let starts = model
            .heads
            .iter()
            .map(|head| {
                let starting = starting_masks(&model.instructions, head.entry);
                std::array::from_fn(|code| starting.iter().any(|&mask| masks[mask][code] >= 1))
            })
            .collect();
        let unicode_version = model.unicode_version.clone();
        Ok(Self {
            model,
            masks,
            starts,
            luhn_accept,
            iban_accept,
            unicode_version,
        })
    }

    fn character_code(&self, character: char) -> usize {
        let ordinal = u32::from(character);
        if ordinal < 128 {
            return ordinal as usize;
        }
        let mut bits = 0;
        for (ranges, bit) in [(&self.model.word_ranges, 1), (&self.model.space_ranges, 2)] {
            let index = ranges.partition_point(|&[_, last]| last < ordinal);
            if ranges
                .get(index)
                .is_some_and(|&[first, _]| first <= ordinal)
            {
                bits |= bit;
            }
        }
        128 + bits
    }

    fn gate(&self, mask: usize, code: Option<usize>) -> bool {
        code.is_some_and(|code| self.masks[mask][code] >= 1)
    }

    fn anchor(&self, kind: i32, codes: &[usize], cursor: usize) -> bool {
        let current = codes.get(cursor).copied();
        if kind == 1 {
            return i16::from(cursor == codes.len())
                + i16::from(cursor + 1 == codes.len() && current == Some(10))
                >= 1;
        }
        let previous = cursor
            .checked_sub(1)
            .and_then(|index| codes.get(index))
            .copied();
        let difference = i16::from(self.gate(self.model.word_mask, previous))
            - i16::from(self.gate(self.model.word_mask, current));
        difference.max(-difference) >= 1
    }

    fn assertion_gate(&self, codes: &[usize], cursor: usize, entry: usize, detail: i32) -> bool {
        let width = detail.unsigned_abs() as usize - 1;
        let matched = cursor
            .checked_sub(width)
            .and_then(|origin| {
                let end = if width == 0 { None } else { Some(cursor) };
                self.run(codes, entry, origin, end, &mut Vec::new())
            })
            .is_some();
        let polarity = if detail > 0 { 1 } else { -1 };
        let threshold = if detail > 0 { 1 } else { 0 };
        i16::from(matched) * polarity >= threshold
    }

    fn run(
        &self,
        codes: &[usize],
        entry: usize,
        start: usize,
        required_end: Option<usize>,
        pending: &mut Vec<(usize, usize, Capture)>,
    ) -> Option<Match> {
        pending.clear();
        let mut position = entry;
        let mut cursor = start;
        let mut capture = [None, None];
        loop {
            let [operation, first, second, detail] = self.model.instructions[position];
            if operation == 0 && required_end.is_none_or(|end| cursor == end) {
                return Some((cursor, capture));
            }
            let failed = operation == 0
                || (operation == 1 && !self.gate(first as usize, codes.get(cursor).copied()))
                || (operation == 4 && !self.assertion_gate(codes, cursor, first as usize, detail))
                || (operation == 5 && !self.anchor(first, codes, cursor));
            if failed {
                let Some((next, saved_cursor, saved_capture)) = pending.pop() else {
                    break;
                };
                position = next;
                cursor = saved_cursor;
                capture = saved_capture;
                continue;
            }
            if operation == 1 {
                cursor += 1;
            }
            if operation == 2 {
                pending.push((second as usize, cursor, capture));
                position = first as usize;
                continue;
            }
            if operation == 3 {
                capture[first as usize] = Some(cursor);
            }
            position = second as usize;
        }
        None
    }

    fn transition(&self, machine: &Checksum, state: usize, code: usize) -> Option<(usize, usize)> {
        for &[mask, target, weight] in &machine.rows[state] {
            let score = 1 + weight as i16 * i16::from(self.gate(mask as usize, Some(code)));
            if score >= 2 {
                return Some((target as usize, mask as usize));
            }
        }
        None
    }

    fn luhn_valid(&self, compact: &[usize]) -> bool {
        let mut state = 0;
        let mut first_mask = None;
        let mut all_same = 1;
        for code in compact.iter().copied().rev() {
            let Some((target, mask)) = self.transition(&self.model.luhn, state, code) else {
                return false;
            };
            state = target;
            let first = *first_mask.get_or_insert(mask);
            all_same = i16::from(all_same + i16::from(self.gate(first, Some(code))) >= 2);
        }
        self.luhn_accept[state] - all_same >= 1
    }

    fn checksum_valid(&self, kind: u8, codes: &[usize]) -> bool {
        if kind == 0 {
            return true;
        }
        let compact: Vec<_> = codes
            .iter()
            .copied()
            .filter(|code| ![32, 45].contains(code))
            .collect();
        if kind == 1 {
            return self.luhn_valid(&compact);
        }
        if compact.len() < 4 {
            return false;
        }
        let country_valid = self.model.countries.iter().any(|&[first, second, length]| {
            i16::from(self.gate(first, Some(compact[0])))
                + i16::from(self.gate(second, Some(compact[1])))
                + i16::from(compact.len() == length)
                >= 3
        });
        if !country_valid {
            return false;
        }
        let mut state = 0;
        for &code in compact[4..].iter().chain(&compact[..4]) {
            let Some((target, _)) = self.transition(&self.model.iban, state, code) else {
                return false;
            };
            state = target;
        }
        self.iban_accept[state] >= 1
    }

    fn detect_head(&self, head: &Head, starts: &[bool; CODES], codes: &[usize]) -> Vec<Span> {
        let mut spans = Vec::new();
        let mut pending = Vec::new();
        let mut cursor = 0;
        while cursor < codes.len() {
            if !starts[codes[cursor]] {
                cursor += 1;
                continue;
            }
            let Some((end, capture)) = self.run(codes, head.entry, cursor, None, &mut pending)
            else {
                cursor += 1;
                continue;
            };
            let origin = cursor;
            cursor = (cursor + 1).max(end);
            if !self.checksum_valid(head.checksum, &codes[origin..end]) {
                continue;
            }
            let bounds = if head.capture {
                capture[0].zip(capture[1])
            } else {
                Some((origin, end))
            };
            let Some((start, stop)) = bounds.filter(|&(start, stop)| start < stop) else {
                continue;
            };
            spans.push(Span {
                start,
                end: stop,
                label: head.label.clone(),
            });
        }
        spans
    }

    /// Adds bounded cue-based recognition without changing model predictions.
    pub fn detect_contextual(&self, text: &str) -> Vec<Span> {
        let mut spans = self.detect(text);
        spans.extend(contextual::scan(text));
        spans.sort();
        spans.dedup();
        spans
    }

    pub fn detect(&self, text: &str) -> Vec<Span> {
        let codes: Vec<_> = text
            .chars()
            .map(|character| self.character_code(character))
            .collect();
        let mut present = [false; CODES];
        for &code in &codes {
            present[code] = true;
        }
        let active: Vec<_> = self
            .masks
            .iter()
            .map(|mask| {
                mask.iter()
                    .zip(present)
                    .any(|(&weight, seen)| seen && weight >= 1)
            })
            .collect();

        let mut spans = Vec::new();
        for (head, starts) in self.model.heads.iter().zip(&self.starts) {
            if head.required_masks.iter().all(|&mask| active[mask]) {
                spans.extend(self.detect_head(head, starts, &codes));
            }
        }
        spans.sort();
        spans
    }
}

/// Replaces spans with numbered labels such as `[EMAIL_1]`.
///
/// Overlapping spans share one replacement. The earliest span supplies its
/// label, with longer spans and then alphabetical labels breaking ties.
/// Identical replaced text with the same label reuses its number in this call.
pub fn redact(text: &str, spans: &[Span]) -> Result<String, String> {
    let mut numbers: HashMap<String, HashMap<&str, usize>> = HashMap::new();
    redact_with(text, spans, |output, label, value| {
        let label = label.to_ascii_uppercase();
        let values = numbers.entry(label.clone()).or_default();
        let next = values.len() + 1;
        let number = values.entry(value).or_insert(next);
        write!(output, "[{label}_{number}]").expect("writing to a String cannot fail");
        Ok(())
    })
}

#[doc(hidden)]
pub fn redact_with<'a>(
    text: &'a str,
    spans: &[Span],
    mut replace: impl FnMut(&mut String, &str, &'a str) -> Result<(), String>,
) -> Result<String, String> {
    if spans.is_empty() {
        return Ok(text.to_owned());
    }

    let offsets: Vec<_> = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain([text.len()])
        .collect();
    if spans
        .iter()
        .any(|span| span.start >= span.end || span.end >= offsets.len())
    {
        return Err("Invalid span bounds".into());
    }

    let mut ordered: Vec<_> = spans.iter().collect();
    ordered.sort_unstable_by_key(|span| (span.start, Reverse(span.end), &span.label));
    let mut ordered = ordered.into_iter().peekable();
    let mut redacted = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(span) = ordered.next() {
        let mut end = span.end;
        while let Some(overlap) = ordered.next_if(|next| next.start < end) {
            end = end.max(overlap.end);
        }

        let start = offsets[span.start];
        let end = offsets[end];
        redacted.push_str(&text[cursor..start]);
        replace(&mut redacted, &span.label, &text[start..end])?;
        cursor = end;
    }
    redacted.push_str(&text[cursor..]);
    Ok(redacted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_union_and_bounds() {
        let span = |start, end| Span {
            start,
            end,
            label: "phone".into(),
        };
        assert_eq!(
            redact(
                "🦊étude!",
                &[span(3, 6), span(1, 3), span(2, 4), span(6, 7)]
            )
            .unwrap(),
            "🦊[PHONE_1][PHONE_2]"
        );
        assert!(redact("abc", &[span(0, 3), span(2, 1)]).is_err());
        assert!(redact("abc", &[span(0, 4)]).is_err());
        assert!(redact("abc", &[span(1, 1)]).is_err());
    }
}
