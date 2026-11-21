# Stdin redaction

Users pipe text into the CLI and receive redacted text.

## Sub-features

- Detect learned PII in prose and the original seven rule categories.
- Preserve untouched whitespace and line endings.
- Display help and the current version.
- Detect additional cued values automatically.

## How to get to it (user POV)

Run `redakt` with UTF-8 stdin. Use `--help` or `--version` for command information.

## Driving it with shell

Preconditions. Run Launch and Doctor in the parent skill.

Run the parent's Drive commands to check numbered labels. Require the fox and CRLF to remain unchanged and the SSN to become `[SSN_1]`. Run `"$binary" --help >"$evidence/help.txt"` and confirm that it lists `--weights` and `--jsonl`. The parent's full test suite checks supported layouts, exclusions, and the frozen 289-document span oracle.

Run these commands to check a cued date on a document without a final newline.

```bash
printf '🦊 DOB: 2000-02-29' | "$binary" --irreversible >"$evidence/contextual.txt"
printf '🦊 DOB: [DATE_1]' >"$evidence/contextual-expected.txt"
cmp "$evidence/contextual-expected.txt" "$evidence/contextual.txt"
cargo test --locked --test contextual
```

Run the default CLI and reverse it in an isolated state directory.

```bash
export REDAKT_STATE_DIR="$scratch/state"
"$binary" <"$evidence/stdin.txt" >"$evidence/masked.txt"
! cmp -s "$evidence/stdin.txt" "$evidence/masked.txt"
"$binary" --reverse <"$evidence/masked.txt" >"$evidence/restored.txt"
cmp "$evidence/stdin.txt" "$evidence/restored.txt"
cargo test --locked --test recovery
REDAKT_BIN="$binary" python3 tests/openai_filter.py
```

## Gotchas

Plain text output has the same line endings as the input, including when the last line has no line ending.
