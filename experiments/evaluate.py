"""Score Unicode character spans on a fixed synthetic fixture."""

import argparse
import hashlib
import json
import os
import platform
import statistics
import sys
import time
from dataclasses import asdict
from pathlib import Path

from detector import Span, detect, redact


LABEL_MAP = {
    "ssn": "account_number",
    "card": "account_number",
    "iban": "account_number",
}


def read_rows(path: Path) -> list[dict]:
    rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
    identifiers = [row["id"] for row in rows]
    if len(set(identifiers)) != len(identifiers):
        raise ValueError("Duplicate document ids")
    return rows


def parse_spans(row: dict, text: str) -> list[Span]:
    if "text" in row and row["text"] != text:
        raise ValueError(f"Text mismatch for {row['id']}")
    for raw in row["spans"]:
        if type(raw["start"]) is not int or type(raw["end"]) is not int:
            raise ValueError("Span offsets must be integers")
        if not isinstance(raw["label"], str) or not raw["label"]:
            raise ValueError("Span labels must be nonempty strings")
    spans = [Span(**span) for span in row["spans"]]
    for span in spans:
        if not 0 <= span.start < span.end <= len(text):
            raise ValueError(f"Invalid span for {row['id']}: {span}")
    if len(set(spans)) != len(spans):
        raise ValueError(f"Duplicate spans for {row['id']}")
    return spans


def coverage(spans: list[Span]) -> set[int]:
    return {offset for span in spans for offset in range(span.start, span.end)}


def metrics(documents: list[dict], predictions: dict[str, list[Span]]) -> dict:
    counts = dict(
        documents=len(documents),
        gold_spans=0,
        predicted_spans=0,
        exact_matches=0,
        typed_exact_matches=0,
        typed_gold_spans=0,
        typed_predicted_spans=0,
        sensitive_characters=0,
        missed_sensitive_characters=0,
        unnecessary_redacted_characters=0,
        nonsensitive_characters=0,
        exact_documents=0,
    )
    for row in documents:
        gold = parse_spans(row, row["text"])
        predicted = predictions[row["id"]]
        gold_boundaries = {(span.start, span.end) for span in gold}
        predicted_boundaries = {(span.start, span.end) for span in predicted}
        gold_typed = {
            (span.start, span.end, LABEL_MAP.get(span.label, span.label))
            for span in gold
        }
        predicted_typed = {
            (span.start, span.end, LABEL_MAP.get(span.label, span.label))
            for span in predicted
        }
        gold_chars = coverage(gold)
        predicted_chars = coverage(predicted)
        counts["gold_spans"] += len(gold_boundaries)
        counts["predicted_spans"] += len(predicted_boundaries)
        counts["exact_matches"] += len(gold_boundaries & predicted_boundaries)
        counts["typed_exact_matches"] += len(gold_typed & predicted_typed)
        counts["typed_gold_spans"] += len(gold_typed)
        counts["typed_predicted_spans"] += len(predicted_typed)
        counts["sensitive_characters"] += len(gold_chars)
        counts["nonsensitive_characters"] += len(row["text"]) - len(gold_chars)
        counts["missed_sensitive_characters"] += len(gold_chars - predicted_chars)
        counts["unnecessary_redacted_characters"] += len(predicted_chars - gold_chars)
        counts["exact_documents"] += gold_boundaries == predicted_boundaries
    for prefix, matched in [
        ("span", "exact_matches"),
        ("typed_span", "typed_exact_matches"),
    ]:
        denominator = "typed_" if prefix == "typed_span" else ""
        counts[prefix + "_precision"] = ratio(
            counts[matched], counts[denominator + "predicted_spans"]
        )
        counts[prefix + "_recall"] = ratio(
            counts[matched], counts[denominator + "gold_spans"]
        )
    counts["missed_sensitive_character_rate"] = ratio(
        counts["missed_sensitive_characters"], counts["sensitive_characters"]
    )
    counts["unnecessary_redaction_rate"] = ratio(
        counts["unnecessary_redacted_characters"], counts["nonsensitive_characters"]
    )
    return counts


def label_metrics(rows: list[dict], predictions: dict[str, list[Span]]) -> dict:
    gold = set()
    predicted = set()
    for row in rows:
        gold.update(
            (row["id"], span.start, span.end, LABEL_MAP.get(span.label, span.label))
            for span in parse_spans(row, row["text"])
        )
        predicted.update(
            (row["id"], span.start, span.end, LABEL_MAP.get(span.label, span.label))
            for span in predictions[row["id"]]
        )
    labels = sorted({span[-1] for span in gold | predicted})
    scores = {}
    for label in labels:
        gold_label = {span for span in gold if span[-1] == label}
        predicted_label = {span for span in predicted if span[-1] == label}
        matched = len(gold_label & predicted_label)
        scores[label] = {
            "gold_spans": len(gold_label),
            "predicted_spans": len(predicted_label),
            "exact_matches": matched,
            "precision": ratio(matched, len(predicted_label)),
            "recall": ratio(matched, len(gold_label)),
        }
    return scores


def ratio(numerator: int, denominator: int) -> float | None:
    return numerator / denominator if denominator else None


def benchmark(rows: list[dict], repeats: int) -> dict:
    durations = []
    checksum = 0
    for row in rows:
        detect(row["text"])
    for _ in range(repeats):
        started = time.perf_counter()
        for row in rows:
            spans = detect(row["text"])
            checksum += len(redact(row["text"], spans))
        durations.append(time.perf_counter() - started)
    return {
        "seconds": durations,
        "median_seconds": statistics.median(durations),
        "min_seconds": min(durations),
        "max_seconds": max(durations),
        "repeats": repeats,
        "characters_processed": checksum,
        "scope": "warm detect plus character-mask redaction; no I/O or startup",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data", type=Path, default=Path("data/synthetic.jsonl"))
    parser.add_argument("--predictions", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repeat", type=int, default=5)
    args = parser.parse_args()
    if args.repeat < 5:
        parser.error("--repeat must be at least 5")
    rows = read_rows(args.data)
    if args.predictions:
        external = {row["id"]: row for row in read_rows(args.predictions)}
        if set(external) != {row["id"] for row in rows}:
            raise ValueError("Prediction ids must exactly match fixture ids")
        predictions = {
            row["id"]: parse_spans(external[row["id"]], row["text"]) for row in rows
        }
    else:
        predictions = {row["id"]: detect(row["text"]) for row in rows}
    groups = ["all", "structured", "contextual", "negative"]
    scores = {
        group: metrics(
            [row for row in rows if group == "all" or row["group"] == group],
            predictions,
        )
        for group in groups
    }
    if any(row["group"] not in groups[1:] for row in rows):
        raise ValueError("Unknown fixture group")
    details = []
    for row in rows:
        spans = predictions[row["id"]]
        details.append(
            {
                **row,
                "predicted": [asdict(span) for span in spans],
                "redacted": redact(row["text"], spans),
            }
        )
    result = {
        "dataset_sha256": hashlib.sha256(args.data.read_bytes()).hexdigest(),
        "detector": str(args.predictions) if args.predictions else "rules",
        "metrics": scores,
        "per_label": label_metrics(rows, predictions),
        "documents": details,
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "cpu_count": os.cpu_count(),
            "load_average": os.getloadavg(),
        },
        "artifact": {
            "detector_source_bytes": Path("detector.py").stat().st_size,
            "learned_weight_bytes": 0,
            "python_executable_bytes": Path(sys.executable).stat().st_size,
            "runtime_note": "Executable excludes shared libraries and stdlib.",
        },
    }
    if not args.predictions:
        result["runtime"] = benchmark(rows, args.repeat)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(scores, indent=2))


if __name__ == "__main__":
    main()
