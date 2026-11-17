<p align="center">
  <img src="assets/logo.svg" alt="privacy-hygiene logo" width="240">
</p>

# privacy-hygiene

A native Rust PII detector and redactor that executes a compiled recurrent matcher with ternary gates. It covers the same seven categories as the Python rules detector. The executable embeds its model and needs no Python, NumPy, weights download, or network access.

This is a training-free circuit experiment, not a complete anonymization tool. Compiling the rules preserves their narrow coverage and known failures. It adds no learned contextual understanding. The original Python rules detector and recorded [OpenAI Privacy Filter comparison](docs/results.md) remain available for comparison.

## Run the native ternary detector

Install Rust with Cargo, then run from the repository root.

```bash
cargo build --release --locked
printf 'SSN 123-45-6789; phone +1 (212) 555-0123' | target/release/privacy-hygiene
cargo install --path . --locked
printf '123-45-6789' | privacy-hygiene
```

The CLI reads UTF-8 stdin and emits JSON with `spans` and `redacted`. Span offsets count Unicode code points, not UTF-8 bytes. Redaction masks the union of spans, including spaces, with `█`. Original CRLF and lone CR characters outside spans remain unchanged. Invalid input or model data exits nonzero on stderr without printing stdin. Use `--help`, `--version`, or `--weights PATH` to load an alternate native compiler artifact.

The model includes packed ternary gates, ordered matching instructions, checksum transitions, and Python's Unicode classification ranges. Model size, executable size, and runtime RAM are separate measurements. See [the ternary architecture and native format](docs/ternary.md) and [the fair comparison](docs/fair-benchmark.md).

To verify both implementations, build the release binary first, then run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
uv run --extra ternary python -m unittest discover -s tests -v
```

## Run the original rules detector

Use Python 3.10 or later.

```bash
printf 'Name: Nora Chen. Mail nora@example.test!' | python3 detector.py
python3 -m unittest discover -s tests -v
python3 -m experiments.evaluate --output results/rules-optimized.json
```

The CLI reads stdin and returns JSON containing `spans` and `redacted`. Each span has `start`, `end`, and `label`. Offsets are half-open Unicode code-point positions in the original Python string. Redaction replaces covered characters, including spaces, with `█`. It preserves character length and masks the union of overlapping spans.

For repeated inference, keep the detector imported. Interpreter startup and JSON output can dominate short CLI requests.

```python
from detector import detect, redact

text = "Name: Nora Chen."
spans = detect(text)
masked = redact(text, spans)
```

## Compile or run the Python ternary backend

The optional Python reference backend requires NumPy. It executes the same compiled model as the native runtime and supports all seven rule categories.

```bash
uv run --extra ternary python -m experiments.compile_ternary --native-output models/weights.bin
printf 'SSN 123-45-6789; phone +1 (212) 555-0123' | uv run --extra ternary python -m experiments.ternary
uv run --extra ternary python -m unittest discover -s tests -v
```

[`experiments/compile_ternary.py`](experiments/compile_ternary.py) compiles the rules into deterministic [`models/weights.npz`](models/weights.npz) and a native artifact, with five trits per packed byte. [`experiments/ternary.py`](experiments/ternary.py) executes the compiled recurrent matcher. Regex parsing happens during compilation, not inference. See [the architecture and evaluation commands](docs/ternary.md) for scope and limitations. The earlier rule and Privacy Filter results remain unchanged.

## Compare deployed detectors

Build the release executable, then compare loaded processes on the same documents. Each trial verifies complete spans and redactions before accepting its timings.

```bash
cargo build --release --locked
python3 -m experiments.benchmark_detectors --output results/fair-paired.json
python3 -m experiments.benchmark_detectors --scalability --output results/fair-scalability.json
```

The first command compares the unchanged corpus. The second compares generated long mixed, clean, invalid-checksum, and variable-length inputs. See [the fair comparison guide](docs/fair-benchmark.md) for the three-way Privacy Filter command, measurement boundaries, and recorded results. The benchmark uses Linux process counters.

## Supported rules

| Category | Coverage |
| --- | --- |
| Email | ASCII local parts and dotted domains |
| Phone | Separated North American numbers |
| SSN | Hyphenated US numbers with syntax exclusions |
| Card | 13 to 19 digits with a valid Luhn checksum |
| IBAN | Uppercase GB, DE, FR, ES, NL, and IT formats with length and mod-97 checks |
| Name and address | Narrow, case-sensitive forms after explicit field cues |

Checksums validate mathematical form, not assignment, ownership, or privacy. The rules miss common contextual and international formats. Long or hostile inputs have no tested performance or memory guarantee.

## Run Privacy Filter

Reproduce the recorded run with Linux, Python 3.12, and an isolated environment. Set `OPF_SOURCE` to a clone of [openai/privacy-filter](https://github.com/openai/privacy-filter) at revision `f7f00ca7fb869683eb732c010299d901457f19c3`. The path below is the reference checkout used for the recorded run.

```bash
OPF_SOURCE="$HOME/work/oss/privacy-filter"

git -C "$OPF_SOURCE" rev-parse HEAD
uv venv --python 3.12 .venv-baseline
uv pip install --python .venv-baseline/bin/python -r experiments/requirements-baseline.txt
uv pip install --python .venv-baseline/bin/python --no-deps -e "$OPF_SOURCE"

.venv-baseline/bin/python -m experiments.run_baseline data/synthetic.jsonl
.venv-baseline/bin/python -m experiments.evaluate \
  --predictions results/privacy-filter.jsonl \
  --output results/privacy-filter-scores.json
.venv-baseline/bin/python -m experiments.measure_size --baseline-source "$OPF_SOURCE"
```

The runner downloads the pinned checkpoint into ignored `checkpoints/`, then uses the official CPU API with bfloat16 weights and default Viterbi calibration. It aborts on tokenizer round-trip warnings or inconsistent repeated predictions. It performs no training or corpus-specific calibration.

## Run the optimized CPU adapter

The optional adapter caches expert tensors as FP32 and groups tokens by expert. Attention and residual activations remain bfloat16. It requires more RAM and depends on internals of the pinned OPF revision. The default baseline command above uses the unmodified official implementation.

```bash
.venv-baseline/bin/python -m experiments.run_baseline data/synthetic.jsonl \
  --grouped-cpu --threads 4 \
  --output results/privacy-filter-grouped-rerun.jsonl
.venv-baseline/bin/python -m experiments.evaluate \
  --predictions results/privacy-filter-grouped-rerun.jsonl \
  --output results/privacy-filter-grouped-rerun-scores.json
.venv-baseline/bin/python -m unittest discover -s tests/opf -v
```

## Reproduce the optimization measurements

Run the official baseline first. These scripts compare outputs, alternate or rotate configurations, and save timing samples and profiles. The model benchmarks require full-corpus span equality before accepting an optimized configuration.

```bash
.venv-baseline/bin/python -m experiments.benchmark_rules
.venv-baseline/bin/python -m experiments.benchmark_baseline
.venv-baseline/bin/python -m experiments.tune_grouped_threads
```

The preserved detector is [results/detector-before.py](results/detector-before.py). No output memoization, training, expert pruning, or threshold fitting is used. See [inference optimization](docs/results.md#inference-optimization) for measured gains and their limits.

## Project files

Run experiment modules from the repository root. The Rust application lives under `src/`, compiled model artifacts under `models/`, and the Python compiler, reference runtime, and experiment tooling under `experiments/`. The standalone rules detector stays at the root.

| File | Purpose |
| --- | --- |
| [src/](src/) | Native packed ternary inference and JSON CLI |
| [detector.py](detector.py) | Rule matching and character masking |
| [models/](models/) | Compiled NPZ and native binary model artifacts |
| [experiments/ternary.py](experiments/ternary.py) | Python numerical reference runtime |
| [experiments/compile_ternary.py](experiments/compile_ternary.py) | Deterministic ternary model compiler |
| [experiments/benchmark_detectors.py](experiments/benchmark_detectors.py) | Matched persistent-process timing, memory, output checks, and artifact hashes |
| [experiments/evaluate.py](experiments/evaluate.py) | Corpus validation, span scoring, and character coverage |
| [experiments/run_baseline.py](experiments/run_baseline.py) | Official model runner and shared prediction format |
| [experiments/optimized_opf.py](experiments/optimized_opf.py) | Optional grouped CPU adapter |
| [data/synthetic.jsonl](data/synthetic.jsonl) | The fixed, labeled corpus |
| [docs/results.md](docs/results.md) | Measurements, methodology, boundaries, and limitations |
| [results/](results/) | Raw predictions, scores, profiles, environment records, and source hashes |

Downloaded model weights and virtual environments are excluded from Git. The small compiled artifacts in `models/` are checked in.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and dataset requirements. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

The [public-source readiness record](docs/open-source-readiness.md) lists audit scope and remaining publication steps.

## License

This project's original code, compiled circuit artifacts, documentation, and synthetic fixtures are licensed under [Apache-2.0](LICENSE). Third-party packages and the separately downloaded OpenAI Privacy Filter checkpoint retain their own licenses. See [NOTICE](NOTICE).
