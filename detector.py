"""Explicit PII rules. No learned parameters or external runtime dependencies."""

import json
import re
import sys
from dataclasses import asdict, dataclass


@dataclass(frozen=True, order=True)
class Span:
    start: int
    end: int
    label: str


DIGIT = re.compile(r"[0-9]")
EMAIL = re.compile(
    r"(?<![\w.+-])[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@"
    r"[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?"
    r"(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)+(?![\w-])"
)
PHONE = re.compile(
    r"(?<!\w)(?:\+1[ .-]?)?(?:\([2-9][0-9]{2}\)|[2-9][0-9]{2})"
    r"[ .-][2-9][0-9]{2}[ .-][0-9]{4}(?!\w)"
)
SSN = re.compile(
    r"(?<![\w-])(?!000|666|9[0-9]{2})[0-9]{3}-"
    r"(?!00)[0-9]{2}-(?!0000)[0-9]{4}(?![\w-])"
)
CARD = re.compile(r"(?<![\w-])(?:[0-9][ -]?){12,18}[0-9](?![\w-])")
IBAN = re.compile(r"(?<!\w)[A-Z]{2}[0-9]{2}(?:[ ]?[A-Z0-9]){11,30}(?!\w)")
NAME_CUES = ("Name", "Patient", "Customer", "My name is")
ADDRESS_CUES = ("Address", "Ship to", "Lives at")
NAME = re.compile(
    r"\b(?:" + "|".join(map(re.escape, NAME_CUES)) + r"):?\s+"
    r"(?P<value>[A-Z][a-z]+(?:[ '-][A-Z][a-z]+){1,3})(?![a-z])"
)
ADDRESS = re.compile(
    r"\b(?:" + "|".join(map(re.escape, ADDRESS_CUES)) + r"):?\s+"
    r"(?P<value>[0-9]{1,6} [A-Za-z0-9 .'-]+? "
    r"(?:Street|St|Road|Rd|Avenue|Ave|Lane|Ln|Drive|Dr)"
    r"(?:, [A-Za-z .'-]+, [A-Z]{2} [0-9]{5})?)(?=[.,;\n]|$)"
)
IBAN_LENGTHS = {"GB": 22, "DE": 22, "FR": 27, "ES": 24, "NL": 18, "IT": 27}


def luhn(value: str) -> bool:
    digits = [int(char) for char in value if char.isdigit()]
    if not 13 <= len(digits) <= 19 or len(set(digits)) == 1:
        return False
    total = 0
    for offset, digit in enumerate(reversed(digits)):
        doubled = digit * 2 if offset % 2 else digit
        total += doubled - 9 if doubled > 9 else doubled
    return total % 10 == 0


def iban_valid(value: str) -> bool:
    compact = value.replace(" ", "")
    if len(compact) != IBAN_LENGTHS.get(compact[:2]):
        return False
    rearranged = compact[4:] + compact[:4]
    numeric = "".join(
        str(ord(char) - 55) if char.isalpha() else char for char in rearranged
    )
    return int(numeric) % 97 == 1


def detect(text: str) -> list[Span]:
    spans = []
    has_digits = DIGIT.search(text) is not None
    for label, pattern, active in [
        ("email", EMAIL, "@" in text),
        ("phone", PHONE, has_digits),
        ("ssn", SSN, has_digits and "-" in text),
    ]:
        if not active:
            continue
        spans.extend(
            Span(match.start(), match.end(), label) for match in pattern.finditer(text)
        )
    if has_digits:
        for label, pattern, valid in [("card", CARD, luhn), ("iban", IBAN, iban_valid)]:
            spans.extend(
                Span(match.start(), match.end(), label)
                for match in pattern.finditer(text)
                if valid(match.group())
            )
    for label, pattern, cues in [
        ("name", NAME, NAME_CUES),
        ("address", ADDRESS, ADDRESS_CUES),
    ]:
        if not any(cue in text for cue in cues):
            continue
        spans.extend(
            Span(*match.span("value"), label) for match in pattern.finditer(text)
        )
    return sorted(spans, key=lambda span: (span.start, span.end, span.label))


def redact(text: str, spans: list[Span]) -> str:
    chunks = []
    cursor = 0
    for span in sorted(spans):
        if not 0 <= span.start < span.end <= len(text):
            raise ValueError("Span outside text")
        if span.end <= cursor:
            continue
        chunks.append(text[cursor:max(cursor, span.start)])
        chunks.append("█" * (span.end - max(cursor, span.start)))
        cursor = span.end
    chunks.append(text[cursor:])
    return "".join(chunks)


def main() -> None:
    text = sys.stdin.read()
    spans = detect(text)
    print(
        json.dumps(
            {
                "spans": [asdict(span) for span in spans],
                "redacted": redact(text, spans),
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()
