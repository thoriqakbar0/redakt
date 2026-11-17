<p align="center">
  <img src="assets/logo.svg" alt="privacy-hygiene logo" width="240">
</p>

# privacy-hygiene

A standalone Rust PII detector and redactor that executes an embedded, packed ternary model. It detects hyphenated US SSNs and separated North American phone numbers. No Python, NumPy, model download, or network access is required.

This is a training-free circuit experiment with limited coverage. It does not detect email, cards, IBANs, names, or addresses. It is not a complete anonymization tool. Long or hostile inputs have no tested performance or memory guarantee.

## Build and run

Install stable Rust with Cargo, then run from the repository root.

```bash
cargo build --release --locked
printf 'SSN 123-45-6789; phone +1 (212) 555-0123' | target/release/privacy-hygiene
cargo install --path . --locked
printf '123-45-6789' | privacy-hygiene
```

The CLI reads UTF-8 stdin and emits JSON with `spans` and `redacted`. Each span contains `start`, `end`, and `label`. Offsets count Unicode code points, not UTF-8 bytes, and use half-open intervals. Redaction masks the union of spans, including spaces, with `█`. CRLF and lone CR characters outside spans remain unchanged.

Invalid input or model data exits nonzero on stderr without printing stdin. Use `--help`, `--version`, or `--weights PATH` to load another artifact in the fixed native format.

The model contains 4,380 trits packed into 876 weight bytes. The complete artifact also includes thresholds, head metadata, and Unicode classification ranges. This excludes the executable and runtime memory. See [the architecture and binary format](docs/ternary.md).

## Verify

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo doc --locked --no-deps
```

## Project files

| Path | Purpose |
| --- | --- |
| [src/](src/) | Packed ternary inference, redaction, and JSON CLI |
| [models/weights.bin](models/weights.bin) | Embedded native ternary model |
| [tests/](tests/) | Native CLI behavior checks |
| [docs/ternary.md](docs/ternary.md) | Circuit architecture and binary format |

The Python compiler, reference runtime, rules detector, and baseline experiments are no longer part of this repository's current tree. Earlier revisions remain available in Git history. The checked-in binary artifact is the model source used by the Rust build; this tree does not provide a model compiler.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

## License

The code, compiled circuit artifact, and documentation are licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
