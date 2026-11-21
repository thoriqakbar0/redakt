"""Convert Nemotron-PII annotations into Redakt code-point spans."""

import ast
import json
import sys

import pyarrow.parquet as parquet


LABELS = {
    "first_name": "name",
    "last_name": "name",
    "full_name": "name",
    "date_of_birth": "date",
    "street_address": "address",
    "phone_number": "phone",
    "credit_debit_card": "card",
    "api_key": "secret",
    "password": "secret",
}


def normalize(row):
    text = row["text"]
    spans = []
    for item in ast.literal_eval(row["spans"]):
        start, end = item["start"], item["end"]
        if not 0 <= start < end <= len(text):
            raise ValueError("Invalid gold span offsets")
        label = LABELS.get(item["label"], item["label"])
        spans.append((start, end, label))

    result = []
    for start, end, label in sorted(set(spans)):
        if result and label == result[-1][2] == "name":
            previous_end = result[-1][1]
            if start <= previous_end or text[previous_end:start].isspace():
                result[-1][1] = max(end, previous_end)
                continue
        result.append([start, end, label])
    return {"text": text, "spans": result}


def convert(path):
    source = parquet.ParquetFile(path)
    for batch in source.iter_batches(batch_size=512, columns=["text", "spans"]):
        for row in batch.to_pylist():
            print(json.dumps(normalize(row), ensure_ascii=False))


if __name__ == "__main__":
    convert(sys.argv[1])
