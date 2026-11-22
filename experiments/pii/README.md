# English PII redaction

The default CLI combines Redakt's existing rules with a learned sequence tagger. Its input is a UTF-8 document. Its output is redacted text, or typed, half-open Unicode code-point spans plus redacted text with `--jsonl`.

## Training

The corpus is NVIDIA's [Nemotron-PII](https://huggingface.co/datasets/nvidia/Nemotron-PII) at revision `b70ffaf5ff39e079776134c5bf4381f00a9fd1ed`, available under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). The trainer takes all 100,000 synthetic English training documents. The published test split contains a separate 100,000 documents. Preliminary model selection uses every twentieth row of the training split as validation and fits the other 95,000 rows.

Preparation includes every annotation type. It combines whitespace-adjacent name parts and normalizes first and last names to `name`, dates of birth to `date`, street addresses to `address`, phone numbers to `phone`, payment card numbers to `card`, and passwords and API keys to `secret`. The normalized corpus has 52 labels. The tagger also learns the outside class so ordinary text can stay visible.

The trainer learns a word-to-class vocabulary from labeled data and fits a multiclass averaged perceptron using word shape, neighboring words, number and punctuation patterns, field cues, and neighboring tags. It quantizes each feature's class weights to a signed ternary vector with a shared scale. The two directions have 2,423,100 nonzero ternary weights in total. The shared vocabulary contains 133,067 word hashes. The packed sequence model is 6,666,775 bytes and the cue model is 990,828 bytes.

## Reproduce the corpus

Use Bash, Python with PyArrow, and the repository's locked Rust dependencies. Run from the repository root.

```bash
set -eu -o pipefail
data=$(mktemp -d)
revision=b70ffaf5ff39e079776134c5bf4381f00a9fd1ed
for split in train test; do
  curl -fLsS --retry 3 "https://huggingface.co/datasets/nvidia/Nemotron-PII/resolve/$revision/data/$split-00000-of-00001.parquet" -o "$data/$split.parquet"
  python3 experiments/pii/prepare.py "$data/$split.parquet" > "$data/$split.jsonl"
done
```

Exact typed recall requires both boundaries and the class to match. Full coverage counts a gold value when the union of the returned spans covers every character, regardless of label. The evaluator drives the release CLI via JSONL. It scores a few hand-written smoke cases separately.

## Fit and evaluate

```bash
cargo build --release --locked --example train_pii --example train_cues
target/release/examples/train_pii "$data/train.jsonl" "$data/forward.json" 4
target/release/examples/train_pii "$data/train.jsonl" "$data/reverse.json" 4 reverse
python3 experiments/pii/pack.py "$data/pii.bin" "$data/forward.json" "$data/reverse.json"
target/release/examples/train_cues train "$data/train.jsonl" "$data/pii-cues.bin"
python3 experiments/pii/bundle.py models/weights.bin "$data/pii.bin" "$data/pii-cues.bin" "$data/redakt.bin"
cmp "$data/redakt.bin" models/redakt.bin
cargo build --release --locked
python3 experiments/pii/evaluate.py "$data/test.jsonl" --binary target/release/redakt
```

The two taggers scan in opposite directions. Inference combines their class scores and uses a validation-selected outside-class offset of 768 fixed-point units to favor recall. A second supervised model learns high-purity associations between the words preceding a value, its character pattern, and its annotated class. The CLI uses these associations to correct structured fields such as routing numbers and fax numbers.

## Measured results

Plain `redakt --jsonl` on all 100,000 published test documents returned 736,388 exact typed matches out of 803,713 gold spans. Micro recall was **91.62%** and precision was **83.12%**. The union of the redactions fully covered 94.53% of annotated values. Name recall was 95.32%. Across the 52 annotated classes, 36 exceeded 90% exact recall individually. See [per-class counts and examples](results.json).

On the repository’s independent 45-document fixture, the CLI found 31 of 33 annotations and returned 27 extra predictions. That fixture's privacy policy leaves public URLs visible. See [the independent evaluation](../../docs/accuracy.md). These two corpora use different annotation policies and neither measures accuracy on production data.
