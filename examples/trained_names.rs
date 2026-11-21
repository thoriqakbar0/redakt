//! Experimental supervised ternary name detector. See experiments/trained-names/README.md.
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::io::{self, BufRead};

const WIDTH: usize = 14336;
const MAGIC: &[u8] = b"RNAME02";
type Range = (usize, usize);

#[derive(Deserialize)]
struct Document {
    text: String,
    names: Vec<Range>,
}

struct Token {
    range: Range,
    word: u32,
    known_name: bool,
    prefix: u32,
    suffix: u32,
    shape: u32,
}

fn hash(bytes: &[u8]) -> u32 {
    bytes.iter().fold(2166136261, |state, byte| {
        (state ^ u32::from(*byte)).wrapping_mul(16777619)
    })
}

fn token_end(chars: &[char], start: usize) -> usize {
    let mut end = start + 1;
    if !chars[start].is_alphanumeric() {
        return end;
    }
    while end < chars.len() {
        let internal = matches!(chars[end], '-' | '\'' | '’')
            && chars.get(end + 1).is_some_and(|c| c.is_alphabetic());
        if !chars[end].is_alphanumeric() && !internal {
            break;
        }
        end += 1;
    }
    end
}

fn tokenize(text: &str, vocabulary: &[u32]) -> Vec<Token> {
    let chars: Vec<_> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let first = chars[start];
        let end = token_end(&chars, start);
        if !first.is_whitespace() {
            let original = &chars[start..end];
            let lower: String = original.iter().flat_map(|c| c.to_lowercase()).collect();
            let bytes = lower.as_bytes();
            let shape = if original.iter().all(|c| c.is_uppercase()) {
                1
            } else if first.is_uppercase() {
                2
            } else if first.is_alphabetic() {
                3
            } else if first.is_numeric() {
                4
            } else {
                5
            };
            let word = hash(bytes);
            tokens.push(Token {
                range: (start, end),
                word,
                known_name: vocabulary.binary_search(&word).is_ok(),
                prefix: hash(&bytes[..bytes.len().min(3)]),
                suffix: hash(&bytes[bytes.len().saturating_sub(3)..]),
                shape,
            });
        }
        start = end;
    }
    tokens
}

fn mix(left: u32, right: u32) -> u32 {
    left.wrapping_mul(0x9e3779b1).rotate_left(13) ^ right
}

fn features(tokens: &[Token], index: usize) -> [usize; 23] {
    let current = &tokens[index];
    let previous = index.checked_sub(1).map(|i| &tokens[i]);
    let next = tokens.get(index + 1);
    let before = previous.map_or(0, |t| t.word);
    let after = next.map_or(0, |t| t.word);
    let prev_shape = previous.map_or(0, |t| t.shape);
    let next_shape = next.map_or(0, |t| t.shape);
    let left2 = index.checked_sub(2).map_or(0, |i| tokens[i].word);
    let right2 = tokens.get(index + 2).map_or(0, |t| t.word);
    let shape = current.shape;
    let values = [
        1,
        current.word,
        current.prefix,
        current.suffix,
        shape,
        before,
        after,
        mix(before, shape),
        mix(after, shape),
        mix(prev_shape, shape),
        mix(shape, next_shape),
        mix(left2, before),
        mix(after, right2),
        mix(left2, shape),
        mix(right2, shape),
        mix(current.prefix, shape),
        mix(current.suffix, shape),
        mix(current.word, shape),
        mix(mix(prev_shape, shape), next_shape),
        (current.range.1 - current.range.0).min(16) as u32,
        // V2 appends vocabulary features after the original twenty slots.
        mix(u32::from(current.known_name), shape),
        mix(u32::from(previous.is_some_and(|t| t.known_name)), shape),
        mix(u32::from(next.is_some_and(|t| t.known_name)), shape),
    ];
    std::array::from_fn(|slot| {
        let mixed = mix(slot as u32 + 1, values[slot]).wrapping_mul(0x85ebca6b);
        ((mixed ^ (mixed >> 16)) as usize) % WIDTH
    })
}

fn token_score(token: &Token, active: &[usize], weights: &[i8]) -> i16 {
    let context: i16 = active.iter().map(|&slot| i16::from(weights[slot])).sum();
    context + 8 * i16::from(token.known_name)
}

fn learn_vocabulary(docs: &[Document]) -> Vec<u32> {
    let mut counts = std::collections::HashMap::<u32, (usize, usize)>::new();
    for doc in docs {
        for token in tokenize(&doc.text, &[]) {
            let end = doc.names.partition_point(|span| span.1 <= token.range.0);
            let inside = doc
                .names
                .get(end)
                .is_some_and(|s| s.0 <= token.range.0 && token.range.1 <= s.1);
            let (positive, total) = counts.entry(token.word).or_default();
            *positive += usize::from(inside);
            *total += 1;
        }
    }
    let mut words: Vec<_> = counts
        .into_iter()
        .filter_map(|(word, (pos, total))| (pos * 3 >= total).then_some(word))
        .collect();
    words.sort_unstable();
    words
}

fn ternary(latent: i16) -> i8 {
    i8::from(latent >= 4) - i8::from(latent <= -4)
}

fn train_round(doc: &Document, latent: &mut [i16], weights: &mut [i8], vocabulary: &[u32]) {
    let tokens = tokenize(&doc.text, vocabulary);
    let mut gold = doc.names.iter().peekable();
    for (index, token) in tokens.iter().enumerate() {
        while gold.peek().is_some_and(|span| span.1 <= token.range.0) {
            gold.next();
        }
        let positive = gold
            .peek()
            .is_some_and(|span| span.0 <= token.range.0 && token.range.1 <= span.1);
        let active = features(&tokens, index);
        let score = token_score(token, &active, weights);
        let direction = if positive { 1 } else { -1 };
        if direction * score < 2 {
            let step = if positive { 2 } else { -1 };
            for slot in active {
                latent[slot] = (latent[slot] + step).clamp(-48, 48);
                weights[slot] = ternary(latent[slot]);
            }
        }
    }
}

struct NameModel {
    weights: Vec<i8>,
    vocabulary: Vec<u32>,
    feature_count: usize,
}

fn predict(text: &str, model: &NameModel) -> Vec<Range> {
    let tokens = tokenize(text, &model.vocabulary);
    let mut ranges: Vec<Range> = Vec::new();
    let mut previous_positive = false;
    for (index, token) in tokens.iter().enumerate() {
        let active = features(&tokens, index);
        let positive = token_score(token, &active[..model.feature_count], &model.weights) > 0;
        if positive {
            if previous_positive {
                ranges.last_mut().unwrap().1 = token.range.1;
            } else {
                ranges.push(token.range);
            }
        }
        previous_positive = positive;
    }
    ranges
}

fn pack(weights: &[i8], vocabulary: &[u32]) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    for group in weights.chunks(5) {
        bytes.push(
            group
                .iter()
                .rev()
                .fold(0, |packed, &w| packed * 3 + (w + 1) as u8),
        );
    }
    for word in vocabulary {
        bytes.extend(word.to_le_bytes());
    }
    bytes
}

fn load(path: &str) -> Result<NameModel, Box<dyn Error>> {
    let file = fs::read(path)?;
    let (feature_count, packed) = if let Some(data) = file.strip_prefix(MAGIC) {
        (23, data)
    } else {
        (
            20,
            file.strip_prefix(b"RNAME01")
                .ok_or("invalid model header")?,
        )
    };
    let (packed, keys) = packed
        .split_at_checked(WIDTH.div_ceil(5))
        .ok_or("truncated weights")?;
    if (feature_count == 20 && !keys.is_empty())
        || keys.len() % 4 != 0
        || packed.iter().any(|&byte| byte >= 243)
    {
        return Err("invalid model size or weights".into());
    }
    let vocabulary: Vec<_> = keys
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(u32::from_le_bytes)
        .collect();
    if vocabulary.windows(2).any(|w| w[0] >= w[1]) {
        return Err("invalid vocabulary".into());
    }
    Ok(NameModel {
        weights: (0..WIDTH)
            .map(|i| ((packed[i / 5] / 3u8.pow((i % 5) as u32)) % 3) as i8 - 1)
            .collect(),
        vocabulary,
        feature_count,
    })
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let [_, command, path] = args.as_slice() else {
        return Err("usage: trained_names train|eval|predict MODEL < stdin".into());
    };
    let lines = io::stdin().lock().lines();
    if command == "train" {
        let mut latent = vec![0; WIDTH];
        let mut weights = vec![0; WIDTH];
        let mut docs: Vec<Document> = Vec::new();
        for line in lines {
            docs.push(serde_json::from_str(&line?)?);
        }
        let vocabulary = learn_vocabulary(&docs);
        for doc in &docs {
            train_round(doc, &mut latent, &mut weights, &vocabulary);
        }
        let count = docs.len();
        if count == 0 {
            return Err("empty training split".into());
        }
        fs::write(path, pack(&weights, &vocabulary))?;
        eprintln!("trained on {count} documents");
        return Ok(());
    }
    let model = load(path)?;
    let mut totals = [0usize; 4];
    for line in lines {
        let line = line?;
        match command.as_str() {
            "predict" => println!("{}", serde_json::to_string(&predict(&line, &model))?),
            "eval" => {
                let doc: Document = serde_json::from_str(&line)?;
                let found = predict(&doc.text, &model);
                totals[0] += 1;
                totals[1] += found
                    .iter()
                    .filter(|s| doc.names.binary_search(s).is_ok())
                    .count();
                totals[2] += found.len();
                totals[3] += doc.names.len();
            }
            _ => return Err("unknown command".into()),
        }
    }
    if command == "eval" {
        println!(
            "documents={} correct={} predicted={} gold={}",
            totals[0], totals[1], totals[2], totals[3]
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_offsets() {
        let model = load(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/experiments/trained-names/names.trits"
        ))
        .unwrap();
        assert_eq!(predict("🦊 I met Nora Chen yesterday.", &model), [(8, 17)]);
    }
}
