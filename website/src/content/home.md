# redakt

Keep secrets out of AI prompts. Use redakt to mask sensitive data in logs, .env files, and prompts on your machine before sharing. The built-in model runs offline on your CPU.

## Install

On macOS or Linux, run the shell installer.

```sh
curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh
```

On 64 bit x86 Windows, download [install.ps1](https://github.com/thoriqakbar0/redakt/blob/main/install.ps1) and run this from its folder in PowerShell.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1
```

With Rust and Cargo installed, use this command.

```sh
cargo install redakt --locked
```

[View project releases](https://github.com/thoriqakbar0/redakt/releases). Version 0.0.6 and later include the trained detector.

## Local model

Redakt's built-in model runs offline on your CPU. Rules catch patterns like card numbers. The model uses nearby words to spot names and addresses in English sentences. For example, it can flag “Sarah” in “Sarah called this morning.” It learned from 100,000 synthetic English documents. [Training and results](https://github.com/thoriqakbar0/redakt/tree/main/experiments/pii).

## How it works

### Prepare a copy to share

Use `--irreversible` to replace matches with numbered labels such as `[SECRET_1]` without saving recovery files. This command saves the output in a private file and prints its path.

```sh
copy=$(mktemp .env.redacted.XXXXXX) && redakt --irreversible < .env > "$copy" && printf '%s\n' "$copy"
```

Review the copy for missed or unintended redactions before sharing it. Repeated matches for the same value and category share a label within a document.

### Restore values in an AI reply

Plain-text redaction saves an encrypted recovery file and a separate code by default. Use the same recovery file for the prompt and the reply.

```sh
redakt --recovery session.recovery < prompt.txt > masked.txt
redakt --reverse --recovery session.recovery < reply.txt > restored.txt
```

Review `masked.txt` before sending it. Save the AI's reply as `reply.txt`. Redakt can put private details back when the AI leaves their placeholders unchanged. Keep `session.recovery` and `session.recovery.code` private.

## Process JSONL

Use `redakt --jsonl` with one object per input line, each with an `id` and `text` field. Each output line includes the redacted text and match spans with Unicode code-point offsets.

[Instructions for coding assistants](/llms.txt)

## Privacy

Redakt processes text on your machine. Update checks ask GitHub for release information only. Set `REDAKT_UPDATE_NOTIFY=0` to turn off automatic checks.

## Support

Ask questions or report bugs in [GitHub issues](https://github.com/thoriqakbar0/redakt/issues). Include your redakt version, operating system, and a synthetic example for bug reports. Follow the [security policy](https://github.com/thoriqakbar0/redakt/blob/main/SECURITY.md) for private vulnerability reports.
