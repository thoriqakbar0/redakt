# Trained English name detector

This experimental Rust example loads a 52,275-byte model with 14,336 learned ternary weights and 12,350 learned word hashes. It predicts person-name spans as pairs of Unicode code-point offsets, with an exclusive end. Run it from the repository root.

```sh
printf '%s\n' 'Sarah called this morning.' |
  cargo run --quiet --release --locked --example trained_names -- predict experiments/trained-names/names.trits
# [[0,5]]
```

Training first builds a vocabulary of words that occur inside annotated names at least one third of the time. The classifier then learns from 23 hashed features per token, including spelling, case, and whether that word or a neighbor is in the vocabulary. Vocabulary membership adds eight points to the token score. During supervised margin updates, missed positive tokens receive twice the update of negative tokens. The artifact stores five ternary weights per byte and four bytes per vocabulary entry.

## Data and evaluation

NVIDIA created [Nemotron-PII](https://huggingface.co/datasets/nvidia/Nemotron-PII), distributed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Preparation extracts name annotations from synthetic English documents, checks their offsets case-insensitively against the original text, and merges overlapping or whitespace-adjacent name parts. Other text serves as negative training examples. All 100,000 training rows were processed, including one repeated document. The 100,000 published test documents had zero exact text matches with training. Templates and names may recur.

The test set contains 97,012 normalized name spans. An exact match requires both ends to agree. Baseline counts use the default Rust CLI's JSONL spans labeled `name`. Each detector ran separately.

| Detector | Exact matches | Predicted spans | Precision | Recall |
| --- | ---: | ---: | ---: | ---: |
| Updated trained example | 67,552 | 83,203 | 81.2% | 69.6% |
| Original trained example | 47,823 | 66,846 | 71.5% | 49.3% |
| Default CLI | 18,913 | 25,472 | 74.3% | 19.5% |

Candidate selection used every twentieth training row (5,000 documents) for validation and the other 95,000 for fitting. The original approach found 1,593 of 3,213 validation names. The selected approach found 2,314 with 2,891 predictions. After selection, training used the entire 100,000-row training split. The new model detects both words in “I met Nora Chen yesterday.” Its vocabulary increases artifact size from 2,875 to 52,275 bytes. These measurements cover synthetic documents and name detection only.

## Reproduce

Use Rust and Python with PyArrow (measured with 26.0.1). Set `data` to a local scratch directory. Run from the repository root in Bash.

```bash
set -eu -o pipefail
data=$(mktemp -d)
revision=b70ffaf5ff39e079776134c5bf4381f00a9fd1ed
for split in train test; do
  curl -fLsS --retry 3 "https://huggingface.co/datasets/nvidia/Nemotron-PII/resolve/$revision/data/$split-00000-of-00001.parquet" -o "$data/$split.parquet"
  python3 experiments/trained-names/prepare.py "$data/$split.parquet" > "$data/$split.jsonl"
done
cargo build --release --locked --example trained_names
bin=target/release/examples/trained_names
"$bin" train "$data/names.trits" < "$data/train.jsonl"
cmp "$data/names.trits" experiments/trained-names/names.trits
"$bin" eval "$data/names.trits" < "$data/test.jsonl"
```

The trained artifact SHA-256 is `044c3b86982c355a14f04462ce0b2466604f7d56207fe00c8e4870721f0df484`.
