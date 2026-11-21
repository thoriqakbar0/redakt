"""Pack fitted directional taggers into one offline Redakt model."""

import argparse
import json
import struct


def number(output, fmt, value):
    output.write(struct.pack("<" + fmt, value))


def emit_rows(output, rows):
    number(output, "I", len(rows))
    for slot, scale, entries in rows:
        output.write(struct.pack("<IhB", slot, scale, len(entries)))
        output.write(bytes(label | (128 if sign < 0 else 0) for label, sign in entries))


def pack(paths, output):
    models = []
    for path in paths:
        with open(path, encoding="utf-8") as source:
            models.append(json.load(source))
    labels, words, _ = models[0]
    if any(model[:2] != [labels, words] for model in models[1:]):
        raise ValueError("Taggers must share the training vocabulary and labels")
    output.write(b"RPII01")
    number(output, "B", len(labels))
    for label in labels:
        encoded = label.encode()
        number(output, "B", len(encoded))
        output.write(encoded)
    number(output, "I", len(words))
    for word, label in words:
        output.write(struct.pack("<IB", word, label))
    number(output, "B", len(models))
    for _, _, rows in models:
        emit_rows(output, rows)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output")
    parser.add_argument("forward")
    parser.add_argument("reverse", nargs="?")
    args = parser.parse_args()
    paths = [args.forward, args.reverse] if args.reverse else [args.forward]
    with open(args.output, "wb") as artifact:
        pack(paths, artifact)
