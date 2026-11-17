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
