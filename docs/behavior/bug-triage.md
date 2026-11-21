# Bug triage

Four findings were reproduced with the Linux release binary built from `6ba6de0`. The address and date examples share a single delimiter problem. Severity reflects the effect on callers and on text they may share.

## Summary

| ID | Finding | Severity | Area | Decision needed |
| --- | --- | --- | --- | --- |
| B-03 | Exclamation marks block cued address and date recognition | medium | Detection | fix |
| B-04 | A product title can be redacted as a person's name | medium | Detection | product call |
| B-01 | Unwritable stderr changes the failure status | low | Invocation | fix |
| B-02 | Positional arrays are accepted as JSONL records | low | JSONL | product call |

## Medium

### B-03: Exclamation marks block cued address and date recognition

- **Where the user meets it** — A cued address or birth date ends an exclamatory sentence.
- **Observed and expected** — The examples below remain visible with an exclamation mark. Replacing the mark with a period redacts the same values and preserves the punctuation. Sentence-ending exclamation marks should also stay outside these values.
- **Reproduce** — Pass `address: 42 Maple Street!` and `DOB: 2000-02-29!` as separate plain text documents. Repeat with a period in place of the exclamation mark.
- **Cause** — The scanner in [src/contextual.rs](../../src/contextual.rs#L93) leaves the exclamation mark in the unquoted candidate. Date and address validation reject the candidate with that punctuation attached.
- **Severity and decision** — Medium because punctuation can silently leave the value visible. Fix the candidate boundary for the affected formats and preserve valid punctuation inside secrets.
- **Raised by** — [Detection](cli/detection.md#open-questions-and-verification).
- **Status** — Confirmed on Linux with the release binary on 2026-11-21, DET-08 and DET-09.

### B-04: A product title can be redacted as a person's name

- **Where the user meets it** — Text has a public field such as `Product name: Maple Editor`.
- **Observed and expected** — Redakt emits `Product name: [NAME_1]`. The independent fixture expects the public product title to stay visible.
- **Reproduce** — Pipe the example into plain text mode.
- **Cause** — The scanner in [src/contextual.rs](../../src/contextual.rs#L272) accepts the standalone `name` cue after a space and checks the value against the name grammar without interpreting the preceding word.
- **Severity and decision** — Medium because the replacement removes useful public text. Decide whether to recognize non-person name fields explicitly or retain broad matching for greater coverage of personal names.
- **Raised by** — [Detection](cli/detection.md#open-questions-and-verification).
- **Status** — Confirmed on Linux with the release binary on 2026-11-21, DET-10.

## Low

### B-01: Unwritable stderr changes the failure status

- **Where the user meets it** — A caller redirects diagnostics to a destination that rejects writes.
- **Observed and expected** — A usage error returns 101 when stderr is `/dev/full`. With writable stderr it returns 1. The expected fallback is status 1 even when the diagnostic cannot be delivered.
- **Reproduce** — On Linux run `redakt --unknown 2>/dev/full` and inspect `$?`. Repeat with writable stderr.
- **Cause** — The error path in [src/main.rs](../../src/main.rs#L106) prints to stderr before setting status 1. The failed print panics.
- **Severity and decision** — Low because the run already failed, but a script that handles status 1 can miss it. Make diagnostic delivery fallible while preserving the intended failure status.
- **Raised by** — [Invocation](foundations/invocation.md#open-questions-and-verification).
- **Status** — Confirmed on Linux with the release binary on 2026-11-21, INV-12.

### B-02: Positional arrays are accepted as JSONL records

- **Where the user meets it** — A JSONL producer sends an array rather than the documented object.
- **Observed and expected** — `[1,"a@example.test"]` is accepted as ID 1 and text `a@example.test`. The documented object shape implies the array would be rejected.
- **Reproduce** — Pipe the example as a single line into `redakt --jsonl`.
- **Cause** — The record in [src/main.rs](../../src/main.rs#L16) uses derived deserialization. At [record input](../../src/main.rs#L39), that derives support for a positional sequence as well as an object.
- **Severity and decision** — Low because valid producers are unaffected. Decide whether to enforce the documented object shape or support and document the accepted array form.
- **Raised by** — [JSONL](cli/jsonl.md#open-questions-and-verification).
- **Status** — Confirmed on Linux with the release binary on 2026-11-21, JSONL-13.
