# Compiled recurrent PII matcher

The native Rust executable and Python reference execute a training-free, compiled matcher for the same seven categories as `detector.py`. These are email, separated North American phone numbers, hyphenated US SSNs, Luhn-valid cards, supported uppercase IBANs, cue-based names, and cue-based addresses.

This preserves the original rules' limitations. It does not add contextual understanding, international coverage, or production accuracy guarantees. The original 64-document synthetic corpus remains a fixture, not a representative privacy benchmark.

## Compile and run

Run from the repository root. NumPy is required for compilation and the Python reference, but not for native inference.

```bash
uv run --extra ternary python -m experiments.compile_ternary --native-output models/weights.bin
cargo build --release --locked
printf 'Name: Nora Chen. Mail nora@example.test!' | target/release/privacy-hygiene
printf 'Card 4111 1111 1111 1111.' | uv run --extra ternary python -m experiments.ternary
```

Compilation parses the original regex definitions and emits the model. Neither runtime calls the regex engine or the rules detector for recognition. The Python reference reuses the public `Span` type and redaction function.

Use Python 3.10 to regenerate the checked-in Unicode classification ranges. A newer interpreter can change Unicode word or whitespace membership. Inference uses the artifact's tables so Python and Rust agree when loading the same artifact.

The original rules detector uses its running interpreter's Unicode database. For characters classified differently by newer Unicode versions, its boundaries can differ from the checked-in model. Recompile with the baseline interpreter when comparing those characters. The recorded benchmark verifies classification agreement for every character in its corpus.

## Architecture and terminology

The earlier model used fixed 19-character windows for SSNs and phone numbers. That architecture cannot express unbounded email lengths or the variable repetitions and captures in the other rules. The current model is an ordered recurrent matcher with ternary gates and compiled checksum transitions.

The compiler emits packed ternary character-class masks, an ordered instruction graph, category entries, capture metadata, Unicode tables, and finite-state checksum transitions. Five trits fit in each packed byte. Sparse transition records identify source and destination states and their gate weights.

The runtime evaluates model gates while host control tracks input positions, branch priority, captures, and search cursors. Luhn and IBAN transition tables are generated during compilation. Their finite state replaces runtime calls to the Python checksum validators.

This is a compiled rule machine, not a learned neural detector or a dense tensor network. The representation includes integer metadata and host bookkeeping as well as ternary weights. A faster native result would compare deployed implementations across languages, not prove that ternary arithmetic itself is faster than regex.

An explicit backtracking stack supports variable-length repetition without one Python call frame per input character. Ordered branches preserve greedy and lazy choices. This retains the source patterns' potential pathological search costs. Long or hostile input has no tested worst-case time or memory guarantee.

## Matching contract

Offsets are half-open Unicode code-point positions in the original UTF-8 input. Redaction masks the union of spans, including internal spaces, with `█`, preserving character count and unrelated CRLF or lone CR characters.

Each category searches independently. Matching preserves Python regex ordering rather than globally selecting the longest alternative. Names and addresses return only their captured value, but advance the search cursor past the full match, including the cue.

Card and IBAN validation happens after the syntax match commits. A rejected checksum does not retry a shorter match or search inside the rejected match. This detail preserves existing behavior, including known missed IBANs when a greedy candidate consumes a suffix.

Python-compatible Unicode word boundaries and whitespace come from the artifact. The end anchor also preserves Python's acceptance before a final newline.

## Persistent native inference

The default CLI reads all stdin as one text and returns `spans` and `redacted`. `--jsonl` keeps a model loaded and handles one document per input line.

```bash
printf '%s\n' '{"id":"one","text":"Name: Nora Chen."}' '{"id":"two","text":"a@example.test"}' | target/release/privacy-hygiene --jsonl
```

Each response includes the request's `id`, `spans`, and `redacted`. The process flushes after each response. This is document-level streaming. Each individual document is still held in memory.

## Model format

The native artifact starts with `PHYT`, a little-endian u16 format version, and a little-endian u32 JSON payload length. Version 2 contains the recurrent program, packed trit masks, checksum transitions, and Unicode ranges. The NPZ archive stores the same native byte vector and version, so both runtimes consume one representation.

The loader checks the header, payload length, indices, gate encodings, and program structure before inference. `--weights PATH` loads an alternate native artifact. Version 1 fixed-window artifacts need recompilation. They are not silently reinterpreted as recurrent programs.

Artifact bytes are not RAM use. Loading expands packed masks and creates runtime structures. Input characters, match state, returned spans, and redaction also consume memory. See [the matched benchmark](fair-benchmark.md) for measured model, executable, and process sizes.

## Verification

```bash
cargo build --release --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
uv run --extra ternary python -m unittest discover -s tests -v
```

Tests include independent category examples, invalid checksums, Unicode boundaries and whitespace, greedy and lazy choices, captures, long repetitions, native transport, malformed artifacts, and deterministic compilation. Differential checks compare complete spans and redactions against the original rules on the corpus and generated inputs. Changing model data must change predictions, so parity alone cannot hide a rules fallback.

Exact parity on the existing corpus gives the same accuracy as the rules detector. It is not an improvement in detection quality. Use separate held-out data and independent annotations before making a broader claim.

## Current artifact

The checked-in Unicode 13.0.0 model contains 78 character masks, 468 matching instructions, 703 non-ASCII alphanumeric ranges, and eight non-ASCII whitespace ranges. Its native artifact is 57,721 bytes and its compressed NPZ archive is 16,080 bytes. The measured Linux release executable is 830,384 bytes, including the native artifact. Executable size depends on the build toolchain and target.

The 876-byte packed-weight figure described the superseded two-category fixed-window model. It does not describe this seven-category program.
