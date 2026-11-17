"""Run the official Privacy Filter CPU implementation without training."""

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import resource
import statistics
import time

MODEL_REVISION = "7ffa9a043d54d1be65afb281eddf0ffbe629385b"
LABELS = {
    "private_email": "email",
    "private_phone": "phone",
    "private_person": "name",
    "private_address": "address",
}


def convert_spans(detected):
    return [
        {"start": span.start, "end": span.end, "label": LABELS.get(span.label, span.label)}
        for span in detected
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus", type=Path)
    parser.add_argument("--checkpoint", type=Path, default=Path("checkpoints/privacy-filter"))
    parser.add_argument("--output", type=Path, default=Path("results/privacy-filter.jsonl"))
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--grouped-cpu", action="store_true")
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args()
    if args.threads < 1 or args.repeats < 1:
        parser.error("threads and repeats must be positive")

    import torch
    from huggingface_hub import snapshot_download
    from opf import OPF

    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(1)
    snapshot_download(
        "openai/privacy-filter", revision=MODEL_REVISION,
        allow_patterns=["original/*"], local_dir=args.checkpoint,
    )
    checkpoint = args.checkpoint / "original"
    examples = [json.loads(line) for line in args.corpus.read_text().splitlines()]
    start = time.perf_counter()
    model = OPF(model=checkpoint, device="cpu", context_window_length=512)
    model.get_prediction_components()
    if args.grouped_cpu:
        from .optimized_opf import enable_grouped_cpu
        enable_grouped_cpu(model.get_runtime().model)
    load_seconds = time.perf_counter() - start
    model.redact("My name is Mira Vale.")

    durations = []
    predictions = []
    for repeat in range(args.repeats):
        current = []
        start = time.perf_counter()
        for example in examples:
            result = model.redact(example["text"])
            if result.warning or result.text != example["text"]:
                raise RuntimeError(f"Tokenizer changed source text for {example['id']}")
            spans = convert_spans(result.detected_spans)
            current.append({"id": example["id"], "text": example["text"], "spans": spans})
        durations.append(time.perf_counter() - start)
        if repeat and current != predictions:
            raise RuntimeError("Predictions changed between repeated runs")
        predictions = current
        print(f"Pass {repeat + 1}: {durations[-1]:.3f}s, {len(current)} examples", flush=True)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("".join(json.dumps(row, ensure_ascii=False) + "\n" for row in predictions))
    weights = checkpoint / "model.safetensors"
    metadata = {
        "model": "openai/privacy-filter", "revision": MODEL_REVISION,
        "implementation": "official opf, upstream f7f00ca7fb869683eb732c010299d901457f19c3",
        "device": "cpu", "dtype": "bfloat16", "context_window_tokens": 512,
        "grouped_cpu": args.grouped_cpu,
        "expert_dtype": "float32" if args.grouped_cpu else "bfloat16",
        "decode": "viterbi",
        "calibration": json.loads((checkpoint / "viterbi_calibration.json").read_text()),
        "threads": args.threads, "interop_threads": 1, "batch_size": 1,
        "warmup": "one separate synthetic sentence", "errors": 0,
        "load_seconds_excluding_download": load_seconds,
        "corpus_sha256": hashlib.sha256(args.corpus.read_bytes()).hexdigest(),
        "examples_per_pass": len(examples), "corpus_seconds": durations,
        "median_seconds": statistics.median(durations),
        "python": platform.python_version(), "platform": platform.platform(),
        "cpu_affinity": sorted(os.sched_getaffinity(0)), "load_average_end": os.getloadavg(),
        "max_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        "packages": {name: importlib.metadata.version(name) for name in
                     ["torch", "numpy", "opf", "tiktoken", "huggingface_hub", "safetensors"]},
        "weights_bytes": weights.stat().st_size,
        "checkpoint_bytes": sum(path.stat().st_size for path in checkpoint.iterdir() if path.is_file()),
    }
    args.output.with_suffix(".runtime.json").write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    main()
