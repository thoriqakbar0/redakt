# Matcher-loop performance

A 2026-11-17 experiment compared the release CLI at `43ec192` with the optimized matcher on a shared Intel Xeon E5-2697 v2 Linux VM. The optimization keeps sequential instructions in the active execution state. Only alternative branches enter the backtracking stack. Branch priority, captures, assertions, and checksum policy remain unchanged.

The workload contains ten JSONL records. Each record repeats a mixed email, phone, name, address, and card sentence 1,000 times. A trial reads 1,180,280 input bytes and emits 50,000 spans. Every response must be byte-identical across all three variants. There were zero errors.

| CLI variant | Median wall seconds | Minimum to maximum seconds | Median worker CPU seconds |
| --- | ---: | ---: | ---: |
| Original default | 0.528 | 0.471 to 0.623 | 0.509 |
| Optimized default | 0.374 | 0.339 to 0.410 | 0.338 |
| Optimized with `--contextual` | 0.448 | 0.384 to 0.521 | 0.425 |

The optimized default took 29% less median wall time. Its measured range did not overlap the baseline range. Contextual mode performs an additional scan, and its wall-time range overlaps the original default. These runs do not establish a reliable speed advantage for contextual mode or predict performance on other workloads.

Each variant ran once for warmup and five times for measurement. Trial order rotates and reverses. Timing includes process startup, model loading, stdin, inference, serialization, and stdout capture. Response validation happens after timing. Both executables use Cargo's release profile. The shared machine had 32 visible CPUs and a load average around 8 before the experiment, so the runs were interleaved rather than measured in separate batches.

An untimed baseline `perf` profile located 26.7% of samples in `Detector::run`, 7.5% in `Detector::detect`, and 9.6% in allocation functions. The worker is single-threaded, and worker CPU time tracks wall time. Removing routine stack traffic addresses that measured matcher cost. The existing backtracking algorithm still has no tested hostile-input worst-case guarantee.

The optimized prototype also matched all 289 frozen oracle records and 300 seeded mixed-input responses against the original executable. The final build passed the frozen oracle and all contextual CLI tests. [Raw timing samples, CPU samples, and input/output hashes](benchmarks/contextual-performance.json) accompany this report.

## Reproduce the workload

Build both revisions with `cargo build --release --locked` in separate checkouts. Set `baseline` and `optimized` to their executable paths. The following optional Python standard-library script generates the fixture and performs the paired wall-time measurement. Python is not a detector dependency.

```bash
python3 - "$baseline" "$optimized" <<'PY'
import json
import statistics
import subprocess
import sys
import time

sentence = ('Contact nora@example.test or +1 (212) 555-0123. '
            'Name: Nora Chen. Address: 42 Maple Street. '
            'Card 4111 1111 1111 1111.\n')
request = ((json.dumps({'id': 'mixed', 'text': sentence * 1000}) + '\n') * 10).encode()
commands = [[sys.argv[1], '--jsonl'], [sys.argv[2], '--jsonl'],
            [sys.argv[2], '--jsonl', '--contextual']]
expected = subprocess.run(commands[0], input=request, capture_output=True, check=True).stdout
records = [json.loads(line) for line in expected.splitlines()]
assert len(records) == 10
assert sum(len(record['spans']) for record in records) == 50000
samples = [[] for command in commands]
for trial in range(6):
    order = list(range(3))
    order = order[trial % 3:] + order[:trial % 3]
    if trial % 2:
        order.reverse()
    for variant in order:
        start = time.perf_counter()
        result = subprocess.run(commands[variant], input=request, capture_output=True, check=True)
        elapsed = time.perf_counter() - start
        assert not result.stderr and result.stdout == expected
        if trial:
            samples[variant].append(elapsed)
for values in samples:
    print({'median_seconds': statistics.median(values), 'range': [min(values), max(values)]})
print({'errors': 0, 'documents_per_trial': 10, 'spans_per_trial': 50000})
PY
```

## Model storage and CLI serialization

On 2026-11-19, release builds compared baseline `ac6ae8d`, model-only optimization `c531137`, and combined model/CLI optimization `3b63cda`. The model precomputes a starting-character lookup for each head and reuses backtracking storage between candidate matches. Nested assertions retain separate storage. The CLI serializes typed responses directly instead of building an intermediate JSON value tree.

The workload repeats the sentence above 1,000 times per document and sends 50 JSONL documents. Each trial reads 5,901,400 bytes and emits 250,000 spans. Every variant produced identical parsed responses, with zero errors. JSON object key order changes in the typed serializer, as permitted by the CLI contract. The model artifact, detection policy, Unicode offsets, and per-record flushing remain unchanged.

| Variant | Median wall seconds | Wall range, seconds | Median worker CPU seconds | CPU range, seconds |
| --- | ---: | ---: | ---: | ---: |
| Baseline | 1.509 | 1.320 to 1.784 | 1.416 | 1.301 to 1.528 |
| Model changes, original CLI serialization | 1.149 | 1.067 to 1.350 | 1.053 | 1.024 to 1.111 |
| Model and CLI changes | 0.774 | 0.688 to 0.911 | 0.709 | 0.608 to 0.778 |

The combined CLI takes 49% less median wall time, with non-overlapping wall ranges. Model changes alone use 26% less median worker CPU time, with non-overlapping CPU ranges. Their wall ranges overlap slightly, so the model-only wall-time improvement is less certain. These are end-to-end CLI measurements, not isolated inference timings or detection-quality improvements.

Each variant ran one warmup and seven measured trials, alternating forward and reverse order. Commands used `taskset -c 2` on a shared Intel Xeon E5-2697 v2 Linux VM with 32 visible CPUs and Rust 1.100.0. Load average before the run was about 1. Timing includes startup, model loading, stdin, detection, redaction, serialization, and captured stdout. Worker CPU time comes from child-process user plus system CPU usage. Warmup responses were parsed and compared across variants; every timed response was then byte-compared with its validated warmup outside the timed region.

Separate untimed `perf` runs placed 25.4% of baseline samples in `Detector::run`, 13.9% in `Detector::detect`, and additional samples in allocation and JSON value serialization. After the combined changes, `Detector::run` accounted for 51.6% of samples. The remaining matcher is the main CPU cost. Sample percentages describe each separate profile, not a comparison of absolute execution time. No hostile-input bound is claimed.

[Raw samples, fixture hashes, and environment details](benchmarks/runtime-performance.json) accompany this report. The earlier lookup-only experiment had overlapping timing ranges and did not establish a standalone speedup.

### Reproduce the follow-up

Build the three revisions above with `cargo build --release --locked` in separate checkouts. Set `baseline`, `model`, and `optimized` to their binary paths. This optional Python standard-library script needs Linux `taskset`; choose a CPU allowed on your machine.

```bash
python3 - "$baseline" "$model" "$optimized" <<'PY'
import json
import resource
import statistics
import subprocess
import sys
import time

sentence = ('Contact nora@example.test or +1 (212) 555-0123. '
            'Name: Nora Chen. Address: 42 Maple Street. '
            'Card 4111 1111 1111 1111.\n')
request = ((json.dumps({'id': 'mixed', 'text': sentence * 1000}) + '\n') * 50).encode()
commands = [['taskset', '-c', '2', binary, '--jsonl'] for binary in sys.argv[1:]]
expected = None
outputs = [None] * 3
wall_samples = [[], [], []]
cpu_samples = [[], [], []]
for trial in range(8):
    for variant in ([0, 1, 2] if trial % 2 == 0 else [2, 1, 0]):
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        started = time.perf_counter()
        result = subprocess.run(commands[variant], input=request, capture_output=True, check=True)
        elapsed = time.perf_counter() - started
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
        assert not result.stderr
        if trial == 0:
            records = [json.loads(line) for line in result.stdout.splitlines()]
            assert len(records) == 50
            assert sum(len(record['spans']) for record in records) == 250000
            if expected is None:
                expected = records
            assert records == expected
            outputs[variant] = result.stdout
        assert result.stdout == outputs[variant]
        if trial:
            wall_samples[variant].append(elapsed)
            cpu_samples[variant].append(cpu)
for wall, cpu in zip(wall_samples, cpu_samples):
    print({'wall_median': statistics.median(wall), 'wall_range': [min(wall), max(wall)],
           'cpu_median': statistics.median(cpu), 'cpu_range': [min(cpu), max(cpu)]})
print({'errors': 0, 'documents': 50, 'spans': 250000})
PY
```
