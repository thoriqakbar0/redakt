//! Train and use a compact model of lexical cues and value shapes.
use redakt::Span;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

#[path = "../src/pii/cues.rs"]
mod cues;
use cues::{Cues, Token, LIMIT};

#[derive(Deserialize)]
struct Document {
    text: String,
    spans: Vec<(usize, usize, String)>,
}

type Counts = HashMap<(u64, u64), HashMap<String, usize>>;
type Pattern = (u64, u64, u8, u8);

fn read(path: &str, mut visit: impl FnMut(Document)) -> Result<(), Box<dyn Error>> {
    let source = BufReader::new(File::open(path)?);
    for line in source.lines() {
        visit(serde_json::from_str(&line?)?);
    }
    Ok(())
}

fn value(tokens: &[Token], start: usize, end: usize) -> Option<&[Token]> {
    let left = tokens.binary_search_by_key(&start, |t| t.start).ok()?;
    let slice = &tokens[left..];
    let count = slice.partition_point(|t| t.end <= end);
    if count == 0 || count > LIMIT || slice[count - 1].end != end {
        return None;
    }
    Some(&slice[..count])
}

fn collect(doc: Document, counts: &mut Counts, lengths: &mut HashMap<(u64, u64), u8>) {
    let tokens = cues::tokens(&doc.text);
    for (start, end, label) in doc.spans {
        let Some(span) = value(&tokens, start, end) else {
            continue;
        };
        let key = cues::shapes(span).last().unwrap();
        for &cue in span[0].cues.iter().take_while(|&&cue| cue != 0) {
            *counts
                .entry((cue, key))
                .or_default()
                .entry(label.clone())
                .or_default() += 1;
            lengths.insert((cue, key), span.len() as u8);
        }
    }
}

fn encode(labels: &[String], patterns: &[Pattern]) -> Vec<u8> {
    let mut out = b"RCUE1".to_vec();
    out.push(u8::try_from(labels.len()).unwrap());
    for label in labels {
        out.extend(label.as_bytes());
        out.push(0);
    }
    for &(cue, shape, length, label) in patterns {
        out.extend(cue.to_le_bytes());
        out.extend(shape.to_le_bytes());
        out.extend([length, label]);
    }
    out
}

fn train(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut counts = HashMap::new();
    let mut lengths = HashMap::new();
    read(path, |doc| collect(doc, &mut counts, &mut lengths))?;
    let mut support = HashMap::new();
    let mut patterns = Vec::new();
    let mut labels: Vec<_> = counts.values().flat_map(|h| h.keys().cloned()).collect();
    labels.sort_unstable();
    labels.dedup();
    for ((cue, shape), labels_used) in counts {
        let (label, times) = labels_used.into_iter().max_by_key(|(_, n)| *n).unwrap();
        if times >= 3 {
            let index = labels.binary_search(&label).unwrap() as u8;
            patterns.push((cue, shape, lengths[&(cue, shape)], index));
            support.insert((cue, shape), times);
        }
    }
    let candidates = Cues::load(&encode(&labels, &patterns))?;
    let mut total = HashMap::new();
    read(path, |doc| {
        candidates.visit(&cues::tokens(&doc.text), |_, _, cue, shape, _, _| {
            *total.entry((cue, shape)).or_insert(0usize) += 1;
        });
    })?;
    patterns
        .retain(|&(cue, shape, _, _)| support[&(cue, shape)] * 100 >= total[&(cue, shape)] * 94);
    patterns.sort_unstable();
    eprintln!("learned {} cue-value patterns", patterns.len());
    Ok(encode(&labels, &patterns))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    match &args[1..] {
        [command, path, output] if command == "train" => fs::write(output, train(path)?)?,
        [command, path] if command == "predict" => {
            let cues = Cues::load(&fs::read(path)?)?;
            for line in std::io::stdin().lock().lines() {
                let text: String = serde_json::from_str(&line?)?;
                let mut spans = Vec::new();
                cues.refine(&text, &mut spans);
                println!("{}", serde_json::to_string(&spans)?);
            }
        }
        _ => return Err("usage: train_cues train FIT MODEL | predict MODEL < JSON_STRINGS".into()),
    }
    Ok(())
}
