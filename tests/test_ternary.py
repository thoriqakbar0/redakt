import copy
import json
from pathlib import Path
import random
import subprocess
import sys
import tempfile
import unittest
import unicodedata

from detector import Span, detect as rules_detect

try:
    import numpy as np
except ImportError:
    np = None

if np is not None:
    from experiments.compile_ternary import (
        Compiler,
        compile_tensors,
        compile_weights,
        native_weights,
    )
    from experiments.ternary import (
        DEFAULT_WEIGHTS,
        Detector,
        detect,
        load_native,
        pack_trits,
        unpack_trits,
    )


def generated_samples():
    samples = [
        json.loads(line)["text"]
        for line in Path("data/synthetic.jsonl").read_text().splitlines()
    ]
    generator = random.Random(17)
    boundaries = ["", " ", "é", "_", "-", "🦊", "1", ".", "\u0345", "²", "𐐀", "\u0301"]
    names = ["Nora Chen", "Ana-Maria Smith", "John O'Neil", "Ada Bea Cee Dee Eve"]
    addresses = [
        "123 Maple Street",
        "1 A St",
        "987654 Long Rd, New York, NY 10001",
        "1234567 Long Road",
        "42 A Street Extra Rd",
        "12 Road St, Bad, ZZ 99999",
    ]
    accounts = [
        "DE89370400440532013000",
        "GB82 WEST 1234 5698 7654 32",
        "FR1420041010050500013M02606",
        "ES9121000418450200051332",
        "NL91ABNA0417164300",
        "IT60X0542811101000000123456",
        "DE90370400440532013000",
    ]
    for _ in range(200):
        area = generator.choice(["000", "666", "999", "123", "212", "415"])
        serial = generator.choice(["0000", "0123", "6789"])
        ssn = area + "-" + generator.choice(["00", "01", "45"]) + "-" + serial
        prefix = generator.choice(["", "+1", "+1 ", "+1.", "+1-"])
        phone_area = generator.choice([area, "(" + area + ")"])
        separator = generator.choice([" ", ".", "-"])
        phone = prefix + phone_area + separator + "555" + separator + serial
        local = generator.choice(["a", "a..b+tag", "a" * 80, "!#$%&'*+/=?^_`{|}~-"])
        domain = generator.choice(
            ["example.test", "a-b.c", "a--b.test", "bad-.test", "a.x.y"]
        )
        email = local + "@" + domain
        card = generator.choice(
            [
                "4111 1111 1111 1111",
                "5555-5555-5555-4444",
                "378282246310005",
                "4111111111111112",
                "0000000000000",
            ]
        )
        name = generator.choice(["Name", "Patient", "Customer", "My name is"])
        name += generator.choice([": ", "\t", "\u2003", ":\n"]) + generator.choice(
            names
        )
        address = generator.choice(["Address", "Ship to", "Lives at"])
        address += (
            generator.choice([": ", "\u0085", "\u202f"])
            + generator.choice(addresses)
            + "."
        )
        values = [ssn, phone, email, card, generator.choice(accounts), name, address]
        generator.shuffle(values)
        before, after = generator.choices(boundaries, k=2)
        samples.append(before + "; ".join(values) + after)
    samples.extend(
        [
            "a" * 5000 + "@" + "b" * 2000 + ".test",
            "a" * 20000,
            "Name: " + "A" + "a" * 5000 + " Chen.",
            "Address: 12 " + "Long " * 800 + "Road, Town, NY 12345.\n",
            "Address: 12 Main Street\n",
            "Address: 12 Main Street\nmore",
            "Name: Ada Bea Name: Cee Dee",
            "Address: 12 Main Street, Town, NY 12345;",
            "4111111111111112 4111111111111111",
            "4111111111111111-4111111111111111",
            "DE89370400440532013000 DE89370400440532013000",
            "Address: 1 A St. Address: 2 B Road.",
            "Name: Ada Bea Cee Dee Eve.",
            "a@example.test-a@example.test",
            "a@example.testé; a@example.test\u0345",
            "\n\u2003Name:\u2003Nora Chen.",
        ]
    )
    samples.extend(
        [
            "4123456789011",
            "41234567890120",
            "412345678901233",
            "4123456789012349",
            "41234567890123458",
            "412345678901234561",
            "4123456789012345677",
            "412345678901",
            "41234567890123456789",
        ]
    )
    return samples


@unittest.skipIf(np is None, "Install the ternary extra to test compiled inference")
class TernaryTests(unittest.TestCase):
    def test_deterministic_models_and_packing(self):
        model = compile_tensors()
        native = native_weights(model)
        loaded, _ = load_native(native)
        self.assertEqual(loaded, model)
        for count in (1, 5, 132):
            vector = np.resize(np.array([-1, 0, 1], dtype=np.int8), count)
            np.testing.assert_array_equal(
                unpack_trits(pack_trits(vector), (count,)), vector
            )
        with tempfile.TemporaryDirectory() as directory:
            first, second = (
                Path(directory) / name for name in ("first.npz", "second.npz")
            )
            compile_weights(first)
            compile_weights(second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(
                Detector(first).detect("212-555-0123"), [Span(0, 12, "phone")]
            )
            if unicodedata.unidata_version == Detector().model["unicode_version"]:
                with np.load(first) as compiled, np.load(DEFAULT_WEIGHTS) as committed:
                    for name in compiled.files:
                        np.testing.assert_array_equal(compiled[name], committed[name])
                self.assertEqual(native, Path("models/weights.bin").read_bytes())
        with self.assertRaises(ValueError):
            pack_trits(np.array([2]))
        for vector in ([255], [242]):
            with self.assertRaises(ValueError):
                unpack_trits(np.array(vector, dtype=np.uint8), (1,))

    def test_independent_known_answers(self):
        cases = [
            ("🦊 123-45-6789!", [Span(2, 13, "ssn")]),
            ("+1 (212) 555.0123", [Span(0, 17, "phone")]),
            ("a+b@example.test", [Span(0, 16, "email")]),
            ("4111 1111 1111 1111", [Span(0, 19, "card")]),
            ("GB82 WEST 1234 5698 7654 32", [Span(0, 27, "iban")]),
            ("Name: Nora Chen.", [Span(6, 15, "name")]),
            ("Address: 123 Maple Street.", [Span(9, 25, "address")]),
            ("Address: 123 Maple Street\n", [Span(9, 25, "address")]),
            ("", []),
            ("000-12-1234;666-12-1234;900-12-1234;123-00-1234;123-45-0000", []),
            ("4111111111111112; 0000000000000; DE90370400440532013000", []),
            ("é123-45-6789;123-45-6789_;-123-45-6789;123-45-6789-", []),
            ("4111111111111112 4111111111111111", [Span(17, 33, "card")]),
            ("4111111111111111 1", []),
        ]
        for text, expected in cases:
            with self.subTest(text=text):
                self.assertEqual(detect(text), expected)

    def test_original_parity(self):
        for text in generated_samples():
            with self.subTest(text=text[:100]):
                self.assertEqual(detect(text), rules_detect(text))

    def test_unsupported_syntax(self):
        try:
            from re import _parser as parser
        except ImportError:
            import sre_parse as parser
        for pattern in (r"(a)\1", r".", r"(?i:a)", r"(?i)a", r"(?:a?)*"):
            with self.subTest(pattern=pattern), self.assertRaises(ValueError):
                compiler = Compiler()
                compiler.sequence(parser.parse(pattern), compiler.emit(0))

    def test_malformed_models(self):
        model = compile_tensors()
        mutations = []
        for field, value in [
            ("masks", [[255]]),
            ("word_mask", 9999),
            ("word_ranges", [[128, 127]]),
            ("instructions", [[2, 0, 0, 0]]),
            ("instructions", [[0, 0, 0, 0], [1, 0, 3, 0], [4, 0, 0, 1], [4, 1, 0, 1]]),
            (
                "instructions",
                [[0, 0, 0, 0]] + [[4, index, 0, 1] for index in range(65)],
            ),
        ]:
            broken = copy.deepcopy(model)
            broken[field] = value
            mutations.append(broken)
        broken = copy.deepcopy(model)
        broken["instructions"][-1] = [4, len(broken["instructions"]) - 1, 0, 1]
        mutations.append(broken)
        broken = copy.deepcopy(model)
        broken["luhn"]["rows"][0][0][1] = 20
        mutations.append(broken)
        for broken in mutations:
            with self.assertRaises(ValueError):
                load_native(native_weights(broken))
        native = native_weights(model)
        for malformed in (
            b"",
            native[:20],
            native + b"extra",
            b"BAD!" + native[4:],
            native[:4] + b"\x01\x00" + native[6:],
        ):
            with self.assertRaises(ValueError):
                load_native(malformed)

    def test_cli(self):
        result = subprocess.run(
            [sys.executable, "-m", "experiments.ternary"],
            input="123-45-6789",
            text=True,
            capture_output=True,
            check=True,
        )
        self.assertEqual(
            json.loads(result.stdout),
            {
                "spans": [{"start": 0, "end": 11, "label": "ssn"}],
                "redacted": "█" * 11,
            },
        )
        command = [
            sys.executable,
            "-m",
            "experiments.ternary",
            "--corpus",
            "data/synthetic.jsonl",
        ]
        result = subprocess.run(command, text=True, capture_output=True, check=True)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(len(rows), 64)
        self.assertIn("id", rows[0])
        self.assertIn("spans", rows[0])


if __name__ == "__main__":
    unittest.main()
