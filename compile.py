"""Compile fixed-width PII circuits into packed ternary weights without training."""

import argparse
import io
from pathlib import Path
import zipfile

import numpy as np

from ternary import (
    DEFAULT_WEIGHTS, FEATURES, FORMAT_VERSION, NONWORD, UNICODE_WORD, pack_trits,
)

DIGIT = "digit"
AREA = "area_digit"
SEPARATOR = "separator"


def compile_tensors() -> dict[str, np.ndarray]:
    channels = {name: index for index, name in enumerate(FEATURES)}
    embedding = np.zeros((NONWORD + 1, len(FEATURES)), dtype=np.int8)
    classes = {
        DIGIT: "0123456789", AREA: "23456789", "zero": "0", "six": "6",
        "nine": "9", "hyphen": "-", SEPARATOR: " .-", "plus": "+",
        "one": "1", "open": "(", "close": ")",
        "word": "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_",
    }
    for name, chars in classes.items():
        embedding[[ord(char) for char in chars], channels[name]] = 1
    embedding[UNICODE_WORD, channels["word"]] = 1

    ssn = [DIGIT] * 3 + ["hyphen"] + [DIGIT] * 2 + ["hyphen"] + [DIGIT] * 4
    local = [AREA, DIGIT, DIGIT, SEPARATOR, AREA, DIGIT, DIGIT, SEPARATOR]
    local += [DIGIT] * 4
    area_codes = [local, ["open"] + local[:3] + ["close"] + local[3:]]
    prefixes = [[], ["plus", "one"], ["plus", "one", SEPARATOR]]
    patterns = [("ssn", ssn)]
    patterns.extend(("phone", prefix + body) for body in area_codes for prefix in prefixes)

    width = max(len(body) for _, body in patterns) + 2
    kernels = []
    thresholds = []
    for label, body in patterns:
        kernel = np.zeros((width, len(FEATURES)), dtype=np.int8)
        for position, feature in enumerate(body, start=1):
            kernel[position, channels[feature]] = 1
        kernel[[0, len(body) + 1], channels["word"]] = -1
        if label == "ssn":
            kernel[[0, len(body) + 1], channels["hyphen"]] = -1
        kernels.append(kernel)
        thresholds.append(len(body))

    exclusions = [
        [(1, "zero"), (2, "zero"), (3, "zero")],
        [(1, "six"), (2, "six"), (3, "six")],
        [(1, "nine")], [(5, "zero"), (6, "zero")],
        [(8, "zero"), (9, "zero"), (10, "zero"), (11, "zero")],
    ]
    for terms in exclusions:
        kernel = np.zeros((width, len(FEATURES)), dtype=np.int8)
        for position, feature in terms:
            kernel[position, channels[feature]] = 1
        kernels.append(kernel)
        thresholds.append(len(terms))
    readout = np.zeros((len(patterns), len(kernels)), dtype=np.int8)
    for head in range(len(patterns)):
        readout[head, head] = 1
    readout[0, len(patterns):] = -1
    return {
        "embedding": embedding, "kernels": np.array(kernels), "readout": readout,
        "thresholds": np.array(thresholds, dtype=np.int16),
        "lengths": np.array([len(body) for _, body in patterns], dtype=np.int16),
        "labels": np.array([label for label, _ in patterns]),
    }


def compile_weights(path: Path = DEFAULT_WEIGHTS) -> None:
    tensors = compile_tensors()
    archive = {"version": np.array([FORMAT_VERSION], dtype=np.int16)}
    for name, tensor in tensors.items():
        if tensor.ndim > 1:
            archive[name] = pack_trits(tensor)
            archive[name + "_shape"] = np.array(tensor.shape, dtype=np.int16)
        else:
            archive[name] = tensor
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for name, tensor in sorted(archive.items()):
            buffer = io.BytesIO()
            np.save(buffer, tensor, allow_pickle=False)
            member = zipfile.ZipInfo(name + ".npy", date_time=(1980, 1, 1, 0, 0, 0))
            member.compress_type = zipfile.ZIP_DEFLATED
            output.writestr(member, buffer.getvalue())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_WEIGHTS)
    compile_weights(parser.parse_args().output)
