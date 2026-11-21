<p align="center">
  <img src="assets/logo.svg" alt="redakt logo" width="240">
</p>

# redakt

Redact personal data locally with a standalone Rust CLI. The binary embeds the trained detector and the rules model and processes documents on your machine.

## Install the CLI

The Linux and macOS installer downloads a prebuilt binary from the Redakt website and verifies its SHA-256 checksum. The website serves the same archives as [GitHub Releases](https://github.com/thoriqakbar0/redakt/releases).

### Linux and macOS

Use this command on macOS or glibc-based Linux with an x86-64 or ARM64 processor.

```sh
curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh
```

The installer puts `redakt` in `~/.local/bin` by default. To choose another directory, run `REDAKT_INSTALL_DIR=/path/to/bin sh install.sh`. Add the install directory to your `PATH` if needed, then run `redakt --version`.

### Windows

On 64-bit x86 Windows, download [install.ps1](install.ps1) from GitHub or open PowerShell in a checkout of this repository. Run the script from the directory that contains it.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

The installer puts `redakt.exe` in `%LOCALAPPDATA%\redakt\bin` by default. To choose another directory, pass `-InstallDir` to the script. Add the install directory to your user `Path` in Windows, open a new terminal, and run `redakt --version`.

### Install with Cargo

Install from crates.io with [Rust and Cargo](https://rustup.rs/).

```bash
cargo install redakt --locked
```

To install from a local checkout, run `cargo install --path . --locked`.

To install a prebuilt binary by hand, download the archive for your platform. Verify it against `SHA256SUMS`, extract it, and put the executable on your `PATH`. See the [release guide](docs/releasing.md#build-the-archives) for binary compatibility.

### Update redakt

- Run `redakt update --check` to check GitHub for a newer stable release for your platform.
- On Linux and macOS, run `redakt update` to replace a standalone GitHub binary in place. Redakt verifies the archive against GitHub's SHA-256 digest before replacing the executable. The installation directory must be writable.
- On Windows, use the download link from `redakt update --check` and replace the executable after redakt exits.
- For a Cargo installation, run `cargo install redakt --locked` to upgrade from crates.io.

After a successful redaction with stderr attached to a terminal, redakt checks GitHub Releases at most once every 24 hours per local cache. A failed check also counts toward that limit. The request asks for release metadata and contains no document contents. All update checks use GitHub Releases. Cargo contacts crates.io when you run `cargo install` yourself.

Set `REDAKT_UPDATE_NOTIFY=0` in your shell to disable automatic checks. Notices go to stderr. Redacted output stays on stdout.

## Try it

Pipe in three email addresses, two street addresses, and a sample password.

```bash
printf '%s\n' \
  'Emails: nora@example.test, ada@example.test, jo@example.test' \
  'Address: 12 Oak Lane' \
  'Address: 48 Cedar Road' \
  'Password: demo-password' | redakt --irreversible
```

`--irreversible` prints numbered labels and saves no recovery files.

```text
Emails: [EMAIL_1], [EMAIL_2], [EMAIL_3]
Address: [ADDRESS_1]
Address: [ADDRESS_2]
Password: [SECRET_1]
```

### Mask and restore text

```bash
redakt < prompt.txt > masked.txt
redakt --reverse < reply.txt > restored.txt
```

Plain-text redaction writes random placeholders to stdout and automatically saves encrypted recovery data. Keep the `.recovery` file and its separate `.recovery.code` file private. To restore offline on another machine, transfer the recovery file and code separately, and supply them to redakt there. The code is reusable for that mapping. Redakt never prints the code.

Redakt uses `REDAKT_STATE_DIR` when set. Otherwise it uses `$XDG_STATE_HOME/redakt`, `%LOCALAPPDATA%\redakt` on Windows, or `~/.local/state/redakt`. Each default masking invocation creates a new mapping. If the state directory contains more than one mapping, reverse requires `--recovery PATH`. The default code path adds `.code` to the recovery path. Use `--code-file PATH` to store the code separately.

Use one explicit path per conversation to keep placeholders stable across requests.

```bash
redakt --recovery session.recovery < prompt.txt > masked.txt
redakt --reverse --recovery session.recovery --code-file /private/session.code < reply.txt
```

In the second command, `/private/session.code` stands for the location where you placed the code on the restoring machine. If you kept the code at the default path, omit `--code-file`.

Reverse accepts arbitrary UTF-8 text. It replaces exact placeholders from the selected mapping once per occurrence and preserves the surrounding bytes. Unknown placeholders and placeholders edited by a model pass through. Redakt authenticates the complete mapping before output. If the code is lost, generate a new mapping for future redaction. Existing placeholders still need their original code.

### Filter model requests and replies

Run the first command in a pre-request hook on the text that will reach the model, including replayed history and file or tool text. Run the reverse command only in a local post-response hook. Whenever the app resends displayed content, mask it again. Your app can keep its regular local history.

The [OpenAI-compatible example](examples/openai_filter.py) demonstrates text requests for Chat Completions and Responses. It reads JSON on stdin, masks supported text fields and sends the request, then writes the restored reply text locally. It buffers streamed replies through completion so split placeholders can be recognized. Unsupported request shapes fail before a network request. This example requires a client hook or an application that calls it.

```bash
python3 examples/openai_filter.py --recovery session.recovery \
  --url https://api.openai.com/v1/responses --model YOUR_MODEL < request.json
```

Set `OPENAI_API_KEY` in the environment for endpoints that require it. Keep the recovery file and code out of model requests. Detection coverage depends on redakt's models and the fields your integration passes to the filter.

### Redact PII in prose

Redakt runs the trained English PII detector alongside the built-in rules by default. It looks for names in ordinary prose, contact details, locations, identifiers, and other annotated personal information. The detector runs locally and works with both plain text and `--jsonl`.

```bash
printf '%s\n' 'Sarah called this morning. Her email is sarah@example.test.' | redakt
```

The learned detector can miss private text or mask ordinary words. Review the output before sharing it. The training data comes from NVIDIA's [Nemotron-PII dataset](https://huggingface.co/datasets/nvidia/Nemotron-PII), available under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

### Redact a text file with Claude Code or Codex

Install the CLI, then paste this prompt into Claude Code or Codex and tell it which file to redact. Any filename works as long as the file contains UTF-8 plain text.

> Use `redakt` on this machine to redact the UTF-8 text file I specify. If I haven't given you the file path, ask me for it. Pass the file directly to the CLI through stdin using `redakt --irreversible` and save its output to a new file only my account can access. Leave the original alone and use a filename that isn't already taken. Keep both files' contents out of chat, tool output, and external services. If it works, tell me where to find the copy. If it fails, tell me what went wrong without quoting either file.

Review the new file locally before sharing it.

### Process multiple documents

Use `--jsonl` for match locations or to process multiple documents with one model load. Each input line must contain exactly `id` and `text`. Each response contains `id`, `spans`, and `redacted` and is flushed immediately. JSONL uses numbered labels and saves no recovery files. Span offsets count Unicode code points in the original text, starting at zero, with an exclusive end. Placeholder numbering restarts for each record.

```bash
printf '%s\n' '{"id":"one","text":"SSN 123-45-6789"}' '{"id":"two","text":"a@example.test"}' | redakt --jsonl
```

Blank lines are ignored. An invalid record stops processing with a nonzero exit after any earlier complete responses.

Run `redakt --help` for usage or `redakt --version` to check the installed version. Use `--weights PATH` to select a native rules artifact and the built-in cue-based detector. For example, `redakt --weights models/weights.bin` runs the original seven-category rule model with cue-based detection. Version 1 rules weights require recompilation for the current version 2 format.

### Remove cued private values

The CLI detects cued names, addresses, secrets, bank account numbers, and dates of birth in plain stdin and JSONL.

```bash
printf 'API_KEY=synthetic_example_key_123456' | redakt
```

If you used `--contextual` in version 0.0.2, remove it from your commands. Cue-based detection runs automatically. See [cue-based coverage and its limits](docs/contextual.md).

## How the detectors work

The default CLI combines a supervised sequence tagger, learned field patterns, and the native rules. The tagger uses scaled ternary weights to label tokens using the surrounding text. See [training, evaluation, and reproduction](experiments/pii/README.md).

The compiled rule model has 14,237 fixed ternary weights. The binary embeds the model from [models/weights.bin](models/weights.bin). The matcher uses weights in `{-1, 0, +1}` to test character classes and boundaries. Five ternary weights fit in each packed byte.

The model stores character masks, an ordered instruction graph, capture metadata, Unicode tables, and sparse checksum transitions. The Rust runtime executes that graph to find spans and redact text. It makes no regex-engine calls during inference. Luhn and IBAN checks use compiled transition tables.

The weights encode rules at compile time. See [the ternary architecture and model format](docs/ternary.md) for the execution details.

## Coverage and measurements

The rule-based detector covers emails, phone numbers, US SSNs, payment card numbers, supported IBANs, and cued private values. Its name and address rules require [explicit cues](docs/contextual.md#supplemental-coverage). The trained detector can also recognize names and addresses in prose. IBAN rule coverage is limited to GB, DE, FR, ES, NL, and IT. Checksums do not establish assignment, ownership, or privacy.

For anonymization, treat the detector as one step in the process. The rules can miss personal data outside the formats they cover. The detector has no tested worst-case time or memory guarantee for hostile inputs.

The compiled rules model is 57,811 bytes. The previously measured Linux executable was 830,384 bytes before it included the learned detector. Artifact size is separate from runtime RAM and depends on the build target.

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
| [website/](website/) | Astro and SolidJS homepage and Cloudflare deployment setup |
| [models/weights.bin](models/weights.bin) | Embedded version 2 model |
| [models/pii.bin](models/pii.bin) | Embedded trained PII sequence model |
| [models/pii-cues.bin](models/pii-cues.bin) | Learned cue and value patterns |
| [models/pii-cues.bin](models/pii-cues.bin) | Learned cues for PII span refinement |
| [tests/](tests/) | Rust CLI, model, and frozen parity checks |
| [docs/ternary.md](docs/ternary.md) | Architecture, matching contract, and native format |
| [docs/fair-benchmark.md](docs/fair-benchmark.md) | Matched comparison and external reproduction source |

The default tree contains the Rust runtime and compiled ternary model. The compiler, independent rules detector, reference runtime, and benchmark tools are retained on the [reproduction branch](docs/fair-benchmark.md). Cargo builds and tests the checked-in model directly.

## Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and checks. Report security problems through the private channel in [SECURITY.md](SECURITY.md).

## License

The code, compiled circuit artifact, documentation, and synthetic fixtures are licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
