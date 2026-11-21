//! Supervised matching of preceding words and the shape of a complete PII value.
use crate::Span;
use std::collections::HashMap;

pub const LIMIT: usize = 24;
const SEED: u64 = 14695981039346656037;

fn mix(key: u64, value: u64) -> u64 {
    (key ^ value).wrapping_mul(1099511628211)
}

pub struct Token {
    pub start: usize,
    pub end: usize,
    shape: u64,
    pub cues: [u64; 4],
}

fn token_shape(chars: &[char]) -> u64 {
    let first = chars[0];
    if first.is_numeric() {
        return 256 + chars.len().min(64) as u64;
    }
    if first.is_alphabetic() {
        if chars.iter().all(|ch| ch.is_uppercase()) {
            return 384 + chars.len().min(64) as u64;
        }
        return if first.is_uppercase() { 2 } else { 1 };
    }
    u64::from(u32::from(first)) + 512
}

fn preceding_words(words: &[u64; 4], distance: usize) -> [u64; 4] {
    let mut cues = [0; 4];
    if distance > 16 {
        return cues;
    }
    let mut key = SEED;
    for (index, &word) in words.iter().take_while(|&&w| w != 0).enumerate() {
        key = mix(key, word);
        cues[index] = key;
    }
    cues
}

pub fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<_> = text.chars().collect();
    let mut result = Vec::new();
    let mut words = [0u64; 4];
    let mut last_word_end = 0;
    let mut start = 0;
    while start < chars.len() {
        let first = chars[start];
        if first.is_whitespace() {
            start += 1;
            continue;
        }
        let mut end = start + 1;
        if first.is_alphabetic() {
            while end < chars.len() && chars[end].is_alphabetic() {
                end += 1;
            }
        } else if first.is_numeric() {
            while end < chars.len() && chars[end].is_numeric() {
                end += 1;
            }
        }
        let slice = &chars[start..end];
        let cues = preceding_words(&words, start - last_word_end);
        result.push(Token {
            start,
            end,
            shape: token_shape(slice),
            cues,
        });
        if first.is_alphabetic() {
            let word = slice
                .iter()
                .flat_map(|c| c.to_lowercase())
                .fold(SEED, |k, c| mix(k, u64::from(u32::from(c))));
            words.rotate_right(1);
            words[0] = word;
            last_word_end = end;
        }
        start = end;
    }
    result
}

pub fn shapes(tokens: &[Token]) -> impl Iterator<Item = u64> + '_ {
    let mut previous_end = tokens.first().map_or(0, |t| t.start);
    let mut key = SEED;
    tokens.iter().map(move |token| {
        key = mix(key, token.shape * 2 + u64::from(previous_end < token.start));
        previous_end = token.end;
        key
    })
}

#[derive(Default)]
struct Values {
    patterns: HashMap<u64, u8>,
    longest: usize,
}

impl Values {
    fn matches<'a>(&'a self, tokens: &'a [Token]) -> impl Iterator<Item = (usize, u64, u8)> + 'a {
        let slice = &tokens[..tokens.len().min(self.longest)];
        shapes(slice).zip(slice).filter_map(|(shape, token)| {
            self.patterns
                .get(&shape)
                .map(|&label| (token.end, shape, label))
        })
    }
}

pub struct Cues {
    labels: Vec<String>,
    values: HashMap<u64, Values>,
}

impl Cues {
    pub fn visit(
        &self,
        tokens: &[Token],
        mut accept: impl FnMut(usize, usize, u64, u64, u8, usize),
    ) {
        for start in 0..tokens.len() {
            self.visit_value(&tokens[start..], &mut accept);
        }
    }

    fn visit_value(
        &self,
        tokens: &[Token],
        accept: &mut impl FnMut(usize, usize, u64, u64, u8, usize),
    ) {
        let first = &tokens[0];
        for (depth, cue) in first.cues.iter().enumerate() {
            let Some(values) = self.values.get(cue) else {
                continue;
            };
            for (end, shape, label) in values.matches(tokens) {
                accept(first.start, end, *cue, shape, label, depth);
            }
        }
    }

    pub fn detect(&self, text: &str) -> Vec<Span> {
        let mut found = Vec::new();
        self.visit(&tokens(text), |start, end, _, _, label, depth| {
            found.push((start, end, label, depth))
        });
        found.sort_unstable_by_key(|&(start, end, _, depth)| {
            (start, std::cmp::Reverse(depth), std::cmp::Reverse(end))
        });
        let mut spans = Vec::new();
        let mut stop = 0;
        for (start, end, label, _) in found {
            if start < stop {
                continue;
            }
            stop = end;
            spans.push(Span {
                start,
                end,
                label: self.labels[usize::from(label)].clone(),
            });
        }
        spans
    }

    pub fn refine(&self, text: &str, spans: &mut Vec<Span>) {
        reconcile(spans, self.detect(text));
    }

    pub fn load(bytes: &[u8]) -> Result<Self, String> {
        let mut data = bytes.strip_prefix(b"RCUE1").ok_or("invalid cue model")?;
        let count = *data.first().ok_or("missing cue labels")? as usize;
        data = &data[1..];
        let mut labels = Vec::new();
        for _ in 0..count {
            let end = data
                .iter()
                .position(|&byte| byte == 0)
                .ok_or("missing cue label")?;
            labels.push(
                std::str::from_utf8(&data[..end])
                    .map_err(|_| "invalid cue label")?
                    .to_owned(),
            );
            data = &data[end + 1..];
        }
        let (rows, tail) = data.as_chunks::<18>();
        if !tail.is_empty() {
            return Err("truncated cue model".into());
        }
        let mut values = HashMap::<u64, Values>::new();
        for row in rows {
            let cue = u64::from_le_bytes(row[..8].try_into().unwrap());
            let shape = u64::from_le_bytes(row[8..16].try_into().unwrap());
            let len = usize::from(row[16]);
            if !(1..=LIMIT).contains(&len) || usize::from(row[17]) >= count {
                return Err("invalid cue entry".into());
            }
            let group = values.entry(cue).or_default();
            group.longest = len.max(group.longest);
            group.patterns.insert(shape, row[17]);
        }
        Ok(Self { labels, values })
    }
}

fn reconcile(spans: &mut Vec<Span>, cues: Vec<Span>) {
    if cues.is_empty() {
        return;
    }
    let mut blocked = vec![false; cues.len()];
    for span in spans.iter() {
        let first = cues.partition_point(|cue| cue.end <= span.start);
        for (offset, cue) in cues[first..]
            .iter()
            .take_while(|cue| cue.start < span.end)
            .enumerate()
        {
            if cue == span || span.start < cue.start || cue.end < span.end {
                blocked[first + offset] = true;
            }
        }
    }
    spans.retain(|span| {
        let index = cues.partition_point(|cue| cue.end <= span.start);
        cues.get(index)
            .is_none_or(|cue| cue.start >= span.end || blocked[index])
    });
    spans.extend(
        cues.into_iter()
            .zip(blocked)
            .filter_map(|(cue, blocked)| (!blocked).then_some(cue)),
    );
    spans.sort_unstable();
    spans.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize, label: &str) -> Span {
        Span {
            start,
            end,
            label: label.to_owned(),
        }
    }

    #[test]
    fn conservative_repairs() {
        let mut base = vec![
            span(1, 4, "phone"),
            span(4, 6, "account_number"),
            span(8, 15, "secret"),
            span(8, 15, "email"),
            span(20, 31, "address"),
            span(42, 45, "name"),
        ];
        let cues = vec![
            span(0, 6, "fax_number"),
            span(8, 15, "email"),
            span(22, 29, "city"),
            span(33, 35, "date"),
            span(39, 43, "name"),
        ];
        reconcile(&mut base, cues);
        assert_eq!(
            base,
            [
                span(0, 6, "fax_number"),
                span(8, 15, "email"),
                span(8, 15, "secret"),
                span(20, 31, "address"),
                span(33, 35, "date"),
                span(42, 45, "name")
            ]
        );
    }
}
