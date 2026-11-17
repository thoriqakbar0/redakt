"""Compare official CPU inference with cached, expert-grouped inference."""

import copy
from dataclasses import replace
import json
from pathlib import Path
import resource
import statistics
import time

import torch
from opf import OPF

from .optimized_opf import enable_grouped_cpu
from .run_baseline import convert_spans


PROBE_INDICES = [0, 15, 28, 41]


def predict(model, rows):
    predictions = []
    for row in rows:
        result = model.redact(row['text'])
        if result.warning or result.text != row['text']:
            raise RuntimeError(f'Text round-trip failed for {row["id"]}')
        predictions.append({
            'id': row['id'], 'text': row['text'],
            'spans': convert_spans(result.detected_spans),
        })
    return predictions


def summary(samples):
    return {
        'seconds': samples,
        'median_seconds': statistics.median(samples),
        'min_seconds': min(samples),
        'max_seconds': max(samples),
    }


def profile_model(model, probes, path):
    with torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CPU]) as profile:
        predict(model, probes[:1])
    Path(path).write_text(
        profile.key_averages().table(sort_by='self_cpu_time_total', row_limit=20))


def tune(variants, probes, expected):
    samples = {name: [] for name in variants}
    mismatches = {}
    for repeat in range(5):
        configurations = list(variants)
        offset = repeat % len(configurations)
        configurations = configurations[offset:] + configurations[:offset]
        for name in configurations:
            model, threads = variants[name]
            torch.set_num_threads(threads)
            started = time.perf_counter()
            predictions = predict(model, probes)
            samples[name].append(time.perf_counter() - started)
            changed = sum(before != after for before, after in zip(expected, predictions))
            mismatches[name] = max(mismatches.get(name, 0), changed)
            print(f'{repeat + 1} {name}: {samples[name][-1]:.4f}s, '
                  f'{mismatches[name]} changed documents', flush=True)
    return samples, mismatches


def measure_corpus(model, rows):
    predictions = predict(model, rows)
    samples = []
    for repeat in range(5):
        started = time.perf_counter()
        current = predict(model, rows)
        samples.append(time.perf_counter() - started)
        if current != predictions:
            raise RuntimeError('Predictions changed between repeated runs')
        print(f'Corpus pass {repeat + 1}: {samples[-1]:.3f}s', flush=True)
    return predictions, samples


def main():
    torch.set_num_threads(4)
    torch.set_num_interop_threads(1)
    rows = [json.loads(line) for line in Path('data/synthetic.jsonl').read_text().splitlines()]
    probes = [rows[index] for index in PROBE_INDICES]
    model = OPF(model='checkpoints/privacy-filter/original', device='cpu',
                context_window_length=512)
    runtime, _ = model.get_prediction_components()
    expected = predict(model, probes)
    profile_model(model, probes, 'results/baseline-profile.txt')

    grouped = copy.copy(model)
    grouped._runtime = replace(runtime, model=enable_grouped_cpu(copy.deepcopy(runtime.model)))
    predict(grouped, probes)
    variants = {'official/4': (model, 4)}
    variants.update({f'grouped/{threads}': (grouped, threads) for threads in [1, 2, 4]})
    samples, mismatches = tune(variants, probes, expected)
    eligible = [name for name in variants if not mismatches[name]]
    if not eligible:
        raise RuntimeError('No configuration preserved probe predictions')
    chosen = min(eligible, key=lambda name: statistics.median(samples[name]))
    selected, threads = variants[chosen]
    torch.set_num_threads(threads)
    profile_model(selected, probes, 'results/optimized-baseline-profile.txt')
    predictions, full_samples = measure_corpus(selected, rows)

    original = [
        json.loads(line)
        for line in Path('results/privacy-filter.jsonl').read_text().splitlines()
    ]
    changed = [before['id'] for before, after in zip(original, predictions) if before != after]
    if changed:
        raise RuntimeError(f'Optimized predictions changed for {changed}')
    Path('results/privacy-filter-optimized.jsonl').write_text(
        ''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in predictions))
    report = {
        'configuration': chosen, 'probe_ids': [row['id'] for row in probes],
        'trials': {name: summary(values) for name, values in samples.items()},
        'probe_prediction_changes': mismatches, 'full_corpus': summary(full_samples),
        'changed_documents_vs_original': changed, 'errors': 0,
        'work': {'probe_documents_per_trial': len(probes), 'corpus_documents_per_trial': len(rows)},
        'parameter_bytes': {
            name: sum(parameter.numel() * parameter.element_size()
                      for parameter in variant.get_runtime().model.parameters())
            for name, (variant, _) in variants.items()
        },
        'max_rss_kib_including_both_resident_models': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        'notes': 'Frozen checkpoint; only expert tensors cast once to FP32 in memory. '
        'Attention, embedding and residual dtype remain BF16. No training or expert pruning. '
        'Private OPF runtime adapted only in this experiment; upstream source unmodified. '
        'Five rotated trials per configuration, then five full corpus passes. '
        'Numerical equivalence is checked on spans, not assumed for logits.',
    }
    Path('results/baseline-optimization.json').write_text(json.dumps(report, indent=2) + '\n')


if __name__ == '__main__':
    main()
