"""Combine the compiled rules, sequence tagger, and cue table for native inference."""

import argparse
import struct
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("rules", "tagger", "cues", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()

    rules = args.rules.read_bytes()
    tagger = args.tagger.read_bytes()
    cues = args.cues.read_bytes()
    header = struct.pack("<4sII", b"RDM1", len(rules), len(tagger))
    args.output.write_bytes(header + rules + tagger + cues)


if __name__ == "__main__":
    main()
