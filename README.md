<p align="center">
  <img src="assets/logo.svg" alt="privacy-hygiene logo" width="240">
</p>

# privacy-hygiene

A standalone Rust PII detector and redactor that executes an embedded compiled matcher with ternary gates. It covers email, separated North American phones, hyphenated US SSNs, Luhn-valid cards, six supported uppercase IBAN formats, and cue-based names and addresses. No Python, NumPy, model download, or network access is required.

This is a training-free rule-machine experiment, not a learned contextual detector or a complete anonymization tool. It preserves the original rules' narrow formats and known failures. Long or hostile inputs have no tested performance or memory guarantee.

## Build and run

Install stable Rust with Cargo, then run from the repository root.

```bash
cargo build --release --locked
printf 'Name: Nora Chen. Mail nora@example.test!' | target/release/privacy-hygiene
cargo install --path . --locked
printf '123-45-6789' | privacy-hygiene
```

The CLI reads UTF-8 stdin and emits JSON with `spans` and `redacted`. Each span contains `start`, `end`, and `label`. Offsets are half-open Unicode code-point positions, not UTF-8 bytes. Redaction masks the union of spans, including spaces, with `█`. CRLF and lone CR characters outside spans remain unchanged.

Invalid input or model data exits nonzero on stderr without printing stdin. Use `--help`, `--version`, or `--weights PATH` to load another native artifact. Version 2 replaces the former fixed-window format. Version 1 weights require recompilation.

Use `--jsonl` to keep the model loaded across documents. Each input line has exactly `id` and `text`; each response contains `id`, `spans`, and `redacted` and is flushed immediately. Blank lines are ignored. A later invalid record exits nonzero after any earlier complete responses.

```bash
printf '%s\n' '{"id":"one","text":"Name: Nora Chen."}' '{"id":"two","text":"a@example.test"}' | privacy-hygiene --jsonl
```

## Coverage and measurements

Names require one of `Name`, `Patient`, `Customer`, or `My name is`. Addresses require `Address`, `Ship to`, or `Lives at`. These rules are case-sensitive. IBAN coverage is limited to GB, DE, FR, ES, NL, and IT. Checksums do not establish assignment, ownership, or privacy.

The native model is 57,721 bytes. The measured Linux executable is 830,384 bytes with the model embedded. Artifact size is separate from runtime RAM and depends on the build target.

A matched five-trial CPU comparison verified exact native/rules outputs. Native RAM was lower, while native/rules timing ranges overlapped on both short and long documents. See [the benchmark findings and raw evidence](docs/fair-benchmark.md) and [the architecture and format](docs/ternary.md).

## Verify

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo doc --locked --no-deps
```

Rust tests cover all seven categories, a frozen 289-document independent rules oracle, long matches, checksum selection, Unicode offsets and whitespace, persistent JSONL, altered weights, and malformed models. Local verification remains the default. The optional cross-platform GitHub Actions workflow is manual-only.

## Project files

| Path | Purpose |
| --- | --- |
| [src/](src/) | Compiled ternary-gated matching, redaction, and JSON CLI |
| [models/weights.bin](models/weights.bin) | Embedded version 2 model |
| [tests/](tests/) | Rust CLI, model, and frozen parity checks |
| [docs/ternary.md](docs/ternary.md) | Architecture, matching contract, and native format |
| [docs/fair-benchmark.md](docs/fair-benchmark.md) | Matched comparison and external reproduction source |

This tree remains Rust-only. The Python compiler, independent rules detector, reference runtime, and benchmark reproduction tools are retained on a separate reproduction branch linked from the documentation. Cargo does not need them to build or test the checked-in artifact.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

## License

The code, compiled circuit artifact, documentation, and synthetic fixtures are licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
