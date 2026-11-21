# redakt product behavior

## Purpose

This specification follows the user from launching redakt to receiving redacted output or a diagnostic. Use it to predict what a command does, check a proposed change, or reproduce a surprising result.

## Scope decisions

- Surface begins with the redakt 0.0.5 baseline at commit `6ba6de0`. The recovery update describes redakt 0.0.6 at commit `78fb4c7`. Each feature footer records its source baseline. Examples use Bash on Linux, with redirected streams unless they explicitly call for a terminal.
- Configuration: embedded weights and plain text by default. The documented CLI options include JSONL and file weights.
- Run from the source checkout with `cargo build --release --locked`, then use `target/release/redakt`. Use synthetic data.
- The website, agent prompt, Rust library API, installation and release workflows, cross-platform behavior, model compilation, and performance benchmarking are outside this CLI specification. The library and the model are sources for observed CLI behavior.
- The spec lives here under `docs/behavior/` alongside the source. The source commit in each feature footer pins the behavior being described. The coverage table records the level of verification.
- Proposed behavior is tracked separately from the pinned CLI spec.

## Conventions

Use sentence case, the [glossary](glossary.md), and relative section links. Describe actions and observable results. Use a technical note where a mechanism changes what the reader should expect. Record unresolved behavior at the end of the relevant document.

## Document template

1. **Summary** gives the action and where to find it.
2. **The simple case** follows the usual path.
3. **The interaction, event by event** uses one state diagram and five phases of an invocation: Invoke, Exit immediately, Begin running, While running, Finish.
4. **Modifiers** covers Help and version, `--jsonl`, `--weights PATH`, and Terminal or pipe, in that order. Columns cover startup and changes during a run.
5. **Cancel and interrupt** uses the following fixed rows in order. Columns cover startup and the running phase.

   - Ctrl+C (once or twice)
   - SIGTERM
   - Terminal closes (SIGHUP)
   - Stdin closes (EOF)
   - Stdout pipe closes
   - Network drops
   - Disk full or file inaccessible
   - Source file changes
   - Second invocation
   - Process killed outright

6. **Interactions with other systems** addresses Configuration, Streams and Unicode, Exit and diagnostics, Files and permissions, Concurrency, and Offline use, in that order.
7. **Edge cases** records observable exceptions and boundaries.
8. **Open questions and verification** tracks uncertain or suspected behavior and cites the source commit.

## Method

Read the command entry point, the matching source and tests, and then exercise the release binary. The foundation pages own common behavior. Use [the verification protocol](verification/README.md) to record individual observations and [triage](bug-triage.md) for suspected defects.

## Structure

- [AGENTS.md](AGENTS.md) points to the working instructions.
- [goal.md](goal.md) records ownership, established facts, and the reading order.
- [glossary.md](glossary.md) defines the shared terms.
- Foundations
  - [Invocation](foundations/invocation.md) owns argument parsing, streams, lifecycle, and process-wide interruption.
  - [Documents and spans](foundations/documents.md) owns offsets, overlap, and numbering.
- CLI
  - [Help and version](cli/information.md) is the pilot and owns the information commands.
  - [Plain text](cli/plain-text.md) follows a single document from stdin to stdout.
  - [JSONL](cli/jsonl.md) follows the record loop from first response to EOF or failure.
  - [Detection](cli/detection.md) describes which values are recognized.
  - [Alternate weights](cli/weights.md) follows choosing and loading a model from a file.
  - [Restore placeholders in a reply](cli/restore.md) describes recovery locations, file selection, and the local restore workflow.
- Verification
  - [Protocol](verification/README.md) describes setup and evidence.
  - [Foundations](verification/foundations.md) checks startup and the document model.
  - [Command journeys](verification/commands.md) checks the information, plain text, and JSONL paths.
  - [Detection and weights](verification/detection.md) checks recognition and file weights.
- [Bug triage](bug-triage.md) gathers findings for product decisions.

## Coverage
Use `not started`, `drafted`, or `verified` for current behavior and `proposed` for unimplemented features. See the [verification protocol](verification/README.md#how-to-run-a-pass) before moving a document to `verified`.

| Document | Status |
| --- | --- |
| Foundations / Invocation | drafted |
| Foundations / Documents and spans | drafted |
| CLI / Help and version | drafted |
| CLI / Plain text | drafted |
| CLI / JSONL | drafted |
| CLI / Detection | drafted |
| CLI / Alternate weights | drafted |
| CLI / Restore placeholders in a reply | drafted |
| Glossary | drafted |
| Verification protocol and three checklists | drafted |
| Bug triage | drafted |

## Reference

- [Project README](../../README.md) and [contributor checks](../../CONTRIBUTING.md)
- [Command entry point](../../src/main.rs) for options, model startup, both input loops, and exit status
- [Detection and redaction](../../src/lib.rs) and [cue-based recognition](../../src/contextual.rs)
- [CLI tests](../../tests/cli.rs), [cue tests](../../tests/contextual.rs), and [independent accuracy fixtures](../../tests/fixtures/README.md)
- [Embedded weights](../../models/weights.bin) and [model format](../ternary.md)
