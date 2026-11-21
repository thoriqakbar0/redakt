# Goal: describe redakt from the command line

Read the [README](README.md), [glossary](glossary.md), and [help and version pilot](cli/information.md) before editing. Refer to the foundations before adding a command journey.

## Source and reading order

Use the root checkout at the documented source commit. Read the [entry point](../../src/main.rs), then the [redaction and detector behavior](../../src/lib.rs) and [cue-based rules](../../src/contextual.rs) as applicable. Read the [CLI tests](../../tests/cli.rs), [cue tests](../../tests/contextual.rs), and [existing coverage notes](../contextual.md) before driving the release binary.

## Writing rules

- Describe what the user does and sees. Put technical detail in a technical note only where the mechanism changes the observed outcome.
- Follow the eight sections, five phases, variant rows, interrupt rows, and cross-cutting order from the README. Use a Mermaid state diagram of user-observable states.
- Use glossary terms and link to the document that owns a shared fact. Say what is surprising. Record suspected defects under open questions and in triage.
- End each feature with open questions followed by the pinned source commit. Update coverage as you draft and verify. Preserve checklist and triage IDs.
- Keep product source read-only for this work. Record uncertainty and continue.

## Things already established

- [Invocation](foundations/invocation.md) owns argument order, mode selection, stream conventions, status 0 and 1, and process-wide interruption.
- [Documents and spans](foundations/documents.md) owns document boundaries, zero-based half-open Unicode code-point offsets, overlap precedence, and numbering.
- [Help and version](cli/information.md) owns the startup short path and its output errors.
- [Plain text](cli/plain-text.md) owns buffering through EOF and writing one result. [JSONL](cli/jsonl.md) owns validation, per-line output, flush, and partial success.
- [Detection](cli/detection.md) owns recognition coverage. [Alternate weights](cli/weights.md) owns artifact choice, validation, and startup failure.
- [Proposed restore](proposed/restore.md) owns the proposed reply workflow and its pending acceptance checks.
