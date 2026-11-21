"""Score Redakt's public JSONL output against labeled Unicode code-point spans."""

import argparse
import json
import subprocess
from collections import Counter
from pathlib import Path


def metric_row(gold, predicted, correct, covered):
    return {
        "gold": gold,
        "predicted": predicted,
        "correct": correct,
        "fully_covered": covered,
        "precision": correct / predicted if predicted else None,
        "recall": correct / gold if gold else None,
        "coverage": covered / gold if gold else None,
    }


def redacted_ranges(predictions):
    merged = []
    for start, end, _ in sorted(predictions):
        if merged and start <= merged[-1][1]:
            merged[-1][1] = max(end, merged[-1][1])
        else:
            merged.append([start, end])
    return merged


class Scores:
    def __init__(self):
        self.documents = 0
        self.gold = Counter()
        self.predicted = Counter()
        self.correct = Counter()
        self.covered = Counter()

    def add(self, gold, predicted):
        self.documents += 1
        self.gold.update(label for _, _, label in gold)
        self.predicted.update(label for _, _, label in predicted)
        self.correct.update(label for _, _, label in gold & predicted)
        ranges = redacted_ranges(predicted)
        for start, end, label in gold:
            if any(left <= start and end <= right for left, right in ranges):
                self.covered[label] += 1

    def result(self):
        counters = self.gold, self.predicted, self.correct, self.covered
        per_class = {
            label: metric_row(*(counter[label] for counter in counters))
            for label in sorted(self.gold.keys() | self.predicted.keys())
        }
        return {
            "documents": self.documents,
            "micro": metric_row(*(counter.total() for counter in counters)),
            "per_class": per_class,
        }


def evaluate_document(process, record, index, scores):
    request = {"id": index, "text": record["text"]}
    process.stdin.write(json.dumps(request, ensure_ascii=False) + "\n")
    process.stdin.flush()
    line = process.stdout.readline()
    if not line:
        raise RuntimeError(f"CLI stopped before response {index}")
    response = json.loads(line)
    if response["id"] != index:
        raise ValueError(f"Unexpected JSONL response ID for record {index}")

    predicted = {(span["start"], span["end"], span["label"]) for span in response["spans"]}
    gold = {tuple(span) for span in record["spans"]}
    if any(not 0 <= start < end <= len(record["text"]) for start, end, _ in gold | predicted):
        raise ValueError(f"Span outside source text for record {index}")
    scores.add(gold, predicted)
    return response["redacted"]


def smoke_cases():
    fixtures = [
        ("I met Nora Chen yesterday.", [("Nora Chen", "name")]),
        ("Email me at alex@example.org or call 415-555-0198.",
         [("alex@example.org", "email"), ("415-555-0198", "phone")]),
        ("🦊 Ship to 42 Maple Street. DOB: 1990-04-12.",
         [("42 Maple Street", "address"), ("1990-04-12", "date")]),
        ("Please send the revised report tomorrow.", []),
    ]
    for text, values in fixtures:
        spans = []
        for value, label in values:
            start = text.index(value)
            spans.append([start, start + len(value), label])
        yield {"text": text, "spans": spans}


def run(binary, source):
    scores = Scores()
    smoke = []
    command = [str(binary.resolve()), "--jsonl"]
    with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                          text=True, encoding="utf-8") as process:
        try:
            for line in source:
                evaluate_document(process, json.loads(line), scores.documents, scores)
            for record in smoke_cases():
                single = Scores()
                output = evaluate_document(process, record, scores.documents + len(smoke), single)
                smoke.append({"input": record["text"], "output": output, **single.result()["micro"]})
        finally:
            process.stdin.close()
        if process.stdout.read() or process.wait() != 0:
            raise RuntimeError("CLI produced extra output or exited unsuccessfully")

    if scores.documents == 0:
        raise ValueError("Empty evaluation set")
    return {**scores.result(), "smoke": smoke}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path, help="Normalized JSONL file with text and spans")
    parser.add_argument("--binary", type=Path, default=Path("target/release/redakt"))
    args = parser.parse_args()
    with args.dataset.open(encoding="utf-8") as source:
        print(json.dumps(run(args.binary, source), indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
