import dataclasses
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import tempfile
import unittest
import unicodedata

try:
    import numpy as np
except ImportError:
    np = None

if np is not None:
    from detector import redact
    from experiments.ternary import Detector, unpack_trits
    from experiments.compile_ternary import compile_tensors, compile_weights, native_weights

BINARY = Path("target/release/privacy-hygiene" + (".exe" if os.name == "nt" else "")).resolve()
MODEL = Path("models/weights.bin")


@unittest.skipUnless(BINARY.exists() and np is not None,
                     "Build cargo build --release --locked and install NumPy for native parity")
class RustTests(unittest.TestCase):
    def invoke(self, text, *arguments):
        return subprocess.run([str(BINARY), *map(str, arguments)], input=text,
                              capture_output=True, check=False)

    def prediction(self, text, *arguments):
        result = self.invoke(text.encode(), *arguments)
        self.assertEqual(result.returncode, 0, result.stderr)
        return json.loads(result.stdout)

    def test_hand_examples(self):
        samples = ["123-45-6789", "212-555-0123", "+1212-555-0123", "+1 212-555-0123",
                   "(212) 555-0123", "+1(212) 555-0123", "+1 (212) 555-0123"]
        for index, text in enumerate(samples):
            with self.subTest(text=text):
                result = self.prediction("🦊 " + text + "\r\n")
                label = "ssn" if index == 0 else "phone"
                self.assertEqual(result, {
                    "spans": [{"start": 2, "end": 2 + len(text),
                               "label": label}],
                    "redacted": "🦊 " + "█" * len(text) + "\r\n",
                })
        rejected = ["", "000-12-1234", "666-12-1234", "900-12-1234", "123-00-1234",
                     "123-45-0000", "é123-45-6789", "123-45-6789_", "-123-45-6789",
                     "123-45-6789-", "112-555-0123", "212-155-0123", "Ⅷ212-555-0123"]
        for text in rejected:
            self.assertEqual(self.prediction(text)["spans"], [])
        expected = [{"start": 1, "end": 12, "label": "ssn"}]
        self.assertEqual(self.prediction("\u0345123-45-6789\r")["spans"], expected)
        self.assertEqual(self.prediction("123-45-6789;212-555-0123")['spans'], [
            {"start": 0, "end": 11, "label": "ssn"},
            {"start": 12, "end": 24, "label": "phone"},
        ])

    def test_python_parity(self):
        lines = Path("data/synthetic.jsonl").read_text().splitlines()
        samples = [json.loads(line)["text"] for line in lines]
        generator = random.Random(17)
        for _ in range(250):
            area = generator.choice(["000", "666", "999", "123", "212", "415"])
            serial = generator.choice(["0000", "0123", "6789"])
            ssn = area + "-" + generator.choice(["00", "01", "45"]) + "-" + serial
            prefix = generator.choice(["", "+1", "+1 ", "+1.", "+1-"])
            phone_area = generator.choice([area, "(" + area + ")"])
            separator = generator.choice([" ", ".", "-"])
            phone = prefix + phone_area + separator + "555" + separator + serial
            before, after = generator.choices(["", " ", "é", "_", "-", "🦊", "1", "."], k=2)
            samples.append(before + ssn + ";" + phone + after)
        samples.extend("🦊" * start + "+1 (212) 555-0123" for start in range(4078, 4098))
        samples.extend(["\u0345212-555-0123", "²212-555-0123", "212-555-0123\u0301",
                        "123-45-6789\r212-555-0123\r\n", "𐐀212-555-0123"])
        model = Detector()
        for text in samples:
            with self.subTest(text=text[:80]):
                spans = [dataclasses.asdict(span) for span in model.detect(text)]
                self.assertEqual(self.prediction(text), {
                    "spans": spans, "redacted": redact(text, model.detect(text)),
                })

    def test_custom_weights(self):
        tensors = compile_tensors()
        native = native_weights(tensors)
        version_length = native[18]
        version = native[19:19 + version_length].decode()
        range_offset = 19 + version_length
        range_count = struct.unpack_from("<I", native, range_offset)[0]
        offset = range_offset + 4 + 8 * range_count

        packed = np.frombuffer(native[offset:offset + 876], dtype=np.uint8)
        unpacked = unpack_trits(packed, (4380,))
        tensor_offset = 0
        for name in ("embedding", "kernels", "readout"):
            tensor = tensors[name]
            actual = unpacked[tensor_offset:tensor_offset + tensor.size].reshape(tensor.shape)
            np.testing.assert_array_equal(actual, tensor)
            tensor_offset += tensor.size

        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "weights.npz"
            artifact = Path(directory) / "weights.bin"
            compile_weights(archive, artifact)
            self.assertEqual(artifact.read_bytes(), native)

            bundled = MODEL.read_bytes()
            bundled_version = bundled[19:19 + bundled[18]].decode()
            if version == bundled_version == unicodedata.unidata_version:
                self.assertEqual(native, bundled)

            expected = [{"start": 0, "end": 11, "label": "ssn"}]
            actual = self.prediction("123-45-6789", "--weights", artifact)["spans"]
            self.assertEqual(actual, expected)

            tensors["thresholds"][0] = 1000
            artifact.write_bytes(native_weights(tensors))
            self.assertEqual(self.prediction("123-45-6789", "--weights", artifact)["spans"], [])

            tensors = compile_tensors()
            tensors["readout"][:] = 0
            artifact.write_bytes(native_weights(tensors))
            self.assertEqual(self.prediction("212-555-0123", "--weights", artifact)["spans"], [])

    def test_private_errors(self):
        sensitive = b"123-45-6789"

        with tempfile.TemporaryDirectory() as directory:
            artifact = Path(directory) / "bad.bin"
            valid = MODEL.read_bytes()
            mutations = [b"", valid[:20], valid + b"extra", b"BAD!" + valid[4:]]
            for offset, replacement in [(4, b"\xff"), (6, b"\xff"),
                                        (len(valid) - 1, b"\xff"),
                                        (len(valid) - 9, b"\xff\xff")]:
                mutations.append(valid[:offset] + replacement + valid[offset + len(replacement):])
            version_end = 19 + valid[18]
            packed_start = version_end + 4 + 8 * struct.unpack_from("<I", valid, version_end)[0]
            mutations.append(valid[:packed_start] + b"\xff" + valid[packed_start + 1:])

            for malformed in mutations:
                artifact.write_bytes(malformed)
                result = self.invoke(sensitive, "--weights", artifact)

                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertNotIn(sensitive, result.stderr)

        result = self.invoke(b"private\xff")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"private", result.stderr)
        standalone = subprocess.run([str(BINARY)], input=b"123-45-6789",
                                    capture_output=True, env={**os.environ, "PATH": ""})
        self.assertEqual(standalone.returncode, 0, standalone.stderr)
        self.assertEqual(json.loads(standalone.stdout)["redacted"], "█" * 11)
        for argument in ["--help", "--version"]:
            self.assertEqual(self.invoke(b"", argument).returncode, 0)
        self.assertNotEqual(self.invoke(b"", "--weights").returncode, 0)
