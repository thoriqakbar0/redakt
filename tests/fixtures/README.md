# Independent rules oracle

`rules-parity.jsonl` freezes 289 synthetic cases with their original rules-detector spans and redacted text. It includes the 64-document corpus, seeded mixed-category cases, Unicode boundaries and whitespace, long variable-length matches, greedy and lazy captures, invalid checksums, and card length boundaries.

Expected values were produced by the independent `detector.py`, not the native implementation under test. The generator and rules source are preserved in [reproduction commit 6ba3e585](https://github.com/thoriqakbar0/privacy-hygiene/tree/6ba3e5850af6b2b13a22b81f27bd9df619df30d5). The rules source SHA-256 is `f29be211597b0801a232ee6e1a7c0a8bf724695c4bc157631ced410c11bc6160`. Generation used Python 3.10 and Unicode 13.0.0, matching the checked-in native model's classification tables.

Rust tests send only each record's `id` and `text` through the real persistent CLI and compare its complete response against the frozen `spans` and `redacted`. No Python runtime is required. Independent hand-labelled examples remain in `tests/cli.rs` so an oracle error is not the only correctness check.

## Hand-labelled detection quality

`accuracy.jsonl` is separate from the rules oracle. Its expected private values were annotated by hand, including benign documents and unsupported formats. See [the annotation policy, scoring contract, and reproduction command](../../docs/accuracy.md).
