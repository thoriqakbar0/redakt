<p align="center">
  <img src="assets/logo.svg" alt="privacy-hygiene logo" width="240">
</p>

# privacy-hygiene

A standalone Rust PII detector powered by an embedded ternary-gated rule machine. Detect and redact emails, phones, US SSNs, cards, supported IBANs, and cue-based names and addresses with one CLI binary. No model download or network access is needed after installation.

## How the ternary matcher works

The matcher uses weights in `{-1, 0, +1}` to make threshold decisions over character classes and boundaries. Five ternary weights fit in each packed byte. The binary embeds the compiled model in [models/weights.bin](models/weights.bin).

The model stores character masks, an ordered instruction graph, capture metadata, Unicode tables, and sparse checksum transitions. The Rust runtime executes that graph to find spans and redact text. It makes no regex-engine calls during inference. Luhn and IBAN checks use compiled transition tables.

This is a training-free rule-machine experiment. Its ternary weights encode compiled rules, not learned contextual understanding. Names and addresses still need explicit cues, and narrow formats can miss personal data. It is not a complete anonymization tool. Hostile inputs have no tested worst-case time or memory guarantee.

See [the ternary architecture and model format](docs/ternary.md) for the execution details.

## Install the CLI

With [Rust and Cargo](https://rustup.rs/) installed, run this from any directory. No clone is needed.

```bash
cargo install --git https://github.com/thoriqakbar0/privacy-hygiene.git --locked
```

Prefer a prebuilt binary? Check [GitHub Releases](https://github.com/thoriqakbar0/privacy-hygiene/releases). No release is published yet. Once available, download the archive for your platform, verify it against `SHA256SUMS`, extract it, and put the executable on your `PATH`. See [binary compatibility and checksum instructions](docs/releasing.md#build-the-archives).

There is no installable detector skill in this repository yet. Use the CLI directly from your terminal or agent.

## Try it

Paste this into your terminal after installation.

```bash
printf 'Email: nora@example.test' | privacy-hygiene
```

Expected JSON, with key order allowed to differ:

```json
{"redacted":"Email: █████████████████","spans":[{"start":7,"end":24,"label":"email"}]}
```

To check a UTF-8 text file, run:

```bash
privacy-hygiene < sample.txt
```

The CLI writes JSON to stdout and errors to stderr. Each result contains `spans` and `redacted`. Span offsets count Unicode code points, starting at zero, with an exclusive end. Redaction masks matched characters, including spaces, with `█`.

### Process multiple documents

Use `--jsonl` to keep the model loaded. Each input line must contain exactly `id` and `text`. Each response contains `id`, `spans`, and `redacted` and is flushed immediately.

```bash
printf '%s\n' '{"id":"one","text":"Name: Nora Chen."}' '{"id":"two","text":"a@example.test"}' | privacy-hygiene --jsonl
```

Blank lines are ignored. An invalid record stops processing with a nonzero exit after any earlier complete responses.

Run `privacy-hygiene --help` for usage or `privacy-hygiene --version` to check the installed version. Use `--weights PATH` to load another native artifact. Version 1 weights require recompilation for the current version 2 format.

### Remove additional labelled private values

Use `--contextual` to add cue-based recognition for names and addresses, labelled secrets, bank account numbers, and dates of birth. It works with plain stdin and `--jsonl`.

```bash
printf 'API_KEY=synthetic_example_key_123456' | privacy-hygiene --contextual
```

The additional pass returns ordinary spans and masks their union with the original seven categories. It runs locally without a download or a new dependency. Default detection stays unchanged. See [contextual coverage and its limits](docs/contextual.md) before relying on it.

## Coverage and measurements

Names require one of `Name`, `Patient`, `Customer`, or `My name is`. Addresses require `Address`, `Ship to`, or `Lives at`. These rules are case-sensitive. IBAN coverage is limited to GB, DE, FR, ES, NL, and IT. Checksums do not establish assignment, ownership, or privacy.

The native model is 57,721 bytes. The measured Linux executable is 830,384 bytes with the model embedded. Artifact size is separate from runtime RAM and depends on the build target.

A matched five-trial CPU comparison verified exact native/rules outputs. Native RAM was lower, while native/rules timing ranges overlapped on both short and long documents. See [the benchmark findings and raw evidence](docs/fair-benchmark.md) and [the architecture and format](docs/ternary.md).

A later matcher-loop optimization reduced median CLI time by 29% on a repeated mixed-PII fixture while preserving exact output. See [the workload, timing ranges, and reproduction](docs/performance.md). This is a fixture-specific measurement on a shared Linux VM.

The [independent detection-quality evaluation](docs/accuracy.md) reports per-category misses and false positives on hand-labelled synthetic documents for default and contextual modes. These selected examples do not establish general accuracy.

## Run the tests

For contributors, run from a checkout with Rust and Cargo installed:

```bash
cargo test --locked
```

The tests cover all seven categories, 289 frozen rules-oracle cases, Unicode offsets, JSONL, and malformed models. See [CONTRIBUTING.md](CONTRIBUTING.md#run-checks) for the full checks. GitHub Actions is manual-only.

To build macOS executables from Linux, follow [the macOS cross-compilation guide](docs/cross-compiling-macos.md). Executing and testing those binaries requires macOS.

## Project files

| Path | Purpose |
| --- | --- |
| [src/](src/) | Compiled ternary-gated matching, redaction, and JSON CLI |
| [models/weights.bin](models/weights.bin) | Embedded version 2 model |
| [tests/](tests/) | Rust CLI, model, and frozen parity checks |
| [docs/ternary.md](docs/ternary.md) | Architecture, matching contract, and native format |
| [docs/fair-benchmark.md](docs/fair-benchmark.md) | Matched comparison and external reproduction source |

The default tree contains the Rust runtime and compiled ternary model. The compiler, independent rules detector, reference runtime, and benchmark tools are retained on the [reproduction branch](docs/fair-benchmark.md). Cargo builds and tests the checked-in model directly.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

## License

The code, compiled circuit artifact, documentation, and synthetic fixtures are licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
