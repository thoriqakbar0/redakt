"""Compile the original ordered PII patterns into ternary-gated matcher programs."""

import argparse
import io
import json
import struct
import unicodedata
import zipfile
from pathlib import Path

import numpy as np

try:
    from re import _parser as sre_parse
except ImportError:
    import sre_parse

import detector
from .ternary import CODE_COUNT, DEFAULT_WEIGHTS, FORMAT_VERSION, pack_trits


class Compiler:
    def __init__(self):
        self.masks = []
        self.instructions = []

    def emit(self, operation, first=0, second=0, detail=0):
        self.instructions.append([operation, first, second, detail])
        return len(self.instructions) - 1

    def mask(self, values):
        packed = pack_trits(np.array(values, dtype=np.int8)).tolist()
        if packed not in self.masks:
            self.masks.append(packed)
        return self.masks.index(packed)

    def characters(self, terms):
        values = [0] * CODE_COUNT
        negate = bool(terms and terms[0][0] == sre_parse.NEGATE)
        for operation, argument in terms:
            self.character_term(values, operation, argument)

        if negate:
            values = [1 - value for value in values]
        return self.mask(values)

    def character_term(self, values, operation, argument):
        if operation == sre_parse.NEGATE:
            return
        if operation == sre_parse.LITERAL:
            if argument >= 128:
                raise ValueError("Only ASCII literals are supported")
            values[argument] = 1
            return
        if operation == sre_parse.RANGE:
            first, last = argument
            if last >= 128:
                raise ValueError("Only ASCII literal ranges are supported")
            values[first : last + 1] = [1] * (last - first + 1)
            return
        if operation == sre_parse.CATEGORY:
            self.category(values, argument)
            return
        raise ValueError(f"Unsupported character operation {operation}")

    @staticmethod
    def category(values, category):
        predicates = {
            sre_parse.CATEGORY_WORD: lambda char: char.isalnum() or char == "_",
            sre_parse.CATEGORY_SPACE: str.isspace,
        }
        if category not in predicates:
            raise ValueError(f"Unsupported category {category}")
        predicate = predicates[category]
        values[:128] = [int(predicate(chr(ordinal))) for ordinal in range(128)]
        for bits in range(4):
            values[128 + bits] = int(
                bool(bits & (1 if category == sre_parse.CATEGORY_WORD else 2))
            )

    def required_masks(self, terms):
        required = set()
        for operation, argument in terms:
            if operation == sre_parse.LITERAL:
                required.add(self.characters([(operation, argument)]))
            elif operation == sre_parse.IN:
                required.add(self.characters(argument))
            elif operation == sre_parse.SUBPATTERN:
                required.update(self.required_masks(argument[3]))
            elif operation == sre_parse.BRANCH:
                branches = [self.required_masks(body) for body in argument[1]]
                required.update(set.intersection(*branches))
            elif (
                operation in (sre_parse.MAX_REPEAT, sre_parse.MIN_REPEAT)
                and argument[0] > 0
            ):
                required.update(self.required_masks(argument[2]))
        return required

    def sequence(self, terms, continuation):
        if terms.state.flags != sre_parse.SRE_FLAG_UNICODE:
            raise ValueError("Unsupported pattern flags")
        entry = continuation
        for operation, argument in reversed(list(terms)):
            entry = self.term(operation, argument, entry)
        return entry

    def term(self, operation, argument, continuation):
        if operation in (sre_parse.LITERAL, sre_parse.IN):
            terms = (
                [(operation, argument)] if operation == sre_parse.LITERAL else argument
            )
            return self.emit(1, self.characters(terms), continuation)
        if operation == sre_parse.SUBPATTERN:
            group, add_flags, delete_flags, body = argument
            if add_flags or delete_flags or group not in (None, 1):
                raise ValueError("Unsupported capture or flags")
            if group is None:
                return self.sequence(body, continuation)
            end = self.emit(3, 1, continuation)
            return self.emit(3, 0, self.sequence(body, end))
        if operation == sre_parse.BRANCH:
            flags, alternatives = argument
            if flags:
                raise ValueError("Unsupported branch flags")
            entries = [self.sequence(body, continuation) for body in alternatives]
            entry = entries[-1]
            for alternative in reversed(entries[:-1]):
                entry = self.emit(2, alternative, entry)
            return entry
        if operation in (sre_parse.MAX_REPEAT, sre_parse.MIN_REPEAT):
            return self.repeat(
                argument, continuation, operation == sre_parse.MAX_REPEAT
            )
        if operation in (sre_parse.ASSERT, sre_parse.ASSERT_NOT):
            return self.assertion(argument, continuation, operation == sre_parse.ASSERT)
        if operation == sre_parse.AT:
            kinds = {sre_parse.AT_BOUNDARY: 0, sre_parse.AT_END: 1}
            if argument not in kinds:
                raise ValueError(f"Unsupported anchor {argument}")
            return self.emit(5, kinds[argument], continuation)
        raise ValueError(f"Unsupported pattern operation {operation}")

    def repeat(self, argument, continuation, greedy):
        minimum, maximum, body = argument
        entry = continuation
        if maximum == sre_parse.MAXREPEAT:
            if body.getwidth()[0] == 0:
                raise ValueError("Empty unbounded repeat")
            split = self.emit(2)
            repeated = self.sequence(body, split)
            choices = (repeated, entry) if greedy else (entry, repeated)
            self.instructions[split] = [2, *choices, 0]
            entry = split
        else:
            for _ in range(maximum - minimum):
                repeated = self.sequence(body, entry)
                choices = (repeated, entry) if greedy else (entry, repeated)
                entry = self.emit(2, *choices)

        for _ in range(minimum):
            entry = self.sequence(body, entry)
        return entry

    def assertion(self, argument, continuation, positive):
        direction, body = argument
        width = 0
        if direction < 0:
            minimum, maximum = body.getwidth()
            if minimum != maximum:
                raise ValueError("Variable lookbehind")
            width = minimum

        assertion = self.sequence(body, self.emit(0))
        polarity = 1 if positive else -1
        return self.emit(4, assertion, continuation, polarity * (width + 1))


def unicode_ranges(predicate):
    ranges = []
    start = None
    for ordinal in range(128, 0x110001):
        matches = ordinal < 0x110000 and predicate(chr(ordinal))
        if matches and start is None:
            start = ordinal
        if not matches and start is not None:
            ranges.append([start, ordinal - 1])
            start = None
    return ranges


def checksum(compiler, modulus, alphabet, transition, acceptance):
    masks = [compiler.characters([(sre_parse.LITERAL, ord(char))]) for char in alphabet]
    rows = []
    for state in range(modulus):
        row = [
            [mask, transition(state, symbol), 1] for symbol, mask in enumerate(masks)
        ]
        rows.append(row)
    return {
        "rows": rows,
        "accept": pack_trits(np.array(acceptance, dtype=np.int8)).tolist(),
    }


def luhn_transition(state, digit):
    contribution = digit
    if state >= 10:
        contribution = digit * 2 - 9 if digit > 4 else digit * 2
    remainder = (state % 10 + contribution) % 10
    return remainder + (0 if state >= 10 else 10)


def iban_transition(state, symbol):
    width = 10 if symbol < 10 else 100
    return (state * width + symbol) % 97


def compile_tensors():
    compiler = Compiler()
    word = compiler.characters([(sre_parse.CATEGORY, sre_parse.CATEGORY_WORD)])
    heads = []
    for label in ("email", "phone", "ssn", "card", "iban", "name", "address"):
        pattern = getattr(detector, label.upper())
        parsed = sre_parse.parse(pattern.pattern, pattern.flags)
        entry = compiler.sequence(parsed, compiler.emit(0))
        heads.append(
            {
                "label": label,
                "entry": entry,
                "capture": label in ("name", "address"),
                "required_masks": sorted(compiler.required_masks(parsed)),
                "checksum": {"card": 1, "iban": 2}.get(label, 0),
            }
        )
    luhn_accept = [int(state % 10 == 0) for state in range(20)]
    luhn = checksum(compiler, 20, "0123456789", luhn_transition, luhn_accept)
    iban_accept = [int(state == 1) for state in range(97)]
    iban = checksum(
        compiler,
        97,
        "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        iban_transition,
        iban_accept,
    )

    countries = []
    for country, length in sorted(detector.IBAN_LENGTHS.items()):
        first = compiler.characters([(sre_parse.LITERAL, ord(country[0]))])
        second = compiler.characters([(sre_parse.LITERAL, ord(country[1]))])
        countries.append([first, second, length])

    return {
        "unicode_version": unicodedata.unidata_version,
        "word_ranges": unicode_ranges(str.isalnum),
        "space_ranges": unicode_ranges(str.isspace),
        "masks": compiler.masks,
        "word_mask": word,
        "instructions": compiler.instructions,
        "heads": heads,
        "luhn": luhn,
        "iban": iban,
        "countries": countries,
    }


def native_weights(tensors):
    payload = json.dumps(tensors, separators=(",", ":"), sort_keys=True).encode("ascii")
    return b"PHYT" + struct.pack("<HI", FORMAT_VERSION, len(payload)) + payload


def compile_weights(path=DEFAULT_WEIGHTS, native_output=None):
    native = native_weights(compile_tensors())
    archive = {
        "version": np.array([FORMAT_VERSION], dtype=np.int16),
        "native": np.frombuffer(native, dtype=np.uint8),
    }
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for name, tensor in sorted(archive.items()):
            buffer = io.BytesIO()
            np.save(buffer, tensor, allow_pickle=False)
            member = zipfile.ZipInfo(name + ".npy", date_time=(1980, 1, 1, 0, 0, 0))
            member.compress_type = zipfile.ZIP_DEFLATED
            output.writestr(member, buffer.getvalue())
    if native_output is not None:
        native_output.write_bytes(native)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_WEIGHTS)
    parser.add_argument("--native-output", type=Path)
    arguments = parser.parse_args()
    compile_weights(arguments.output, arguments.native_output)
