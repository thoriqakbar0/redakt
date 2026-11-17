"""Ordered compiled matching with ternary character and finite-state checksum gates."""

import argparse
from bisect import bisect_left
from dataclasses import asdict
from functools import lru_cache
import json
import math
from pathlib import Path
import struct
import sys

import numpy as np

from detector import Span, redact

FORMAT_VERSION = 2
CODE_COUNT = 132
DEFAULT_WEIGHTS = Path(__file__).resolve().parent.parent / "models" / "weights.npz"
MAX_MODEL_BYTES = 2_000_000


def pack_trits(tensor):
    if not np.isin(tensor, [-1, 0, 1]).all():
        raise ValueError("Weights must be ternary")
    digits = (tensor.reshape(-1) % 3).astype(np.uint8)
    digits = np.pad(digits, (0, (-len(digits)) % 5))
    powers = np.array([1, 3, 9, 27, 81], dtype=np.uint8)
    return (digits.reshape(-1, 5) * powers).sum(axis=1).astype(np.uint8)


def unpack_trits(packed, shape):
    if not shape or any(type(size) is not int or size < 1 for size in shape):
        raise ValueError("Invalid ternary dimensions")
    count = math.prod(shape)
    if packed.dtype != np.uint8 or packed.ndim != 1:
        raise ValueError("Packed weights must be a byte vector")
    if len(packed) != (count + 4) // 5 or np.any(packed >= 243):
        raise ValueError("Invalid packed ternary weights")
    powers = np.array([1, 3, 9, 27, 81], dtype=np.uint8)
    digits = ((packed[:, None] // powers) % 3).reshape(-1)
    if np.any(digits[count:]):
        raise ValueError("Invalid ternary padding")
    signed = digits[:count].astype(np.int16)
    return np.where(signed == 2, -1, signed).reshape(shape)


def packed_vector(values, count):
    if not isinstance(values, list) or any(
        type(value) is not int or not 0 <= value < 243 for value in values
    ):
        raise ValueError("Invalid packed vector")
    return unpack_trits(np.array(values, dtype=np.uint8), (count,))


def validate_ranges(ranges):
    if not isinstance(ranges, list):
        raise ValueError("Invalid Unicode ranges")
    previous = 127
    for pair in ranges:
        if not isinstance(pair, list) or len(pair) != 2:
            raise ValueError("Invalid Unicode range")
        first, last = pair
        if (
            type(first) is not int
            or type(last) is not int
            or not previous < first <= last <= 0x10FFFF
            or first <= 0xDFFF
            and last >= 0xD800
        ):
            raise ValueError("Invalid Unicode range")
        previous = last


def validate_program(instructions, mask_count):
    if not isinstance(instructions, list) or not 1 <= len(instructions) <= 10000:
        raise ValueError("Invalid instruction count")
    edges = []
    for position, instruction in enumerate(instructions):
        if (
            not isinstance(instruction, list)
            or len(instruction) != 4
            or any(
                type(value) is not int or not -(2**31) <= value < 2**31
                for value in instruction
            )
        ):
            raise ValueError("Invalid instruction")
        operation, first, second, detail = instruction
        if operation not in range(6):
            raise ValueError("Invalid instruction operation")

        targets = instruction_targets(instruction)
        if any(not 0 <= target < len(instructions) for target in targets):
            raise ValueError("Invalid instruction target")

        if operation == 1 and not 0 <= first < mask_count:
            raise ValueError("Invalid mask index")

        if operation == 3 and first not in (0, 1):
            raise ValueError("Invalid capture slot")

        if operation == 4 and (not 1 <= abs(detail) <= 1000 or first >= position):
            raise ValueError("Invalid assertion")

        if operation == 5 and first not in (0, 1):
            raise ValueError("Invalid anchor")

        if operation != 4 and detail != 0:
            raise ValueError("Invalid instruction detail")

        edges.append([] if operation in (0, 1) else targets)
    validate_epsilon_graph(instructions, edges)
    validate_assertions(instructions)


def validate_epsilon_graph(instructions, edges):
    colors = [0] * len(instructions)
    for root in range(len(instructions)):
        pending = [(root, False)]
        while pending:
            position, leaving = pending.pop()
            if leaving:
                colors[position] = 2
                continue
            if colors[position] == 1:
                raise ValueError("Cyclic zero-width program")
            if colors[position] == 2:
                continue

            colors[position] = 1
            pending.append((position, True))
            pending.extend((target, False) for target in edges[position])


def instruction_targets(instruction):
    operation, first, second, _ = instruction
    if operation == 0:
        return []
    return [first, second] if operation in (2, 4) else [second]


def assertion_depth(instructions, entry, limit, depths):
    pending = [entry]
    visited = set()
    depth = 1
    while pending:
        target = pending.pop()
        if target >= limit:
            raise ValueError("Recursive assertion program")
        if target in visited:
            continue
        visited.add(target)
        depth = max(depth, depths[target] + 1)
        pending.extend(instruction_targets(instructions[target]))
    return depth


def validate_assertions(instructions):
    depths = [0] * len(instructions)
    for position, instruction in enumerate(instructions):
        if instruction[0] != 4:
            continue
        depth = assertion_depth(instructions, instruction[1], position, depths)
        if depth > 64:
            raise ValueError("Assertion nesting exceeds model limit")
        depths[position] = depth


def validate_model(model):
    expected = {
        "unicode_version",
        "word_ranges",
        "space_ranges",
        "masks",
        "word_mask",
        "instructions",
        "heads",
        "luhn",
        "iban",
        "countries",
    }
    if not isinstance(model, dict) or set(model) != expected:
        raise ValueError("Invalid native model fields")
    version = model["unicode_version"]
    if (
        not isinstance(version, str)
        or not version
        or any(char not in "0123456789." for char in version)
    ):
        raise ValueError("Invalid Unicode version")

    validate_ranges(model["word_ranges"])
    validate_ranges(model["space_ranges"])
    if not isinstance(model["masks"], list) or not 1 <= len(model["masks"]) <= 512:
        raise ValueError("Invalid mask count")

    masks = np.array([packed_vector(mask, CODE_COUNT) for mask in model["masks"]])
    mask_count = len(masks)
    if type(model["word_mask"]) is not int or not 0 <= model["word_mask"] < mask_count:
        raise ValueError("Invalid word mask")

    validate_program(model["instructions"], mask_count)
    labels = ["email", "phone", "ssn", "card", "iban", "name", "address"]
    if not isinstance(model["heads"], list) or len(model["heads"]) != len(labels):
        raise ValueError("Invalid heads")

    for head, label in zip(model["heads"], labels):
        if (
            set(head) != {"label", "entry", "capture", "checksum", "required_masks"}
            or head["label"] != label
            or type(head["entry"]) is not int
            or not 0 <= head["entry"] < len(model["instructions"])
            or type(head["capture"]) is not bool
            or type(head["checksum"]) is not int
            or head["checksum"] not in (0, 1, 2)
        ):
            raise ValueError("Invalid head")

        required = head["required_masks"]
        if not isinstance(required, list) or any(
            type(mask) is not int or not 0 <= mask < mask_count for mask in required
        ):
            raise ValueError("Invalid required masks")

    validate_checksum(model["luhn"], 20, 10, mask_count)
    validate_checksum(model["iban"], 97, 36, mask_count)

    if (
        not isinstance(model["countries"], list)
        or not 1 <= len(model["countries"]) <= 100
    ):
        raise ValueError("Invalid countries")
    for country in model["countries"]:
        if (
            len(country) != 3
            or any(type(value) is not int for value in country)
            or not all(0 <= mask < mask_count for mask in country[:2])
            or not 4 <= country[2] <= 34
        ):
            raise ValueError("Invalid country")
    return masks


def validate_checksum(machine, states, alphabet, mask_count):
    if (
        not isinstance(machine, dict)
        or set(machine) != {"rows", "accept"}
        or not isinstance(machine["rows"], list)
        or len(machine["rows"]) != states
    ):
        raise ValueError("Invalid checksum dimensions")
    packed_vector(machine["accept"], states)

    for row in machine["rows"]:
        if not isinstance(row, list) or len(row) != alphabet:
            raise ValueError("Invalid checksum row")
        for transition in row:
            valid = (
                isinstance(transition, list)
                and len(transition) == 3
                and all(type(value) is int for value in transition)
            )
            if not valid:
                raise ValueError("Invalid checksum transition")
            mask, target, weight = transition
            if (
                not 0 <= mask < mask_count
                or not 0 <= target < states
                or weight not in (-1, 0, 1)
            ):
                raise ValueError("Invalid checksum transition")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("Duplicate native model field")
        result[key] = value
    return result


def load_native(native):
    if len(native) < 10 or len(native) > MAX_MODEL_BYTES:
        raise ValueError("Invalid native model size")
    magic, version, length = struct.unpack_from("<4sHI", native)
    if magic != b"PHYT" or version != FORMAT_VERSION or length != len(native) - 10:
        raise ValueError("Unsupported or truncated native model")
    try:
        model = json.loads(native[10:], object_pairs_hook=unique_object)
        masks = validate_model(model)
    except (
        TypeError,
        KeyError,
        IndexError,
        OverflowError,
        RecursionError,
        json.JSONDecodeError,
    ) as error:
        raise ValueError("Malformed native model") from error
    return model, masks


def starting_masks(instructions, entry):
    pending = [entry]
    visited = set()
    masks = set()
    while pending:
        position = pending.pop()
        if position in visited:
            continue
        visited.add(position)
        operation, first, second, _ = instructions[position]
        if operation == 1:
            masks.add(first)
        elif operation != 0:
            pending.append(second)
            if operation == 2:
                pending.append(first)
    return sorted(masks)


class Detector:
    def __init__(self, path=DEFAULT_WEIGHTS):
        with np.load(path, allow_pickle=False) as archive:
            if set(archive.files) != {"version", "native"} or archive[
                "version"
            ].tolist() != [FORMAT_VERSION]:
                raise ValueError("Unsupported ternary archive version")
            native = archive["native"]
            if native.dtype != np.uint8 or native.ndim != 1:
                raise ValueError("Invalid native archive payload")
            self.model, self.masks = load_native(native.tobytes())
        self.instructions = self.model["instructions"]
        self.starts = [
            starting_masks(self.instructions, head["entry"])
            for head in self.model["heads"]
        ]
        self.range_ends = {
            name: [last for _, last in self.model[name]]
            for name in ("word_ranges", "space_ranges")
        }
        self.acceptance = {
            name: packed_vector(self.model[name]["accept"], states)
            for name, states in (("luhn", 20), ("iban", 97))
        }

    def character_code(self, character):
        ordinal = ord(character)
        if ordinal < 128:
            return ordinal
        bits = 0
        for name, bit in (("word_ranges", 1), ("space_ranges", 2)):
            index = bisect_left(self.range_ends[name], ordinal)
            ranges = self.model[name]
            if index < len(ranges) and ranges[index][0] <= ordinal:
                bits |= bit
        return 128 + bits

    def gate(self, mask, code):
        return code is not None and self.masks[mask, code] >= 1

    def assertion_gate(self, codes, cursor, entry, detail):
        width = abs(detail) - 1
        origin = cursor - width
        matched = (
            origin >= 0
            and self.run(codes, entry, origin, cursor if width else None) is not None
        )
        polarity = 1 if detail > 0 else -1
        threshold = 1 if detail > 0 else 0
        return int(matched) * polarity >= threshold

    def anchor(self, kind, codes, cursor):
        current = codes[cursor] if cursor < len(codes) else None
        if kind == 1:
            at_end = int(cursor == len(codes))
            at_final_newline = int(cursor + 1 == len(codes) and current == 10)
            return at_end + at_final_newline >= 1

        previous = codes[cursor - 1] if cursor else None
        word = self.model["word_mask"]
        difference = int(self.gate(word, previous)) - int(self.gate(word, current))
        return max(difference, -difference) >= 1

    def run(self, codes, entry, start, required_end=None):
        pending = [(entry, start, (-1, -1))]
        while pending:
            position, cursor, capture = pending.pop()
            operation, first, second, detail = self.instructions[position]
            if operation == 0 and (required_end is None or cursor == required_end):
                return cursor, capture
            if operation == 0:
                continue

            if operation == 1 and (
                cursor >= len(codes) or not self.gate(first, codes[cursor])
            ):
                continue
            if operation == 1:
                cursor += 1
            elif operation == 2:
                pending.append((second, cursor, capture))
                pending.append((first, cursor, capture))
                continue
            elif operation == 3:
                capture = (cursor, capture[1]) if first == 0 else (capture[0], cursor)
            elif operation == 4 and not self.assertion_gate(
                codes, cursor, first, detail
            ):
                continue
            elif operation == 5 and not self.anchor(first, codes, cursor):
                continue
            pending.append((second, cursor, capture))
        return None

    def transition(self, name, state, code):
        for mask, target, weight in self.model[name]["rows"][state]:
            score = 1 + weight * int(self.gate(mask, code))
            if score >= 2:
                return target, mask
        return None

    def luhn_valid(self, compact):
        state = 0
        first_mask = None
        all_same = 1
        for code in reversed(compact):
            transition = self.transition("luhn", state, code)
            if transition is None:
                return False
            state, mask = transition
            if first_mask is None:
                first_mask = mask
            all_same = int(all_same + int(self.gate(first_mask, code)) >= 2)
        return self.acceptance["luhn"][state] - all_same >= 1

    def checksum_valid(self, kind, codes):
        if kind == 0:
            return True
        compact = [code for code in codes if code not in (32, 45)]
        if kind == 1:
            return self.luhn_valid(compact)
        if len(compact) < 4:
            return False
        country_valid = any(
            int(self.gate(first, compact[0]))
            + int(self.gate(second, compact[1]))
            + int(len(compact) == length)
            >= 3
            for first, second, length in self.model["countries"]
        )
        if not country_valid:
            return False
        state = 0
        for code in compact[4:] + compact[:4]:
            transition = self.transition("iban", state, code)
            if transition is None:
                return False
            state = transition[0]
        return self.acceptance["iban"][state] >= 1

    def detect_head(self, head, starts, codes):
        spans = []
        cursor = 0
        while cursor < len(codes):
            if not any(self.gate(mask, codes[cursor]) for mask in starts):
                cursor += 1
                continue
            matched = self.run(codes, head["entry"], cursor)
            if matched is None:
                cursor += 1
                continue
            end, capture = matched
            origin = cursor
            cursor = max(cursor + 1, end)
            if not self.checksum_valid(head["checksum"], codes[origin:end]):
                continue
            start, stop = capture if head["capture"] else (origin, end)
            if 0 <= start < stop <= len(codes):
                spans.append(Span(start, stop, head["label"]))
        return spans

    def detect(self, text):
        codes = [self.character_code(char) for char in text]
        present = set(codes)
        active = {
            mask: any(self.gate(mask, code) for code in present)
            for head in self.model["heads"]
            for mask in head["required_masks"]
        }
        spans = []
        for head, starts in zip(self.model["heads"], self.starts):
            if all(active[mask] for mask in head["required_masks"]):
                spans.extend(self.detect_head(head, starts, codes))
        return sorted(spans)


@lru_cache(maxsize=1)
def default_detector():
    return Detector()


def detect(text):
    return default_detector().detect(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--weights", type=Path, default=DEFAULT_WEIGHTS)
    parser.add_argument("--corpus", type=Path)
    arguments = parser.parse_args()
    model = Detector(arguments.weights)
    if arguments.corpus:
        for line in arguments.corpus.read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            spans = model.detect(row["text"])
            print(
                json.dumps({"id": row["id"], "spans": [asdict(span) for span in spans]})
            )
        return
    text = sys.stdin.read()
    spans = model.detect(text)
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
