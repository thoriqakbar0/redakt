import json
import subprocess
import sys
import unittest
from pathlib import Path

from detector import Span, detect, redact
from experiments.evaluate import metrics, parse_spans


class BehaviorTests(unittest.TestCase):
    def test_detection(self):
        cases = [
            ("Card 4111 1111 1111 1111.", "4111 1111 1111 1111", "card"),
            ("IBAN DE89370400440532013000.", "DE89370400440532013000", "iban"),
            ("Name: Nora Chen.", "Nora Chen", "name"),
            ("Address: 123 Maple Street.", "123 Maple Street", "address"),
            ("🦊 (a+b@example.test),", "a+b@example.test", "email"),
        ]
        for text, value, label in cases:
            with self.subTest(text=text):
                start = text.index(value)
                self.assertIn(Span(start, start + len(value), label), detect(text))
        invalid = "4111 1111 1111 1112; DE90370400440532013000; 000-12-1234"
        self.assertEqual(detect(invalid), [])

    def test_redaction(self):
        text = "🦊 abc def!"
        spans = [
            Span(6, 9, "email"), Span(2, 5, "name"),
            Span(4, 9, "address"), Span(2, 5, "name"),
        ]
        self.assertEqual(redact(text, spans), "🦊 ███████!")
        self.assertEqual(redact(text, []), text)
        for invalid in [Span(-1, 5, "name"), Span(4, 3, "name"), Span(5, 20, "name")]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                redact(text, spans + [invalid])

    def test_scoring(self):
        rows = [
            {
                "id": "one",
                "text": "Alice xyz",
                "spans": [{"start": 0, "end": 5, "label": "name"}],
            }
        ]
        partial = metrics(rows, {"one": [Span(1, 6, "name")]})
        self.assertEqual(partial["exact_matches"], 0)
        self.assertEqual(partial["missed_sensitive_characters"], 1)
        self.assertEqual(partial["unnecessary_redacted_characters"], 1)
        wrong_type = metrics(rows, {"one": [Span(0, 5, "email")]})
        self.assertEqual(wrong_type["exact_matches"], 1)
        self.assertEqual(wrong_type["typed_exact_matches"], 0)
        self.assertEqual(wrong_type["missed_sensitive_characters"], 0)

    def test_external_spans(self):
        invalid = [
            {"start": True, "end": 3, "label": "name"},
            {"start": 0, "end": 8, "label": "name"},
            {"start": 0, "end": 3, "label": ""},
        ]
        for span in invalid:
            with self.subTest(span=span), self.assertRaises(ValueError):
                parse_spans({"id": "one", "spans": [span]}, "Alice")
        with self.assertRaises(ValueError):
            parse_spans({"id": "one", "text": "Other", "spans": []}, "Alice")

    def test_fixture_offsets_and_cli(self):
        for line in Path("data/synthetic.jsonl").read_text().splitlines():
            row = json.loads(line)
            parse_spans(row, row["text"])
        process = subprocess.run(
            [sys.executable, "detector.py"],
            input="Hi a@example.test!",
            text=True,
            capture_output=True,
            check=True,
        )
        result = json.loads(process.stdout)
        self.assertEqual(result["spans"], [{"start": 3, "end": 17, "label": "email"}])
        self.assertEqual(result["redacted"], "Hi " + "█" * 14 + "!")


if __name__ == "__main__":
    unittest.main()
