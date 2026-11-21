"""Convert Nemotron-PII parquet to JSONL with Unicode code-point name spans."""

import ast
import json
import sys

import pyarrow.parquet as parquet


def name_ranges(row):
    text = row["text"]
    ranges = []
    for span in ast.literal_eval(row["spans"]):
        if span["label"] not in {"first_name", "last_name", "full_name"}:
            continue
        start, end = span["start"], span["end"]
        if not 0 <= start < end <= len(text):
            raise ValueError("Name offsets outside document")
        if text[start:end].casefold() != span["text"].casefold():
            raise ValueError("Name and character offsets disagree")
        ranges.append((start, end))

    merged = []
    for start, end in sorted(ranges):
        if merged and (start <= merged[-1][1] or text[merged[-1][1]:start].isspace()):
            merged[-1][1] = max(end, merged[-1][1])
        else:
            merged.append([start, end])
    return merged


def convert(path, output):
    documents = parquet.ParquetFile(path)
    for batch in documents.iter_batches(batch_size=512, columns=["text", "spans"]):
        for row in batch.to_pylist():
            record = {"text": row["text"], "names": name_ranges(row)}
            print(json.dumps(record, ensure_ascii=False), file=output)


if __name__ == "__main__":
    convert(sys.argv[1], sys.stdout)
