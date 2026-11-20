<p align="center">
  <img src="assets/logo.svg" alt="privacy-hygiene logo" width="240">
</p>

# privacy-hygiene

Redact personal data locally with a standalone Rust CLI. The binary embeds the model and runs offline after installation.

## Install the CLI

Install version 0.0.3 from crates.io with [Rust and Cargo](https://rustup.rs/).

```bash
cargo install privacy-hygiene --locked
```

To install from the current development branch, run `cargo install --git https://github.com/thoriqakbar0/privacy-hygiene.git --locked`.

For a prebuilt binary, download the version 0.0.3 archive for your platform from [GitHub Releases](https://github.com/thoriqakbar0/privacy-hygiene/releases). Verify it against `SHA256SUMS`, extract it, and put the executable on your `PATH`. See [binary compatibility and checksum instructions](docs/releasing.md#build-the-archives).

## Try it

After installing, pipe in a sample email address.

```bash
printf 'Email: nora@example.test\n' | privacy-hygiene
```

The command prints the redacted text.

```text
Email: █████████████████
```

To save the output for a UTF-8 text file, redirect it to a separate file.

```bash
privacy-hygiene < sample.txt > result
```

The CLI writes redacted text to stdout and errors to stderr. It preserves line endings and masks matched characters, including spaces, with `█`.

### Redact a file with Claude Code or Codex

Both plugins use the same [redaction skill](plugins/privacy-hygiene/skills/privacy-hygiene/SKILL.md). Install the CLI first, then run the commands for your agent from this checkout.

**Claude Code**

```bash
claude plugin marketplace add "$PWD"
claude plugin install privacy-hygiene@privacy-hygiene
```

Start a new session and enter `/privacy-hygiene:privacy-hygiene redact support.log for sharing`.

**Codex**

```bash
codex plugin marketplace add .
codex plugin add privacy-hygiene@privacy-hygiene
```

Start a new session and ask `Use the privacy-hygiene plugin to redact support.log for sharing.`

Review the new file locally before sharing it.

### Process multiple documents

Use `--jsonl` for match locations or to process multiple documents with one model load. Each input line must contain exactly `id` and `text`. Each response contains `id`, `spans`, and `redacted` and is flushed immediately. Span offsets count Unicode code points, starting at zero, with an exclusive end.

```bash
printf '%s\n' '{"id":"one","text":"SSN 123-45-6789"}' '{"id":"two","text":"a@example.test"}' | privacy-hygiene --jsonl
```

Blank lines are ignored. An invalid record stops processing with a nonzero exit after any earlier complete responses.

Run `privacy-hygiene --help` for usage or `privacy-hygiene --version` to check the installed version. Use `--weights PATH` to load another native artifact. Version 1 weights require recompilation for the current version 2 format.

### Remove cued private values

The CLI detects cued names, addresses, secrets, bank account numbers, and dates of birth in plain stdin and JSONL.

```bash
printf 'API_KEY=synthetic_example_key_123456' | privacy-hygiene
```

If you used `--contextual` in version 0.0.2, remove it from your commands. The CLI masks both native model spans and cue-based spans. See [cue-based coverage and its limits](docs/contextual.md).

## How the ternary matcher works

privacy-hygiene detects personal data locally using a compiled rule model with 14,237 fixed ternary weights. The binary embeds the model from [models/weights.bin](models/weights.bin). The matcher uses weights in `{-1, 0, +1}` to test character classes and boundaries. Five ternary weights fit in each packed byte.

The model stores character masks, an ordered instruction graph, capture metadata, Unicode tables, and sparse checksum transitions. The Rust runtime executes that graph to find spans and redact text. It makes no regex-engine calls during inference. Luhn and IBAN checks use compiled transition tables.

The weights encode rules at compile time. See [the ternary architecture and model format](docs/ternary.md) for the execution details.

## Coverage and measurements

The CLI detects emails, phone numbers, US SSNs, payment card numbers, supported IBANs, and cued private values. Names and addresses require [explicit cues](docs/contextual.md#supplemental-coverage). IBAN coverage is limited to GB, DE, FR, ES, NL, and IT. Checksums do not establish assignment, ownership, or privacy.

For anonymization, treat the detector as one step in the process. The rules can miss personal data outside the formats they cover. This experimental detector has no tested worst-case time or memory guarantee for hostile inputs.

The native model is 57,811 bytes. The previously measured Linux executable was 830,384 bytes with the model embedded. Artifact size is separate from runtime RAM and depends on the build target.

A matched five-trial CPU comparison verified exact native/rules outputs. Native RAM was lower, while native/rules timing ranges overlapped on both short and long documents. See [the benchmark findings and raw evidence](docs/fair-benchmark.md) and [the architecture and format](docs/ternary.md).

A later matcher-loop optimization reduced median CLI time by 29% on a repeated mixed-PII fixture while preserving exact output. See [the workload, timing ranges, and reproduction](docs/performance.md). This is a fixture-specific measurement on a shared Linux VM.

A subsequent model and CLI optimization reduced median JSONL time by 49% on a larger version of that fixture. Reusing matcher storage and serializing typed responses preserved parsed JSON results. See [the follow-up measurements](docs/performance.md#model-storage-and-cli-serialization).

The [independent detection-quality evaluation](docs/accuracy.md) reports per-category misses and false positives on hand-labelled synthetic documents. These selected examples do not establish general accuracy.

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
| [src/](src/) | Compiled ternary-gated matching, redaction, and CLI |
| [models/weights.bin](models/weights.bin) | Embedded version 2 model |
| [tests/](tests/) | Rust CLI, model, and frozen parity checks |
| [docs/ternary.md](docs/ternary.md) | Architecture, matching contract, and native format |
| [docs/fair-benchmark.md](docs/fair-benchmark.md) | Matched comparison and external reproduction source |

The default tree contains the Rust runtime and compiled ternary model. The compiler, independent rules detector, reference runtime, and benchmark tools are retained on the [reproduction branch](docs/fair-benchmark.md). Cargo builds and tests the checked-in model directly.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

## License

The code, compiled circuit artifact, documentation, and synthetic fixtures are licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
