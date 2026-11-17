# Comparing deployed detectors fairly

This benchmark compares the native compiled matcher, the original Python rules detector, and optionally the pinned OpenAI Privacy Filter CPU implementation. It measures the same document requests through persistent JSONL processes. It compares deployed implementations across languages and runtimes, not the intrinsic cost of ternary weights versus regex.

## Reproduce

Build the native binary and run the correctness suite first.

```bash
cargo build --release --locked
cargo test --locked
uv run --extra ternary python -m unittest discover -s tests -v
python3 -m experiments.benchmark_detectors --output results/fair-paired.json
```

For a three-way comparison, prepare the pinned baseline environment and checkpoint as described in the README. The checkpoint path points to the `original` directory containing `model.safetensors`. This benchmark only loads existing local files. It does not download weights.

Run the harness with the baseline interpreter so the rules worker and Privacy Filter use the same Python version. Choose four CPU IDs available in your Linux affinity set.

```bash
taskset -c 0-3 .venv-baseline/bin/python -m experiments.benchmark_detectors \
  --privacy-filter \
  --baseline-python .venv-baseline/bin/python \
  --checkpoint checkpoints/privacy-filter/original \
  --output results/fair-official.json \
  --predictions results/fair-official-predictions.jsonl
```

Add `--grouped-cpu` and use different output paths to measure the optional faster adapter separately. Its cached FP32 experts increase RAM use. The official implementation remains the primary baseline.

## Measurement boundaries

Every worker loads once and answers a separate synthetic warmup request before timing. The harness records launch plus loading plus warmup as a separate observation. This is not a pure model-loading measurement.

One full untimed corpus pass then establishes expected outputs and warms the exercised paths. Every subsequent timed pass must reproduce those outputs.

Each timed trial sends all corpus documents individually, receives complete spans and redacted text, and validates every response. Wall time includes request serialization, pipe transport, inference, response serialization, and parent validation. For these short documents, transport and validation can dominate the rules and native results. Do not interpret those numbers as pure inference kernel timings.

The harness requires at least five trials and rotates and reverses execution order. Workers run sequentially within each trial. All inherit the same CPU affinity. Rules and native inference are single-threaded. Privacy Filter uses four compute threads and one interop thread by default. This gives each deployed implementation the same available CPU set, without forcing identical thread counts.

Before timing, and during every trial, rules and ternary must return exactly equal spans and redactions. Privacy Filter has a different detection policy. Its outputs must have valid offsets and labels, correct common masking, and remain identical across trials. Tokenizer warnings or changed source text abort the run. Errors and timeouts fail the benchmark rather than becoming timing samples.

Worker CPU seconds come from Linux process counters. Short trials may fall below their clock-tick resolution. Worker peak RSS is read from each worker's own Linux high-water mark after the trials. It includes model loading and warmup. It excludes the parent harness and the other workers, but is not a total machine or container memory measurement.

The JSON report records all timing samples, medians, ranges, trial order, startup observations, per-worker CPU and RSS, CPU affinity, machine load, Python and package versions, source and executable hashes, and checkpoint hashes. Profiling runs are separate from timing.

## Accuracy and scope

The common corpus is the unchanged 64-document synthetic fixture. Its short documents fit the configured 512-token Privacy Filter context. The benchmark preserves document boundaries rather than concatenating the corpus into a new input.

Score the saved Privacy Filter predictions with the same evaluator used for rules.

```bash
python3 -m experiments.evaluate \
  --predictions results/fair-official-predictions.jsonl \
  --output results/fair-official-scores.json
```

Exact rules parity means the compiled matcher inherits the rules detector's 89.2% exact precision and 66.0% exact recall on this fixture. It also inherits false positives, missed contextual values, and international limitations. Comparing a two-category circuit against seven-category rules would not answer the same question.

Use `--corpus PATH` for other JSONL workloads. Long-document scalability should be a separate native-versus-rules track unless the model's context and boundary behavior are also verified. Repeated synthetic data and a shared VM do not establish production throughput or accuracy.

`--scalability` generates four deterministic long documents from the selected corpus. They contain mixed corpus text repeated 100 times, clean prose repeated 5,000 times, invalid identifiers repeated 1,000 times, and long email, name, and address matches. This track rejects `--privacy-filter` because long-context parity has not been established. Its report includes the exact request hash and total character count.

## Results

These fresh runs were collected on 2026-11-17 on the shared Intel Xeon E5-2697 v2 Linux VM. All workers inherited CPU affinity 0 through 3. Both Python workers used CPython 3.12.16. The compiled model uses Unicode 13.0.0, and classification agreement was checked for all 87 distinct corpus characters. The official baseline uses the pinned upstream revision documented in `docs/results.md` and bfloat16 parameters. The grouped adapter caches FP32 experts.

Each row reports five trials over the same 64 individual documents. The first three rows come from one paired run. The grouped row comes from a second paired run, so the two model variants were not interleaved with each other.

| Implementation | Median seconds | Minimum to maximum seconds | Peak worker RAM, MB |
| --- | ---: | ---: | ---: |
| Native ternary | 0.01171 | 0.00847 to 0.01890 | 2.53 |
| Python rules | 0.02674 | 0.01520 to 0.03198 | 16.02 |
| Official Privacy Filter | 153.328 | 142.450 to 166.744 | 4,221.62 |
| Grouped Privacy Filter | 22.315 | 21.215 to 23.086 | 6,134.17 |

The native and rules timing ranges overlap. The short-document speed comparison is inconclusive on this VM. Tiny requests include substantial transport and parent-validation costs. Native inference uses much less measured worker RAM. The shared machine's load changed during the runs, and these numbers do not establish production throughput or performance on other CPUs or GPUs.

Launch plus loading plus synthetic warmup took 0.005 seconds for native, 0.107 seconds for rules, 37.224 seconds for the official model, and 27.937 seconds for the grouped model. These are separate single-run observations. Median worker CPU cost for a corpus pass was 452.96 CPU-seconds for the official model and 81.23 CPU-seconds for grouped inference. Short native and rules CPU samples are near the process-counter clock-tick resolution and must not be interpreted as zero computation.

The long-document track processes four documents containing 540,739 characters per trial. It uses the same executable and Python version as the corpus track.

| Implementation | Median seconds | Minimum to maximum seconds | Median worker CPU-seconds | Peak worker RAM, MB |
| --- | ---: | ---: | ---: | ---: |
| Native ternary | 0.257 | 0.205 to 0.297 | 0.19 | 7.11 |
| Python rules | 0.309 | 0.270 to 0.340 | 0.24 | 24.16 |

These wall-time ranges also overlap. This track does not establish a reliable speed advantage for native inference. It confirms identical complete outputs for longer inputs and lower measured native RAM use. Separate diagnostic profiles locate native work in the matcher and scanning loop and rules work in regex iteration and checksum processing. Their overhead is excluded from every timing sample.

The native executable is 0.830384 MB with its 0.057721 MB model embedded. The optional compressed Python artifact is 0.016080 MB. Rules source is 0.004091 MB and requires Python. The Privacy Filter checkpoint is 2,798.989275 MB and requires Python and PyTorch. Fresh installation estimates are 80.40 MB for rules plus Python and 3,806.11 MB for the official model and its environment. These count the existing installation, including libpython even if statically linked, and exclude OS libraries. They are not minimum possible bundles or marginal costs when a runtime is already installed.

Rules and native return exactly the same spans and redactions throughout all trials. Both have 89.2% exact precision and 66.0% exact recall on this corpus. Both Privacy Filter variants produce identical predictions, with 64.8% precision and 70.0% recall. This preserves the fixture's policy and does not establish general accuracy.

Raw evidence is in [official trials](../results/fair-official.json), [grouped trials](../results/fair-grouped.json), [long-document trials](../results/fair-scalability.json), [accuracy and parity](../results/fair-accuracy.json), [file-size components](../results/fair-size.json), and [diagnostic profiles](../results/fair-profiles.txt). Complete predictions are retained for [native](../results/fair-ternary-predictions.jsonl), [official](../results/fair-official-predictions.jsonl), and [grouped](../results/fair-grouped-predictions.jsonl) inference.

Earlier measurements in `docs/results.md` and the original two-category circuit timings have different boundaries and are not mixed into these results.
