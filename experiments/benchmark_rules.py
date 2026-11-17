"""Compare the preserved and optimized rules without changing detection policy."""

import argparse
import cProfile
from dataclasses import asdict
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import pstats
import random
import statistics
import subprocess
import sys
import time

import detector


def load_reference(path):
    spec = importlib.util.spec_from_file_location("rules_reference", path)
    reference = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = reference
    spec.loader.exec_module(reference)
    return reference


def output(module, text):
    spans = module.detect(text)
    return {"spans": [asdict(span) for span in spans], "redacted": module.redact(text, spans)}


def run_api(module, texts, iterations):
    checksum = 0
    started = time.perf_counter()
    for repeat in range(iterations):
        for text in texts:
            spans = module.detect(text)
            checksum += len(module.redact(text, spans)) + len(spans)
    return time.perf_counter() - started, checksum


def run_cli(script, text, expected):
    started = time.perf_counter()
    process = subprocess.run(
        [sys.executable, str(script)], input=text, text=True, capture_output=True, check=True
    )
    duration = time.perf_counter() - started
    if json.loads(process.stdout) != expected:
        raise AssertionError("CLI output changed")
    return duration, len(process.stdout)


def summarize(samples):
    return {"seconds": samples, "median_seconds": statistics.median(samples),
            "min_seconds": min(samples), "max_seconds": max(samples)}


def profile(module, workloads):
    trace = cProfile.Profile()
    trace.enable()
    for texts, iterations in workloads.values():
        run_api(module, texts, 1)
    trace.disable()
    stream = io.StringIO()
    pstats.Stats(trace, stream=stream).strip_dirs().sort_stats("cumulative").print_stats(16)
    return stream.getvalue()


def validate_intervals(reference):
    generator = random.Random(3904)
    cases = 0
    for size in range(1, 100):
        text = "🦊" + "x" * (size - 1)
        for repeat in range(20):
            offsets = []
            for count in range(generator.randrange(20)):
                start = generator.randrange(size)
                end = generator.randrange(start + 1, size + 1)
                offsets.append((start, end, "name"))
            expected = reference.redact(text, [reference.Span(*span) for span in offsets])
            actual = detector.redact(text, [detector.Span(*span) for span in offsets])
            if actual != expected:
                raise AssertionError("Interval redaction changed")
            cases += 1
    return cases


def validate_texts(reference, texts):
    generator = random.Random(417)
    for repeat in range(256):
        parts = generator.choices(texts, k=generator.randrange(1, 8))
        text = generator.choice([" 🦊 ", "\n", " / ", "\t"]).join(parts)
        if output(detector, text) != output(reference, text):
            raise AssertionError("Concatenated text detection changed")
    return 256


def benchmark(reference, workloads, repeats):
    results = {}
    modules = {"reference": reference, "optimized": detector}
    for name, (texts, iterations) in workloads.items():
        expected = [output(reference, text) for text in texts]
        if [output(detector, text) for text in texts] != expected:
            raise AssertionError(f"Output changed for {name}")
        samples = {label: [] for label in modules}
        checksums = []

        for repeat in range(repeats):
            order = list(modules) if repeat % 2 == 0 else list(reversed(modules))
            for label in order:
                duration, checksum = run_api(modules[label], texts, iterations)
                samples[label].append(duration / iterations)
                checksums.append(checksum)
        if len(set(checksums)) != 1:
            raise AssertionError("Timed work differed between implementations")
        results[name] = {
            "documents_per_pass": len(texts), "characters_per_pass": sum(map(len, texts)),
            "iterations_per_sample": iterations, "errors": 0,
            "consumed_output_checksum_per_sample": checksums[0],
            **{label: summarize(values) for label, values in samples.items()},
        }
    return results


def benchmark_cli(reference, reference_path, workloads, repeats):
    scripts = {"reference": reference_path, "optimized": Path("detector.py")}
    results = {}
    for name, (texts, iterations) in workloads.items():
        text = "\n".join(texts)
        expected = output(reference, text)
        samples = {label: [] for label in scripts}
        sizes = []
        for repeat in range(repeats):
            order = list(scripts) if repeat % 2 == 0 else list(reversed(scripts))
            for label in order:
                duration, size = run_cli(scripts[label], text, expected)
                samples[label].append(duration)
                sizes.append(size)
        if len(set(sizes)) != 1:
            raise AssertionError("CLI output sizes differed")
        results[name] = {"characters": len(text), "errors": 0,
                         "json_output_characters": sizes[0],
                         **{label: summarize(values) for label, values in samples.items()}}
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, default=Path("results/detector-before.py"))
    parser.add_argument("--output", type=Path, default=Path("results/rules-optimization.json"))
    parser.add_argument("--repeats", type=int, default=7)
    args = parser.parse_args()
    if args.repeats < 5:
        parser.error("At least five repeats are required")
    reference = load_reference(args.reference)
    corpus = Path("data/synthetic.jsonl")
    texts = [json.loads(line)["text"] for line in corpus.read_text().splitlines()]
    workloads = {
        "fixture": (texts, 100),
        "mixed_document": (["\n".join(texts) * 100], 2),
        "clean_document": (["Plain ordinary sentences without personal information. " * 5000], 2),
    }
    result = {
        "claim": "Local rules detect-plus-redact latency improves with identical outputs.",
        "acceptance": "Nonoverlapping measured ranges, unchanged outputs, zero errors.",
        "environment": {"python": sys.version, "platform": platform.platform(),
                        "cpu_count": os.cpu_count(), "load_average_start": os.getloadavg()},
        "repeats": args.repeats,
        "order": "Alternating reference/optimized, reversed within every second pair",
        "scope": "API includes detection and masking, excludes imports and I/O. CLI includes "
        "fresh interpreter, imports, stdin, JSON output, and subprocess transport; OS cache warm.",
        "dataset_sha256": hashlib.sha256(corpus.read_bytes()).hexdigest(),
        "reference_sha256": hashlib.sha256(args.reference.read_bytes()).hexdigest(),
        "optimized_sha256": hashlib.sha256(Path("detector.py").read_bytes()).hexdigest(),
        "source_bytes": {
            "reference": args.reference.stat().st_size,
            "optimized": Path("detector.py").stat().st_size,
        },
        "random_interval_cases_verified": validate_intervals(reference),
        "random_concatenated_text_cases_verified": validate_texts(reference, texts),
        "api": benchmark(reference, workloads, args.repeats),
        "cli": benchmark_cli(reference, args.reference, workloads, args.repeats),
        "profiles_excluded_from_timing": {"reference": profile(reference, workloads),
                                          "optimized": profile(detector, workloads)},
        "load_average_end": os.getloadavg(),
        "limitations": "Shared VM, synthetic repeated documents, no production throughput claim. "
        "No output memoization. CLI startup may obscure small inference wins.",
    }
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(args.output)


if __name__ == "__main__":
    main()
