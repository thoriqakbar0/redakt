"""Check whether additional CPU threads improve the grouped inference path."""

import json
from pathlib import Path
import statistics

import torch
from opf import OPF

from .benchmark_baseline import PROBE_INDICES, measure_corpus, predict, summary, tune
from .optimized_opf import enable_grouped_cpu


def main():
    torch.set_num_threads(4)
    torch.set_num_interop_threads(1)
    rows = [json.loads(line) for line in Path('data/synthetic.jsonl').read_text().splitlines()]
    reference = [
        json.loads(line)
        for line in Path('results/privacy-filter.jsonl').read_text().splitlines()
    ]
    probes = [rows[index] for index in PROBE_INDICES]
    expected = [reference[index] for index in PROBE_INDICES]

    model = OPF(model='checkpoints/privacy-filter/original', device='cpu',
                context_window_length=512)
    enable_grouped_cpu(model.get_runtime().model)
    predict(model, probes)
    variants = {f'grouped/{threads}': (model, threads) for threads in [4, 8, 16, 32]}
    samples, mismatches = tune(variants, probes, expected)
    eligible = [name for name in variants if not mismatches[name]]
    chosen = min(eligible, key=lambda name: statistics.median(samples[name]))
    control = samples['grouped/4']
    if max(samples[chosen]) >= min(control):
        chosen = 'grouped/4'
    result = {
        'selected': chosen,
        'trials': {name: summary(values) for name, values in samples.items()},
        'changed_probe_documents': mismatches,
        'errors': 0,
        'scope': 'Five rotated trials, four fixed probe documents per trial, same cached weights',
    }
    if chosen != 'grouped/4':
        selected, threads = variants[chosen]
        torch.set_num_threads(threads)
        predictions, durations = measure_corpus(selected, rows)
        result['full_corpus'] = summary(durations)
        result['changed_documents'] = [
            before['id'] for before, after in zip(reference, predictions) if before != after
        ]
        if result['changed_documents']:
            raise RuntimeError('Thread tuning changed full-corpus predictions')
        Path('results/privacy-filter-thread-tuned.jsonl').write_text(
            ''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in predictions))
    Path('results/baseline-thread-sweep.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
