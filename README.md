<p align="center">
  <img src="assets/logo.svg" alt="redakt logo" width="240">
</p>

# redakt

Redact secrets and personal data in logs, prompts, and other UTF-8 text. Redakt reads from stdin and writes redacted text to stdout. The Rust CLI includes its detectors and processes text on your machine.

## Try it

After you [install redakt](#install-the-cli), run this example in a shell on Linux or macOS.

```sh
printf '%s\n' 'Sarah called this morning.' \
	'Email: sarah@example.test' 'Password: demo-password' | redakt --irreversible
```

Redakt replaces the name, email address, and password with numbered labels.

```text
[NAME_1] called this morning.
Email: [EMAIL_1]
Password: [SECRET_1]
```

`--irreversible` saves no recovery files. Review redacted text before you share it. Redakt can miss private data or mask ordinary words.

## Install the CLI

The install scripts download a prebuilt archive and verify its SHA-256 checksum.

To install by hand, download the archive for your platform and `SHA256SUMS`. Verify the archive, extract it, and put the executable on your `PATH`. See [GitHub Releases](https://github.com/thoriqakbar0/redakt/releases) and the [release guide](docs/releasing.md#build-the-archives).

### Linux and macOS

Run the installer on glibc-based Linux or macOS with an x86-64 or ARM64 processor.

```sh
curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh
```

The installer uses `~/.local/bin` by default. To install elsewhere, run `REDAKT_INSTALL_DIR=/path/to/bin sh install.sh`. Add the install directory to your `PATH` if needed, then run `redakt --version`.

### Windows

On x86-64 Windows, download [install.ps1](install.ps1) and run it from its folder in 64-bit Windows PowerShell.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

The installer uses `%LOCALAPPDATA%\redakt\bin` by default. To install elsewhere, pass `-InstallDir`. Add the install directory to your user `Path`. Open a new terminal and run `redakt --version`.

### Install with Cargo

Use [Rust and Cargo](https://rustup.rs/) to install redakt from crates.io.

```sh
cargo install redakt --locked
```

To install from a local checkout, run `cargo install --path . --locked`.

## Redact a file for sharing

On Linux or macOS, run these commands to redact a UTF-8 file and print the path to a private copy. Replace `input.txt` with your source file.

```sh
copy=$(mktemp ./redacted.XXXXXX) &&
	redakt --irreversible < input.txt > "$copy" &&
	printf '%s\n' "$copy"
```

If you use Claude Code or Codex, paste this prompt and supply the file path.

> Run `redakt --irreversible` on the file I specify. Pass the file directly to stdin and save stdout to a new file that only my account can access. Keep the original file intact. Keep both files' contents out of chat, tool output, and external services. Report the new path, or explain the error without quoting either file.

Open the copy locally and check it for missed or unintended redactions before you share it.

## Restore private values in a reply

Use plain `redakt` to replace detected values with random placeholders and save an encrypted recovery file. Use one recovery file per conversation so that repeated values keep their placeholders across requests.

```sh
redakt --recovery session.recovery < prompt.txt > masked.txt
```

Review `masked.txt` before sending it. Save the reply as `reply.txt`, then restore it locally.

```sh
redakt --reverse --recovery session.recovery < reply.txt > restored.txt
```

Redakt restores exact placeholders from the selected mapping. Unknown or edited placeholders stay as written.

Keep `session.recovery` and its separate code file, `session.recovery.code`, private. You need both files to restore the original values. To restore on another machine, transfer the files separately. Pass `--code-file PATH` if the code lives somewhere other than the default path. See [recovery file locations and restoration](docs/behavior/cli/restore.md).

### Filter model requests and replies

In an application, redact the text before each request, including any history, file text, or tool output that the app resends. Restore the reply only for local use.

The [OpenAI-compatible Python example](examples/openai_filter.py) reads request JSON from stdin. It masks supported Chat Completions and Responses text fields, sends the request, and writes the restored reply to stdout. It waits for streamed replies to finish before restoring placeholders. Unsupported request shapes fail before the network request.

```sh
python3 examples/openai_filter.py --recovery session.recovery \
	--url https://api.openai.com/v1/responses --model YOUR_MODEL < request.json
```

Set `OPENAI_API_KEY` if your endpoint requires it. Keep both recovery files out of requests.

## Process multiple documents

Use `--jsonl` to process multiple documents with one model load or to get match offsets. Send one JSON object per line with exactly two fields, `id` and `text`. Use IDs that are safe to share because redakt copies them to the output.

```sh
printf '%s\n' '{"id":"one","text":"SSN 123-45-6789"}' \
	'{"id":"two","text":"a@example.test"}' | redakt --jsonl
```

Redakt flushes one response per record with `id`, `redacted`, and `spans`. Span offsets count Unicode code points in the original text from zero, with an exclusive end. Numbered labels start over for each record. JSONL saves no recovery files. See [JSONL input errors and streaming behavior](docs/behavior/cli/jsonl.md).

## Update redakt

Run `redakt update --check` to look for a newer stable release for your platform.

- To update a standalone Linux or macOS binary in a writable directory, run `redakt update`. Redakt verifies the archive against GitHub's SHA-256 digest before replacing the executable.
- On Windows, use the download link from `redakt update --check` when an update is available. Replace the executable after redakt exits.
- To update a Cargo installation, run `cargo install redakt --locked`.

After a successful redaction with stderr attached to a terminal, redakt checks GitHub Releases at most once every 24 hours per local cache. The check sends no document contents. Notices go to stderr. Set `REDAKT_UPDATE_NOTIFY=0` to turn off automatic checks.

## Check detection coverage

Redakt combines rules for values such as emails, US Social Security numbers, and payment card numbers with a trained detector for personal data in English prose. The rules also look for cues such as `Password:` or `DOB:`. See the [cue-based rules and their limits](docs/contextual.md) and the [independent detection evaluation](docs/accuracy.md).

Run `redakt --help` for the available options. Use `--weights PATH` to load a version 2 native rules model with the built-in cue-based detector. Recompile version 1 weights before use. See the [native model format](docs/ternary.md#native-format).

For the implementation and measurements, see the [ternary rule model](docs/ternary.md), the [trained detector and evaluation](experiments/pii/README.md), and the [CPU and memory benchmarks](docs/fair-benchmark.md) with [follow-up timings](docs/performance.md).

## Contribute

Run `cargo test --locked` from the checkout. See the [contributor guide](CONTRIBUTING.md#run-checks) for the full local checks and the [macOS cross-compilation guide](docs/cross-compiling-macos.md) for building macOS binaries from Linux. Report vulnerabilities through the private channel in the [security policy](SECURITY.md).

## License

The code, compiled rules model, documentation, and synthetic fixtures use the [Apache 2.0 license](LICENSE). The trained detector uses annotations from NVIDIA's [Nemotron-PII dataset](https://huggingface.co/datasets/nvidia/Nemotron-PII) under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). See [NOTICE](NOTICE) for attribution.
