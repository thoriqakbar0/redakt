import json
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import unittest

from detector import Span, detect as rules_detect

try:
    import numpy as np
except ImportError:
    np = None

if np is not None:
    from ternary.compile import compile_tensors, compile_weights
    from ternary import CHUNK_SIZE, DEFAULT_WEIGHTS, Detector, detect, pack_trits, unpack_trits


@unittest.skipIf(np is None, "Install the ternary extra to test compiled inference")
class TernaryTests(unittest.TestCase):
    def test_compiled_archive(self):
        tensors = compile_tensors()
        for name, tensor in tensors.items():
            if tensor.ndim < 2:
                continue
            with self.subTest(name=name):
                self.assertTrue(np.isin(tensor, [-1, 0, 1]).all())
                np.testing.assert_array_equal(unpack_trits(pack_trits(tensor), tensor.shape), tensor)
        with tempfile.TemporaryDirectory() as directory:
            first, second = (Path(directory) / name for name in ("first.npz", "second.npz"))
            compile_weights(first)
            compile_weights(second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(first.read_bytes(), DEFAULT_WEIGHTS.read_bytes())
            self.assertEqual(Detector(first).detect("212-555-0123"), [Span(0, 12, "phone")])
        with self.assertRaises(ValueError):
            pack_trits(np.array([2]))
        with self.assertRaises(ValueError):
            unpack_trits(np.array([255], dtype=np.uint8), (1,))

    def test_hand_labelled_examples(self):
        cases = [
            ("🦊 123-45-6789!", [Span(2, 13, "ssn")]),
            ("+1 (212) 555.0123", [Span(0, 17, "phone")]),
            ("+1212-555-0123", [Span(0, 14, "phone")]),
            ("212 555 0123,415.555.0199", [Span(0, 12, "phone"), Span(13, 25, "phone")]),
            ("", []), ("Call a@example.test", []),
            ("000-12-1234;666-12-1234;900-12-1234;123-00-1234;123-45-0000", []),
            ("é123-45-6789;123-45-6789_;-123-45-6789;123-45-6789-", []),
            ("é212-555-0123;212-555-0123_;112-555-0123;212-155-0123", []),
        ]
        for text, expected in cases:
            with self.subTest(text=text):
                self.assertEqual(detect(text), expected)

    def test_corpus_and_generated_parity(self):
        corpus = Path("data/synthetic.jsonl").read_text().splitlines()
        samples = [json.loads(line)["text"] for line in corpus]
        generator = random.Random(17)
        boundaries = ["", " ", "é", "_", "-", "🦊", "1", "."]
        for _ in range(250):
            area = generator.choice(["000", "666", "999", "123", "212", "415"])
            group = generator.choice(["00", "01", "45"])
            serial = generator.choice(["0000", "0123", "6789"])
            ssn = area + "-" + group + "-" + serial

            prefix = generator.choice(["", "+1", "+1 ", "+1.", "+1-"])
            phone_area = generator.choice([area, "(" + area + ")"])
            separator = generator.choice([" ", ".", "-"])
            phone = prefix + phone_area + separator + "555" + separator + serial

            before, after = generator.choices(boundaries, k=2)
            samples.append(before + ssn + ";" + phone + after)
        for text in samples:
            with self.subTest(text=text):
                expected = [span for span in rules_detect(text) if span.label in {"phone", "ssn"}]
                self.assertEqual(detect(text), expected)

    def test_chunk_edges_and_cli(self):
        for start in range(CHUNK_SIZE - 18, CHUNK_SIZE + 2):
            text = "🦊" * start + "+1 (212) 555-0123"
            self.assertEqual(detect(text), [Span(start, start + 17, "phone")])
        result = subprocess.run([sys.executable, "-m", "ternary"], input="123-45-6789",
                                text=True, capture_output=True, check=True)
        self.assertEqual(json.loads(result.stdout), {
            "spans": [{"start": 0, "end": 11, "label": "ssn"}], "redacted": "█" * 11,
        })
        result = subprocess.run([sys.executable, "-m", "ternary", "--corpus", "data/synthetic.jsonl"],
                                text=True, capture_output=True, check=True)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(rows), 64)
        self.assertIn("id", rows[0])
        self.assertIn("spans", rows[0])


if __name__ == "__main__":
    unittest.main()
