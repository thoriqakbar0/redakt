import copy
import dataclasses
import json
import os
from pathlib import Path
import queue
import threading
import subprocess
import tempfile
import unittest

try:
    import numpy as np
except ImportError:
    np = None

if np is not None:
    from detector import detect as rules_detect, redact
    from experiments.compile_ternary import (
        compile_tensors,
        compile_weights,
        native_weights,
    )
    from experiments.ternary import Detector
    from test_ternary import generated_samples

BINARY = Path(
    "target/release/privacy-hygiene" + (".exe" if os.name == "nt" else "")
).resolve()
MODEL = Path("models/weights.bin")


@unittest.skipUnless(
    BINARY.exists() and np is not None,
    "Build cargo build --release --locked and install NumPy for native parity",
)
class RustTests(unittest.TestCase):
    def invoke(self, text, *arguments):
        return subprocess.run(
            [str(BINARY), *map(str, arguments)],
            input=text,
            capture_output=True,
            check=False,
            timeout=30,
        )

    def test_all_categories_against_original(self):
        samples = generated_samples()
        request = "".join(
            json.dumps({"id": index, "text": text}) + "\n"
            for index, text in enumerate(samples)
        )
        process = self.invoke(request.encode(), "--jsonl")
        self.assertEqual(process.returncode, 0, process.stderr)
        rows = [json.loads(line) for line in process.stdout.splitlines()]
        self.assertEqual(len(rows), len(samples))
        for index, (text, row) in enumerate(zip(samples, rows)):
            with self.subTest(index=index, text=text[:100]):
                spans = rules_detect(text)
                expected = {
                    "id": index,
                    "spans": [dataclasses.asdict(span) for span in spans],
                    "redacted": redact(text, spans),
                }
                self.assertEqual(row, expected)

    def test_custom_weights(self):
        tensors = compile_tensors()
        native = native_weights(tensors)
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "weights.npz"
            artifact = Path(directory) / "weights.bin"
            compile_weights(archive, artifact)
            self.assertEqual(artifact.read_bytes(), native)

            text = "a@example.test; Name: Nora Chen."
            process = self.invoke(text.encode(), "--weights", artifact)
            self.assertEqual(process.returncode, 0, process.stderr)
            expected = [
                dataclasses.asdict(span) for span in Detector(archive).detect(text)
            ]
            self.assertEqual(json.loads(process.stdout)["spans"], expected)

            alternate = copy.deepcopy(tensors)
            masks = Detector(archive).masks
            mask = next(
                index for index, vector in enumerate(masks) if vector[ord("a")] >= 1
            )
            alternate["instructions"].append([0, 0, 0, 0])
            accept = len(alternate["instructions"]) - 1
            alternate["instructions"].append([1, mask, accept, 0])
            alternate["heads"][0].update(
                entry=accept + 1, checksum=2, required_masks=[]
            )
            custom = native_weights(alternate)
            artifact.write_bytes(custom)
            np.savez(
                archive,
                version=np.array([2]),
                native=np.frombuffer(custom, dtype=np.uint8),
            )
            self.assertEqual(Detector(archive).detect("a"), [])
            process = self.invoke(b"a", "--weights", artifact)
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertEqual(json.loads(process.stdout)["spans"], [])

            tensors["masks"] = [[0] * len(mask) for mask in tensors["masks"]]
            artifact.write_bytes(native_weights(tensors))
            process = self.invoke(text.encode(), "--weights", artifact)
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertEqual(json.loads(process.stdout)["spans"], [])

    def test_private_model_errors(self):
        sensitive = b"private 123-45-6789"
        valid = MODEL.read_bytes()
        mutations = [
            b"",
            valid[:20],
            valid + b"extra",
            b"BAD!" + valid[4:],
            valid[:4] + b"\x01\x00" + valid[6:],
        ]
        model = json.loads(valid[10:])
        for field, value in [
            ("masks", [[255]]),
            ("word_mask", 9999),
            ("word_ranges", [[128, 127]]),
            ("instructions", [[2, 0, 0, 0]]),
            ("instructions", [[1, 0, 9999, 0]]),
            ("instructions", [[0, 0, 0, 0], [4, 1, 0, 1]]),
            ("instructions", [[0, 0, 0, 0], [1, 0, 3, 0], [4, 0, 0, 1], [4, 1, 0, 1]]),
            (
                "instructions",
                [[0, 0, 0, 0]] + [[4, index, 0, 1] for index in range(65)],
            ),
        ]:
            broken = copy.deepcopy(model)
            broken[field] = value
            mutations.append(native_weights(broken))
        broken = copy.deepcopy(model)
        broken["iban"]["rows"][0][0][1] = 97
        mutations.append(native_weights(broken))
        broken = copy.deepcopy(model)
        broken["luhn"]["rows"][0][0][2] = 2
        mutations.append(native_weights(broken))
        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "bad.bin"
            for malformed in mutations:
                artifact.write_bytes(malformed)
                result = self.invoke(sensitive, "--weights", artifact)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertNotIn(sensitive, result.stderr)
                self.assertNotIn(b"panicked", result.stderr)

    def test_jsonl_flush_and_record_contract(self):
        with subprocess.Popen(
            [str(BINARY), "--jsonl"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ) as process:
            for identifier, text in [
                ("first", "Name: Nora Chen."),
                (2, "a@example.test"),
            ]:
                process.stdin.write(
                    (json.dumps({"id": identifier, "text": text}) + "\n").encode()
                )
                process.stdin.flush()
                responses = queue.Queue()
                reader = threading.Thread(
                    target=lambda: responses.put(process.stdout.readline()), daemon=True
                )
                reader.start()
                response = json.loads(responses.get(timeout=5))
                self.assertEqual(response["id"], identifier)
                spans = rules_detect(text)
                self.assertEqual(
                    response["spans"], [dataclasses.asdict(span) for span in spans]
                )
                self.assertEqual(response["redacted"], redact(text, spans))
            process.stdin.close()
            self.assertEqual(process.wait(timeout=5), 0)
        self.assertEqual(self.invoke(b"\n  \n", "--jsonl").stdout, b"")
        for record in [
            b'{"id":1,"text":42}',
            b'{"id":1}',
            b'{"text":"private"}',
            b"private",
            b"[]",
            b'{"id":1,"text":"private","extra":true}',
            b"private\xff",
        ]:
            process = self.invoke(record, "--jsonl")
            self.assertNotEqual(process.returncode, 0)
            self.assertEqual(process.stdout, b"")
            self.assertNotIn(b"private", process.stderr)

    def test_default_stdin_and_standalone_cli(self):
        process = subprocess.run(
            [str(BINARY)],
            input=b"123-45-6789",
            capture_output=True,
            env={**os.environ, "PATH": ""},
            timeout=5,
        )
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(
            json.loads(process.stdout),
            {"spans": [{"start": 0, "end": 11, "label": "ssn"}], "redacted": "█" * 11},
        )
        result = self.invoke(b"private\xff")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"private", result.stderr)
        for argument in ["--help", "--version"]:
            self.assertEqual(self.invoke(b"", argument).returncode, 0)
        for arguments in [("--weights",), ("--jsonl", "--jsonl"), ("--unknown",)]:
            self.assertNotEqual(self.invoke(b"", *arguments).returncode, 0)


if __name__ == "__main__":
    unittest.main()
