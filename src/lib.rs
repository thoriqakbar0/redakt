use serde::Serialize;
use std::collections::BTreeMap;

const FEATURES: usize = 12;
const WIDTH: usize = 19;
const GATES: usize = 12;
const HEADS: usize = 7;
const NONWORD: usize = 129;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub label: String,
}

pub struct Detector {
    embedding: Vec<i16>,
    kernels: Vec<Vec<(usize, i16)>>,
    readout: Vec<i16>,
    thresholds: Vec<i16>,
    lengths: Vec<usize>,
    labels: Vec<String>,
    ranges: Vec<(u32, u32)>,
    pub unicode_version: String,
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        if count > self.remaining.len() {
            return Err("Truncated native model".into());
        }
        let (bytes, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(bytes)
    }

    fn u16(&mut self) -> Result<u16, String> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn trits(&mut self, count: usize) -> Result<Vec<i16>, String> {
        let mut result = Vec::with_capacity(count);
        for &packed in self.take(count.div_ceil(5))? {
            if packed >= 243 {
                return Err("Invalid packed trits".into());
            }
            let mut digits = packed;
            for _ in 0..5 {
                let digit = digits % 3;
                digits /= 3;
                result.push(match digit {
                    2 => -1,
                    _ => i16::from(digit),
                });
            }
        }
        if result[count..].iter().any(|&weight| weight != 0) {
            return Err("Invalid trit padding".into());
        }
        result.truncate(count);
        Ok(result)
    }
}

impl Detector {
    pub fn embedded() -> Result<Self, String> {
        Self::from_bytes(include_bytes!("../models/weights.bin"))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut reader = Reader { remaining: bytes };
        let magic = reader.take(4)?;
        let version = reader.u16()?;
        if magic != b"PHYT" || version != 1 {
            return Err("Unsupported native model format".into());
        }
        for expected in [130, 12, 12, 19, 7, 12] {
            if reader.u16()? != expected {
                return Err("Unsupported circuit dimensions".into());
            }
        }
        let version_length = usize::from(reader.take(1)?[0]);
        let unicode_version = std::str::from_utf8(reader.take(version_length)?)
            .map_err(|_| "Invalid Unicode version")?
            .to_string();
        if unicode_version.is_empty()
            || !unicode_version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return Err("Invalid Unicode version".into());
        }
        let range_count = reader.u32()? as usize;
        if range_count > reader.remaining.len() / 8 {
            return Err("Invalid Unicode range count".into());
        }
        let mut ranges = Vec::with_capacity(range_count);
        let mut previous = 127;
        for _ in 0..range_count {
            let first = reader.u32()?;
            let last = reader.u32()?;
            if first <= previous
                || first > last
                || last > 0x10ffff
                || (first <= 0xdfff && last >= 0xd800)
            {
                return Err("Invalid Unicode ranges".into());
            }
            ranges.push((first, last));
            previous = last;
        }
        let weights = reader.trits(130 * FEATURES + GATES * WIDTH * FEATURES + HEADS * GATES)?;
        let embedding_end = 130 * FEATURES;
        let kernels_end = embedding_end + GATES * WIDTH * FEATURES;
        let embedding = weights[..embedding_end].to_vec();
        let dense_kernels = &weights[embedding_end..kernels_end];
        let kernels = dense_kernels
            .as_chunks::<{ WIDTH * FEATURES }>()
            .0
            .iter()
            .map(|kernel| {
                kernel
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(_, weight)| *weight != 0)
                    .collect()
            })
            .collect();
        let readout = weights[kernels_end..].to_vec();
        let mut thresholds = Vec::with_capacity(GATES);
        for _ in 0..GATES {
            thresholds.push(reader.u16()? as i16);
        }
        let mut lengths = Vec::with_capacity(HEADS);
        for _ in 0..HEADS {
            let length = usize::from(reader.u16()?);
            if length == 0 || length > WIDTH - 2 {
                return Err("Invalid head length".into());
            }
            lengths.push(length);
        }
        let labels = reader
            .take(HEADS)?
            .iter()
            .map(|label| match label {
                0 => Ok("ssn".to_string()),
                1 => Ok("phone".to_string()),
                _ => Err("Invalid head label".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !reader.remaining.is_empty() {
            return Err("Trailing native model data".into());
        }
        Ok(Self {
            embedding,
            kernels,
            readout,
            thresholds,
            lengths,
            labels,
            ranges,
            unicode_version,
        })
    }

    fn character_code(&self, character: char) -> usize {
        let ordinal = u32::from(character);
        if ordinal < 128 {
            return ordinal as usize;
        }
        let index = self.ranges.partition_point(|&(_, last)| last < ordinal);
        if self
            .ranges
            .get(index)
            .is_some_and(|&(first, _)| first <= ordinal)
        {
            128
        } else {
            NONWORD
        }
    }

    fn window_gates(&self, codes: &[usize], start: usize) -> [i16; GATES] {
        let mut gates = [0_i16; GATES];
        for (gate, kernel) in self.kernels.iter().enumerate() {
            let mut score = 0;
            for &(index, weight) in kernel {
                let position = index / FEATURES;
                let offset = (start + position).checked_sub(1);
                let code = offset.and_then(|offset| codes.get(offset));
                let code = code.copied().unwrap_or(NONWORD);
                score += self.embedding[code * FEATURES + index % FEATURES] * weight;
            }
            gates[gate] = i16::from(score >= self.thresholds[gate]);
        }
        gates
    }

    fn window_matches(&self, codes: &[usize], start: usize) -> Vec<Span> {
        let mut candidates = Vec::new();
        let gates = self.window_gates(codes, start);
        for head in 0..HEADS {
            let score: i16 = gates
                .iter()
                .zip(&self.readout[head * GATES..(head + 1) * GATES])
                .map(|(gate, weight)| gate * weight)
                .sum();
            let end = start + self.lengths[head];
            if score < 1 || end > codes.len() {
                continue;
            }
            candidates.push(Span {
                start,
                end,
                label: self.labels[head].clone(),
            });
        }
        candidates
    }

    pub fn detect(&self, text: &str) -> Vec<Span> {
        let codes: Vec<_> = text
            .chars()
            .map(|character| self.character_code(character))
            .collect();
        let mut candidates = Vec::new();
        for start in 0..codes.len() {
            candidates.extend(self.window_matches(&codes, start));
        }
        candidates.sort_by(|left, right| {
            left.start
                .cmp(&right.start)
                .then(right.end.cmp(&left.end))
                .then(left.label.cmp(&right.label))
        });
        let mut cursors = BTreeMap::new();
        candidates.retain(|span| {
            let cursor = cursors.entry(span.label.clone()).or_insert(0);
            if span.start < *cursor {
                return false;
            }
            *cursor = span.end;
            true
        });
        candidates.sort();
        candidates
    }
}

pub fn redact(text: &str, spans: &[Span]) -> Result<String, String> {
    let mut characters: Vec<_> = text.chars().collect();
    for span in spans {
        if span.start >= span.end || span.end > characters.len() {
            return Err("Invalid span bounds".into());
        }
    }
    for span in spans {
        characters[span.start..span.end].fill('█');
    }
    Ok(characters.into_iter().collect())
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
        assert_eq!(redact("a bcd", &[span(1, 4), span(2, 5)]).unwrap(), "a████");
        assert!(redact("abc", &[span(0, 3), span(2, 1)]).is_err());
        assert!(redact("abc", &[span(0, 4)]).is_err());
        assert!(redact("abc", &[span(1, 1)]).is_err());
    }
}
