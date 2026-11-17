"""Numerical inference for the compiled SSN and phone circuits."""

import argparse
import json
import sys
from dataclasses import asdict
from functools import lru_cache
from pathlib import Path

import numpy as np

from detector import Span, redact

FORMAT_VERSION = 1
FEATURES = (
    "digit", "area_digit", "zero", "six", "nine", "hyphen", "separator",
    "plus", "one", "open", "close", "word",
)
UNICODE_WORD = 128
NONWORD = 129
CHUNK_SIZE = 4096
DEFAULT_WEIGHTS = Path(__file__).with_name("weights.npz")


def pack_trits(tensor: np.ndarray) -> np.ndarray:
    if not np.isin(tensor, [-1, 0, 1]).all():
        raise ValueError("Weights must be ternary")
    digits = (tensor.reshape(-1) % 3).astype(np.uint8)
    digits = np.pad(digits, (0, (-len(digits)) % 5))
    powers = np.array([1, 3, 9, 27, 81], dtype=np.uint8)
    return (digits.reshape(-1, 5) * powers).sum(axis=1).astype(np.uint8)


def unpack_trits(packed: np.ndarray, shape: tuple[int, ...]) -> np.ndarray:
    count = int(np.prod(shape))
    if packed.dtype != np.uint8 or packed.ndim != 1:
        raise ValueError("Packed weights must be a byte vector")
    if len(packed) != (count + 4) // 5 or np.any(packed >= 243):
        raise ValueError("Invalid packed ternary weights")
    powers = np.array([1, 3, 9, 27, 81], dtype=np.uint8)
    digits = ((packed[:, None] // powers) % 3).reshape(-1)[:count]
    signed = digits.astype(np.int16)
    return np.where(signed == 2, -1, signed).reshape(shape)


def character_code(char: str) -> int:
    ordinal = ord(char)
    if ordinal < 128:
        return ordinal
    if char.isalnum():
        return UNICODE_WORD
    return NONWORD


class Detector:
    def __init__(self, path: Path = DEFAULT_WEIGHTS):
        with np.load(path, allow_pickle=False) as archive:
            if archive["version"].tolist() != [FORMAT_VERSION]:
                raise ValueError("Unsupported ternary archive version")
            for name in ("embedding", "kernels", "readout"):
                shape = tuple(int(size) for size in archive[name + "_shape"])
                setattr(self, name, unpack_trits(archive[name], shape))
            self.thresholds = archive["thresholds"].astype(np.int16)
            self.lengths = archive["lengths"].astype(int)
            self.labels = archive["labels"].tolist()
        self.width = self.kernels.shape[1]
        self.flat_kernels = self.kernels.reshape(len(self.kernels), -1).T

    def detect(self, text: str) -> list[Span]:
        candidates = []
        for offset in range(0, len(text), CHUNK_SIZE):
            count = min(CHUNK_SIZE, len(text) - offset)
            section = text[max(0, offset - 1):offset + count + self.width - 2]
            codes = [character_code(char) for char in section]

            if offset == 0:
                codes.insert(0, NONWORD)
            codes.extend([NONWORD] * (count + self.width - 1 - len(codes)))

            features = self.embedding[codes]
            windows = np.lib.stride_tricks.sliding_window_view(
                features, self.width, axis=0
            ).transpose(0, 2, 1)

            scores = windows.reshape(count, -1) @ self.flat_kernels
            gates = (scores >= self.thresholds).astype(np.int16)
            matches = gates @ self.readout.T >= 1

            for position, head in np.argwhere(matches):
                start = offset + int(position)
                end = start + int(self.lengths[head])
                candidates.append(Span(start, end, self.labels[head]))

        selected = []
        cursors = dict.fromkeys(self.labels, 0)
        for span in sorted(candidates, key=lambda item: (item.start, -item.end, item.label)):
            if span.start < cursors[span.label]:
                continue
            selected.append(span)
            cursors[span.label] = span.end
        return sorted(selected)


@lru_cache(maxsize=1)
def default_detector() -> Detector:
    return Detector()


def detect(text: str) -> list[Span]:
    return default_detector().detect(text)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--weights", type=Path, default=DEFAULT_WEIGHTS)
    parser.add_argument("--corpus", type=Path, help="Emit prediction JSONL for a corpus")
    args = parser.parse_args()
    model = Detector(args.weights)
    if args.corpus:
        for line in args.corpus.read_text().splitlines():
            if not line.strip():
                continue
            row = json.loads(line)

            spans = model.detect(row["text"])
            print(json.dumps({"id": row["id"], "spans": [asdict(span) for span in spans]}))
        return

    text = sys.stdin.read()
    spans = model.detect(text)
    result = {"spans": [asdict(span) for span in spans], "redacted": redact(text, spans)}
    print(json.dumps(result, ensure_ascii=False))
