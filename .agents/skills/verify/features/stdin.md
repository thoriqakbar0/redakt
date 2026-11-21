# Stdin redaction

Users pipe text into the CLI and receive redacted text.

## Sub-features

- Detect the seven supported categories.
- Preserve untouched whitespace and line endings.
- Display help and the current version.
- Detect additional cued values automatically.

## How to get to it (user POV)

Run `redakt` with UTF-8 stdin. Use `--help` or `--version` for command information.

## Driving it with shell

Preconditions. Run Launch and Doctor in the parent skill.

Run the parent's Drive commands. Require the fox and CRLF to remain unchanged and the SSN to become `[SSN_1]`. Run `"$binary" --help >"$evidence/help.txt"` and confirm that it lists `--weights` and `--jsonl`. The parent's full test suite checks supported layouts, exclusions, and the frozen 289-document span oracle.

Run these commands to check a cued date on a document without a final newline.

```bash
printf '🦊 DOB: 2000-02-29' | "$binary" >"$evidence/contextual.txt"
printf '🦊 DOB: [DATE_1]' >"$evidence/contextual-expected.txt"
cmp "$evidence/contextual-expected.txt" "$evidence/contextual.txt"
cargo test --locked --test contextual
```

## Gotchas

Plain text output has the same line endings as the input, including when the last line has no line ending.
