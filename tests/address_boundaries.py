"""Run with python3 tests/address_boundaries.py PATH_TO_CLI to assess address redaction."""
import itertools
import json
import subprocess
import sys
from pathlib import Path

weights = Path(__file__).resolve().parents[1] / "models/weights.bin"
cues = ["Address: ", "Ship to ", "Lives at "]
streets = ["1 Oak Lane", "48 Cedar Road", "123 Maple Street"]
endings = ["", ".", "!", "?", ",", ";", ":", ")", '"', " ", "\t", "\n", "\r", "\r\n",
           ". Next sentence.", "\nNext line.", "。"]
requests = []
expected = []
for cue, street, ending in itertools.product(cues, streets, endings):
    prefix = "🦊 " + cue
    record_id = str(len(requests))
    requests.append({"id": record_id, "text": prefix + street + ending})
    span = {"label": "address", "start": len(prefix), "end": len(prefix + street)}
    expected.append({"id": record_id, "spans": [span],
                     "redacted": prefix + "█" * len(street) + ending})

payload = "".join(json.dumps(row) + "\n" for row in requests)
results = []
failed = False
for loader, flags in [("embedded", []), ("file", ["--weights", weights])]:
    run = subprocess.run([sys.argv[1], "--jsonl", *flags], input=payload,
                         capture_output=True, text=True, check=True)
    actual = [json.loads(line) for line in run.stdout.splitlines()]
    if len(actual) != len(expected) or run.stderr:
        sys.exit(f"{loader}: unexpected output count or stderr")
    mismatches = [index for index, row in enumerate(actual) if row != expected[index]]
    print(f"{loader}: {len(mismatches)} of {len(expected)} predictions mismatched")
    for index in mismatches:
        print(json.dumps({"input": requests[index], "expected": expected[index],
                          "actual": actual[index]}))
    failed |= bool(mismatches)
    results.append(actual)

print("Loader parity:", results[0] == results[1])
sys.exit(int(failed or results[0] != results[1]))
