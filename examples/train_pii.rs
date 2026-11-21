//! Train the supervised PII token tagger on normalized, annotated JSONL.
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

#[path = "../src/pii/features.rs"]
mod pii;
use pii::{features, tokenize, Token, WIDTH};

#[derive(Deserialize)]
struct Document {
    text: String,
    spans: Vec<(usize, usize, String)>,
}

struct Example {
    tokens: Vec<Token>,
    targets: Vec<u8>,
}

type TrainingError = Box<dyn Error>;
type FeatureRow = (usize, i16, Vec<(u8, i8)>);

fn load_examples(path: &str) -> Result<(Vec<String>, Vec<Example>), TrainingError> {
    let mut documents = Vec::<Document>::new();
    let mut classes = BTreeSet::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let doc: Document = serde_json::from_str(&line?)?;
        for (_, _, label) in &doc.spans {
            classes.insert(label.clone());
        }
        documents.push(doc);
    }
    if classes.is_empty() || classes.len() > u8::MAX as usize || classes.contains("") {
        return Err("expected between 1 and 255 nonempty PII classes".into());
    }
    let labels: Vec<_> = std::iter::once(String::new()).chain(classes).collect();
    let ids: HashMap<_, _> = labels
        .iter()
        .enumerate()
        .map(|(id, label)| (label.as_str(), id as u8))
        .collect();
    let mut examples = Vec::with_capacity(documents.len());
    for doc in documents {
        let tokens = tokenize(&doc.text);
        let mut targets = vec![0; tokens.len()];
        for (start, end, label) in doc.spans {
            let first = tokens.partition_point(|t| t.end <= start);
            let last = tokens.partition_point(|t| t.start < end);
            if first >= last {
                continue;
            }
            targets[first..last].fill(ids[label.as_str()]);
        }
        examples.push(Example { tokens, targets });
    }
    Ok((labels, examples))
}

fn learn_vocab(examples: &[Example]) -> HashMap<u32, u8> {
    let mut totals = HashMap::<u32, u32>::new();
    let mut positives = HashMap::<(u32, u8), u32>::new();
    for doc in examples {
        for (token, &label) in doc.tokens.iter().zip(&doc.targets) {
            *totals.entry(token.word).or_default() += 1;
            if label != 0 {
                *positives.entry((token.word, label)).or_default() += 1;
            }
        }
    }
    positives
        .into_iter()
        .filter_map(|((word, label), count)| (count > totals[&word] / 2).then_some((word, label)))
        .collect()
}

struct Perceptron {
    classes: usize,
    weights: Vec<i16>,
    sums: Vec<i64>,
    last_change: Vec<u64>,
    steps: u64,
}

impl Perceptron {
    fn new(classes: usize) -> Self {
        let size = WIDTH * classes;
        Self {
            classes,
            weights: vec![0; size],
            sums: vec![0; size],
            last_change: vec![0; size],
            steps: 0,
        }
    }

    fn vote(&self, active: &[usize], scores: &mut [i32]) -> u8 {
        scores.fill(0);
        for &feature in active {
            let start = feature * self.classes;
            let row = &self.weights[start..start + self.classes];
            for (score, &weight) in scores.iter_mut().zip(row) {
                *score += i32::from(weight);
            }
        }
        let mut best = 0;
        for index in 1..scores.len() {
            if scores[index] > scores[best] {
                best = index;
            }
        }
        best as u8
    }

    fn update(&mut self, feature: usize, label: u8, delta: i16) {
        let index = feature * self.classes + usize::from(label);
        let duration = self.steps - self.last_change[index];
        self.sums[index] += duration as i64 * i64::from(self.weights[index]);
        self.last_change[index] = self.steps;
        self.weights[index] = self.weights[index].saturating_add(delta);
    }

    fn train_document(
        &mut self,
        doc: &Example,
        vocabulary: &HashMap<u32, u8>,
        scores: &mut [i32],
        reverse: bool,
    ) -> usize {
        let mut mistakes = 0;
        let mut previous = 0;
        for offset in 0..doc.targets.len() {
            let index = if reverse {
                doc.targets.len() - 1 - offset
            } else {
                offset
            };
            let target = doc.targets[index];
            let active = features(&doc.tokens, index, previous, vocabulary);
            let predicted = self.vote(&active, scores);
            previous = predicted;
            if predicted == target {
                self.steps += 1;
                continue;
            }
            for feature in active {
                self.update(feature, target, 1);
                self.update(feature, predicted, -1);
            }
            mistakes += 1;
            self.steps += 1;
        }
        mistakes
    }

    fn epoch(
        &mut self,
        examples: &[Example],
        vocabulary: &HashMap<u32, u8>,
        reverse: bool,
    ) -> usize {
        let mut scores = vec![0; self.classes];
        examples
            .iter()
            .map(|doc| self.train_document(doc, vocabulary, &mut scores, reverse))
            .sum()
    }

    fn average(&self, index: usize) -> f64 {
        let duration = (self.steps - self.last_change[index]) as i64;
        let integral = self.sums[index] + duration * i64::from(self.weights[index]);
        integral as f64 / self.steps as f64
    }

    fn quantized(&self) -> Vec<FeatureRow> {
        let mut rows = Vec::new();
        for feature in 0..WIDTH {
            let start = feature * self.classes;
            let weights: Vec<_> = (0..self.classes)
                .map(|id| (id as u8, self.average(start + id)))
                .collect();
            if let Some((scale, entries)) = quantize(weights) {
                rows.push((feature, scale, entries));
            }
        }
        rows
    }
}

fn quantize(mut weights: Vec<(u8, f64)>) -> Option<(i16, Vec<(u8, i8)>)> {
    // For a fixed number of nonzero trits, the least-squares scale is their mean magnitude.
    weights.retain(|(_, value)| *value != 0.0);
    weights.sort_unstable_by(|left, right| right.1.abs().total_cmp(&left.1.abs()));
    let (mut count, mut scale, mut gain, mut sum) = (0, 0.0, 0.0, 0.0);
    for (index, (_, value)) in weights.iter().enumerate() {
        sum += value.abs();
        let mean = sum / (index + 1) as f64;
        if mean * sum > gain {
            count = index + 1;
            gain = mean * sum;
            scale = mean;
        }
    }
    let fixed_scale = (scale * 64.0).round() as i16;
    if fixed_scale == 0 {
        return None;
    }
    let mut entries: Vec<_> = weights[..count]
        .iter()
        .map(|&(label, value)| (label, value.signum() as i8))
        .collect();
    entries.sort_unstable();
    Some((fixed_scale, entries))
}

fn main() -> Result<(), TrainingError> {
    let args: Vec<_> = std::env::args().collect();
    let [_, input, output, epochs, mode @ ..] = args.as_slice() else {
        return Err("usage: train_pii TRAIN.jsonl MODEL.bin EPOCHS [reverse]".into());
    };
    let reverse = match mode {
        [] => false,
        [flag] if flag == "reverse" => true,
        _ => return Err("expected reverse as optional final argument".into()),
    };
    let epochs: usize = epochs.parse()?;
    if epochs == 0 {
        return Err("at least one epoch is required".into());
    }
    let (labels, examples) = load_examples(input)?;
    let vocabulary = learn_vocab(&examples);
    let mut trainer = Perceptron::new(labels.len());
    eprintln!(
        "{} documents, {} classes, {} vocabulary entries",
        examples.len(),
        labels.len() - 1,
        vocabulary.len()
    );
    for epoch in 1..=epochs {
        let errors = trainer.epoch(&examples, &vocabulary, reverse);
        eprintln!("epoch {epoch}: {errors} token updates");
    }
    let mut vocab: Vec<_> = vocabulary.into_iter().collect();
    vocab.sort_unstable();
    let artifact = (labels, vocab, trainer.quantized());
    let mut writer = BufWriter::new(File::create(output)?);
    serde_json::to_writer(&mut writer, &artifact)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ternary_centroid() {
        let row = quantize(vec![(0, 0.3), (1, -6.0), (2, 3.0)]);
        assert_eq!(row, Some((288, vec![(1, -1), (2, 1)])));
        assert_eq!(quantize(vec![(1, 0.001)]), None);
    }
}
