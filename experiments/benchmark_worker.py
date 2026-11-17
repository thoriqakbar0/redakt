"""Persistent JSONL adapter for the Python detectors."""

import argparse
from dataclasses import asdict
import json
from pathlib import Path
import sys

from detector import Span, detect, redact


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("detector", choices=["rules", "privacy-filter"])
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--grouped-cpu", action="store_true")
    args = parser.parse_args()

    if args.detector == "privacy-filter":
        if not args.checkpoint or not args.checkpoint.is_dir():
            parser.error("privacy-filter requires an existing local checkpoint directory")
        import torch
        from opf import OPF
        from experiments.run_baseline import convert_spans

        torch.set_num_threads(args.threads)
        torch.set_num_interop_threads(1)
        model = OPF(model=args.checkpoint, device="cpu", context_window_length=512)
        model.get_prediction_components()
        if args.grouped_cpu:
            from experiments.optimized_opf import enable_grouped_cpu
            enable_grouped_cpu(model.get_runtime().model)

    for line in sys.stdin:
        request = json.loads(line)
        source = request["text"]
        if args.detector == "rules":
            spans = detect(source)
        else:
            result = model.redact(source)
            if result.warning or result.text != source:
                message = f"Tokenizer changed or truncated input {request['id']}"
                raise ValueError(f"{message}: {result.warning}")
            spans = [Span(**span) for span in convert_spans(result.detected_spans)]
        response = dict(id=request["id"], spans=[asdict(span) for span in spans],
                        redacted=redact(source, spans))
        print(json.dumps(response, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
