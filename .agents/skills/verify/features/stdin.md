# Stdin redaction

Users pipe text into the CLI and receive spans and redacted text.

## Sub-features

- Detect the seven supported categories.
- Preserve Unicode code-point offsets and untouched whitespace.
- Display help and the current version.

## How to get to it (user POV)

Run `privacy-hygiene` with UTF-8 stdin. Use `--help` or `--version` for command information.

## Driving it with shell

Preconditions. Run Launch and Doctor in the parent skill.

Run the parent's Drive commands. Require the fox and CRLF to remain unchanged and the SSN span to start at 2 and end at 13. Run `"$binary" --help >"$evidence/help.txt"` and confirm that it lists `--weights` and `--jsonl`. The parent's full test suite checks supported layouts, exclusions, and the frozen 289-document oracle.

## Gotchas

Offsets count code points, not UTF-8 bytes. The detector covers narrow rule formats. These checks do not prove complete anonymization or hostile-input performance.
