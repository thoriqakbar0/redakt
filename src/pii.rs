//! Contextual PII sequence tagger trained on annotated English documents.
use crate::{Detector, Span};
use std::collections::HashMap;
use std::io::{Cursor, Read};

mod address;
mod boundaries;
mod cues;
mod features;
mod repeats;
use features::{features, tokenize, Token, WIDTH};

struct Direction {
    rows: Vec<Vec<(u8, i16)>>,
}

fn bytes<const N: usize>(source: &mut Cursor<&[u8]>) -> Result<[u8; N], String> {
    let mut value = [0; N];
    source
        .read_exact(&mut value)
        .map_err(|_| "truncated PII model")?;
    Ok(value)
}

fn size(source: &mut Cursor<&[u8]>) -> Result<usize, String> {
    Ok(u32::from_le_bytes(bytes(source)?) as usize)
}

fn read_weight(
    source: &mut Cursor<&[u8]>,
    scale: i16,
    classes: usize,
) -> Result<(u8, i16), String> {
    let [value] = bytes(source)?;
    let label = value & 127;
    if usize::from(label) >= classes {
        return Err("invalid PII label".into());
    }
    Ok((label, if value < 128 { scale } else { -scale }))
}

impl Direction {
    fn read(source: &mut Cursor<&[u8]>, classes: usize) -> Result<Self, String> {
        let count = size(source)?;
        if count > WIDTH {
            return Err("invalid PII feature count".into());
        }
        let mut rows = vec![Vec::new(); WIDTH];
        for _ in 0..count {
            let index = size(source)?;
            let scale = i16::from_le_bytes(bytes(source)?);
            let [entries] = bytes(source)?;
            let row = rows.get_mut(index).ok_or("invalid PII feature index")?;
            if scale <= 0 || !row.is_empty() {
                return Err("invalid PII feature row".into());
            }
            for _ in 0..entries {
                row.push(read_weight(source, scale, classes)?);
            }
        }
        Ok(Self { rows })
    }

    fn score(&self, active: &[usize], scores: &mut [i32]) {
        scores.fill(0);
        for &slot in active {
            for &(label, weight) in &self.rows[slot] {
                scores[usize::from(label)] += i32::from(weight);
            }
        }
    }
}

fn best(scores: &[i32]) -> u8 {
    let mut label = 0;
    for index in 1..scores.len() {
        if scores[index] > scores[label] {
            label = index;
        }
    }
    label as u8
}

/// A learned, offline multi-category PII tagger.
pub struct PiiTagger {
    labels: Vec<String>,
    vocabulary: HashMap<u32, u8>,
    directions: Vec<Direction>,
    cues: cues::Cues,
}

impl PiiTagger {
    /// Load the embedded model only when broad PII detection is requested.
    pub fn embedded() -> Result<Self, String> {
        let bundle = crate::embedded_model::EmbeddedModel::load()?;
        let mut source = Cursor::new(bundle.tagger);
        if bytes::<6>(&mut source)? != *b"RPII01" {
            return Err("invalid PII model header".into());
        }
        let [count] = bytes(&mut source)?;
        if !(2..=128).contains(&count) {
            return Err("invalid PII label count".into());
        }
        let mut labels = Vec::new();
        for _ in 0..count {
            let [len] = bytes(&mut source)?;
            let mut name = vec![0; len.into()];
            source
                .read_exact(&mut name)
                .map_err(|_| "truncated PII label")?;
            labels.push(String::from_utf8(name).map_err(|_| "invalid PII label")?);
        }
        let words = size(&mut source)?;
        let mut vocabulary = HashMap::new();
        for _ in 0..words {
            let word = u32::from_le_bytes(bytes(&mut source)?);
            let [label] = bytes(&mut source)?;
            if label >= count {
                return Err("invalid PII vocabulary label".into());
            }
            vocabulary.insert(word, label);
        }
        let [count] = bytes(&mut source)?;
        if !(1..=2).contains(&count) {
            return Err("invalid PII direction count".into());
        }
        let mut directions = Vec::new();
        for _ in 0..count {
            directions.push(Direction::read(&mut source, labels.len())?);
        }
        if source.position() != source.get_ref().len() as u64 {
            return Err("trailing PII data".into());
        }
        Ok(Self {
            labels,
            vocabulary,
            directions,
            cues: cues::Cues::load(bundle.cues)?,
        })
    }

    fn reverse_tags(&self, tokens: &[Token], model: &Direction) -> Vec<u8> {
        let mut tags = vec![0; tokens.len() + 1];
        let mut scores = vec![0; self.labels.len()];
        for index in (0..tokens.len()).rev() {
            let active = features(tokens, index, tags[index + 1], &self.vocabulary);
            model.score(&active, &mut scores);
            tags[index] = best(&scores);
        }
        tags
    }

    /// Apply both detectors and return the spans used by the default CLI.
    pub fn detect_with_rules(&self, rules: &Detector, text: &str) -> Vec<Span> {
        let mut spans = rules.detect_contextual(text);
        let address_ends = address::AddressEnds::from_rules(&spans);
        let mut learned = self.detect(text);
        trim_address_endings(text, &mut learned);
        spans.extend(learned);
        boundaries::repair(text, &mut spans);
        self.cues.refine(text, &mut spans);
        spans.sort_unstable();
        spans = repeats::repair(text, spans);
        discard_email_words(text, &mut spans);
        address_ends.clip(text, &mut spans);
        spans.sort_unstable();
        spans.dedup();
        spans
    }

    /// Return typed spans using Unicode code-point offsets.
    pub fn detect(&self, text: &str) -> Vec<Span> {
        let tokens = tokenize(text);
        let reverse = self
            .directions
            .get(1)
            .map(|model| (model, self.reverse_tags(&tokens, model)));
        let mut spans: Vec<Span> = Vec::new();
        let mut prior = 0;
        let mut scores = vec![0; self.labels.len()];
        let mut other = scores.clone();
        for (index, token) in tokens.iter().enumerate() {
            let active = features(&tokens, index, prior, &self.vocabulary);
            self.directions[0].score(&active, &mut scores);
            if let Some((model, tags)) = &reverse {
                let active = features(&tokens, index, tags[index + 1], &self.vocabulary);
                model.score(&active, &mut other);
                for (score, extra) in scores.iter_mut().zip(&other) {
                    *score += extra;
                }
            }
            // Scores use fixed-point units of 1/64 per direction.
            scores[0] -= 768;
            let label = best(&scores);
            let extend = label == prior;
            prior = label;
            if label == 0 {
                continue;
            }
            if extend {
                spans.last_mut().unwrap().end = token.end;
                continue;
            }
            spans.push(Span {
                start: token.start,
                end: token.end,
                label: self.labels[usize::from(label)].clone(),
            });
        }
        spans
    }
}

fn trim_address_endings(text: &str, spans: &mut Vec<Span>) {
    let chars: Vec<_> = text.chars().collect();
    for span in spans.iter_mut().filter(|span| span.label == "address") {
        while span.end > span.start && matches!(chars[span.end - 1], '。' | '！' | '？') {
            span.end -= 1;
        }
    }
    spans.retain(|span| span.end > span.start);
}

// A bare word can inherit the email tag from its surrounding sentence.
fn discard_email_words(text: &str, spans: &mut Vec<Span>) {
    if !spans.iter().any(|span| span.label == "email") {
        return;
    }
    let chars: Vec<_> = text.chars().collect();
    spans.retain(|span| {
        span.label != "email"
            || !chars[span.start..span.end]
                .iter()
                .all(|ch| ch.is_alphabetic())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_boundaries_in_prose() {
        let text = "I met Nora Chen yesterday. Email her at nora@example.org or call 415-555-0198.";
        let spans = PiiTagger::embedded()
            .unwrap()
            .detect_with_rules(&Detector::embedded().unwrap(), text);
        assert_eq!(
            crate::redact(text, &spans).unwrap(),
            "I met [NAME_1] yesterday. Email her at [EMAIL_1] or call [PHONE_1]."
        );
    }
}
