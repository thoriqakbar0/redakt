# Seven-category detector comparison

The native matcher covers the same seven categories as the original Python rules detector. This comparison measures deployed implementations across languages and runtimes, not the intrinsic cost of ternary arithmetic versus regex. The repository's default tree stays Rust-only.

## Findings

Fresh measurements on 2026-11-17 used the shared Intel Xeon E5-2697 v2 Linux VM. All workers inherited CPU affinity 0 through 3. Both Python workers used CPython 3.12.16. The model embeds Unicode 13.0.0; classification agreement was checked for all 87 distinct corpus characters.

Each row reports five trials over the same 64 individual synthetic documents. The first three rows come from one paired run. The grouped row comes from a second paired run, so the two model variants were not interleaved with each other.

| Implementation | Median seconds | Minimum to maximum seconds | Peak worker RAM, MB |
| --- | ---: | ---: | ---: |
| Native ternary | 0.01171 | 0.00847 to 0.01890 | 2.53 |
| Python rules | 0.02674 | 0.01520 to 0.03198 | 16.02 |
| Official Privacy Filter | 153.328 | 142.450 to 166.744 | 4,221.62 |
| Grouped Privacy Filter | 22.315 | 21.215 to 23.086 | 6,134.17 |

Native and rules timing ranges overlap. Their short-document speed comparison is inconclusive on this VM. Tiny requests include substantial pipe transport and parent-validation costs. Native inference uses less measured worker RAM. The shared machine's load changed during the runs, so these numbers do not establish production throughput or performance on other CPUs or GPUs.

Launch plus loading plus synthetic warmup took 0.005 seconds for native, 0.107 seconds for rules, 37.224 seconds for the official model, and 27.937 seconds for grouped inference. These are separate single-run observations. Median worker CPU cost per corpus pass was 452.96 CPU-seconds for the official model and 81.23 CPU-seconds for grouped inference. Short native and rules CPU samples are near the process-counter clock-tick resolution and must not be interpreted as zero computation.

The long-document track processes four documents containing 540,739 characters per trial, using the same executable and Python version.

| Implementation | Median seconds | Minimum to maximum seconds | Median worker CPU-seconds | Peak worker RAM, MB |
| --- | ---: | ---: | ---: | ---: |
| Native ternary | 0.257 | 0.205 to 0.297 | 0.19 | 7.11 |
| Python rules | 0.309 | 0.270 to 0.340 | 0.24 | 24.16 |

These wall-time ranges also overlap. This track confirms exact outputs for longer inputs and lower measured native RAM, but does not establish a reliable speed advantage. Diagnostic profiles locate native work in matching/scanning and rules work in regex iteration and checksum processing. Profiling overhead is excluded from timing samples.

The measured native executable is 0.830384 MB with its 0.057721 MB model embedded. Rules source is 0.004091 MB and requires Python. The Privacy Filter checkpoint is 2,798.989275 MB and requires Python and PyTorch. Fresh installation estimates were 80.40 MB for rules plus Python and 3,806.11 MB for the official model and environment. These count the measured installation, including libpython even if statically linked, and exclude OS libraries. They are not minimum bundles or marginal costs when a runtime is already installed.

Rules and native returned exactly equal spans and redactions throughout every trial. Both have 89.2% exact precision and 66.0% exact recall on the fixture. Both Privacy Filter variants produced identical predictions, with 64.8% precision and 70.0% recall. This small, hand-authored synthetic fixture does not establish general accuracy. Compiling rules preserves their contextual, international, checksum-policy, and false-positive limitations.

## Method

Workers load once and answer a separate synthetic warmup. One full untimed corpus pass establishes expected outputs and warms the exercised paths. Five timed passes then send every document individually and receive full spans and redaction. Wall time includes serialization, pipes, inference, response serialization, and parent validation. It excludes startup, model loading, and downloads.

Order rotates and reverses between trials. Workers execute sequentially. Native and rules inference are single-threaded. Privacy Filter uses four compute threads and one interop thread. Every process has the same available four-core set. The official implementation uses bfloat16 parameters and the grouped adapter caches FP32 experts.

Every rules/native response must match exactly before and during timing. Model responses must have valid offsets and labels, correct common masking, and identical predictions across trials. Tokenizer warnings, changed source text, errors, timeouts, or changes to source/model/executable hashes fail the benchmark. The short documents fit the configured 512-token model context. Corpus documents are not concatenated into a different model input.

Linux process counters provide worker CPU seconds and each worker's own peak RSS after the trials, including loading and warmup. RSS excludes the parent harness and other workers, but is not total container or machine memory. Source, executable, model, corpus and checkpoint hashes, versions, load averages, trial orders, and every timing sample are retained in the raw reports.

## Evidence

- [Official paired trials](benchmarks/fair-official.json).
- [Grouped paired trials](benchmarks/fair-grouped.json).
- [Long-document trials](benchmarks/fair-scalability.json).
- [Accuracy, parity, and Unicode agreement](benchmarks/fair-accuracy.json).
- [File-size components and upstream revision](benchmarks/fair-size.json).
- [Diagnostic profiles](benchmarks/fair-profiles.txt).

The Rust runtime and model are byte-identical to the measured source snapshot. Benchmark tooling paths in the reports describe the measurement-time checkout, not files present in this Rust-only tree. Complete predictions, source fixtures, and Python reproduction remain in the separate source below. Earlier two-category measurements have different scope and must not be combined with these numbers.

## Reproduce outside the default tree

The full compiler, original rules, Python reference, fixed corpus, predictions, pinned baseline setup and benchmark programs are preserved at [reproduction commit 6ba3e585](https://github.com/thoriqakbar0/privacy-hygiene/tree/6ba3e5850af6b2b13a22b81f27bd9df619df30d5), on `thor/ternary-benchmark-reproduction`. They are intentionally excluded from the default tree.

Clone that source separately rather than copying Python tools into this Rust-only checkout.

```bash
git clone --branch thor/ternary-benchmark-reproduction --single-branch https://github.com/thoriqakbar0/privacy-hygiene.git ../privacy-hygiene-benchmark
cd ../privacy-hygiene-benchmark
git checkout 6ba3e5850af6b2b13a22b81f27bd9df619df30d5
cargo build --release --locked
python3 -m experiments.benchmark_detectors --output results/new-paired.json
python3 -m experiments.benchmark_detectors --scalability --output results/new-scalability.json
```

For Privacy Filter, follow that snapshot's README to prepare the pinned baseline environment and checkpoint. The benchmark only loads an existing local checkpoint and does not download weights. Run the harness with the baseline interpreter so rules and the model use the same Python version. Choose four CPU IDs available in your Linux affinity set.

```bash
taskset -c 0-3 .venv-baseline/bin/python -m experiments.benchmark_detectors \
  --privacy-filter \
  --baseline-python .venv-baseline/bin/python \
  --checkpoint checkpoints/privacy-filter/original \
  --output results/new-official.json \
  --predictions results/new-official-predictions.jsonl
```

Add `--grouped-cpu` and use separate output paths for the faster adapter. The `--scalability` track generates mixed corpus text repeated 100 times, clean prose repeated 5,000 times, invalid identifiers repeated 1,000 times, and long email/name/address matches. It rejects Privacy Filter because long-context boundary parity has not been established. Use `--corpus PATH` for other JSONL inputs and keep long-context model validation separate.
