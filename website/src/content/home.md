# redakt

Run logs and .env excerpts through redakt before sending them to an AI provider. Its built-in model runs offline on your CPU and masks detected passwords and access tokens.

The published release also detects supported credit card numbers and names or addresses with labels such as `Name:` and `Address:`.

## Install

On macOS or Linux, run the shell installer.

```sh
curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh
```

With Rust and Cargo installed, use this command.

```sh
cargo install redakt --locked
```

[View project releases](https://github.com/thoriqakbar0/redakt/releases). Version 0.0.6 and later include the trained detector.

## How it works

Redakt replaces detected secrets with labels before you send text to AI. Its model has about 2 million nonzero weights and runs offline on your CPU. It trained on 100,000 synthetic English documents from [NVIDIA’s Nemotron-PII dataset](https://huggingface.co/datasets/nvidia/Nemotron-PII).

The rules look for patterns such as card numbers and labels such as `PASSWORD`. Redakt replaces matches with numbered labels and writes the result to standard output.

Choose a new output path to keep the original intact. Check the copy for tokens, passwords, and database connection strings the model may have missed. Paste only what the AI needs.

```sh
redakt < .env > .env.redacted
```

For example, `PASSWORD=demo-db-password` becomes `PASSWORD=[SECRET_1]`. Repeated matches for the same value and category share a label within a document.

### From prompt to reply

1. Write the prompt locally. `Draft a note to ops@example.test.`
2. Run it through redakt and send the result. `Draft a note to [EMAIL_1].`
3. The AI can use the placeholder in its reply. `To: [EMAIL_1]`
4. If the reply preserves the placeholder, fill in the original value locally. `To: ops@example.test`

Redakt handles the redaction step. Keep the association between the value and its label on your machine.

## Redact personal data in prose

The trained detector can find names in English prose. For example, `Sarah called this morning.` becomes `[NAME_1] called this morning.` Review the output for missed or unintended redactions.

## Process JSONL

Use `redakt --jsonl` with one object per input line, each with an `id` and `text` field. Each output line includes the redacted text and match spans with Unicode code-point offsets.

- [Instructions for coding assistants](/llms.txt)
- [About the CLI](/about)
- [Contact the maintainers](/contact)
- [Privacy and data handling](/privacy)
