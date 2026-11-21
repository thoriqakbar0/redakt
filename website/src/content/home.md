# redakt

redakt is a local log redaction CLI. Run it on text before sending a log or document to a language model, a support ticket, or another person.

## Install

```sh
cargo install --git https://github.com/thoriqakbar0/redakt --locked
```

[View project releases](https://github.com/thoriqakbar0/redakt/releases) or inspect the [source and usage examples](https://github.com/thoriqakbar0/redakt#try-it).

## Redact a file

Write the result to a fresh path and inspect it before sharing. The original file stays intact.

```sh
redakt < support.log > support.redacted.log
```

For example, `Email: ops@example.test` becomes `Email: [EMAIL_1]`. Matches for the same value and category reuse a label within a document. The CLI runs offline after installation. Detection is limited to supported formats and cues and can miss private information.

## Process JSONL

Use `redakt --jsonl` with one object per input line, each with an `id` and `text` field. Each output line includes the redacted text and match spans with Unicode code-point offsets.

- [Instructions for coding assistants](/llms.txt)
- [About the CLI](/about)
- [Contact the maintainers](/contact)
- [Privacy and data handling](/privacy)
