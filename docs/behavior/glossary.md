# Glossary

## Command and data

- **Invocation** — one running redakt process, starting with the argument list and ending with an exit status.
- **stdin**, **stdout**, **stderr** — respectively the input stream, the result stream, and the diagnostic stream. A shell can connect any of these to a terminal, pipe, or file.
- **Document** — all plain text supplied to one invocation, or the `text` of one JSONL record.
- **JSONL** — newline-delimited JSON. A **record** is one parsed input value. A **response** is one JSON object followed by a newline. An **ID** is the supplied JSON value echoed in that response.
- **Span** — a category label and half-open range in the original document. **Code point** means a Unicode scalar value here. A single visible character can contain more than one code point.
- **Category** — the kind of value a span identifies, such as `email`. A **placeholder** is the numbered text that replaces a matched range, such as `[EMAIL_1]`.
- **Overlap** — two spans share at least one code point. **Adjacent** spans meet at a boundary without sharing a code point.
- **Weights** — the native model artifact the CLI loads. **Embedded weights** are already in the executable. **File weights** are chosen by passing a path.
- **Cue** — a recognizable word or phrase introducing a private value, such as `date of birth:`. **Cue-based recognition** applies built-in format checks to that value.
- **Flush** — send buffered stdout data to its destination so the reader can receive it. This does not promise persistence on disk.
- **Diagnostic** — an error message sent to stderr with the `redakt:` prefix.

## Events that end or interrupt

| Event | Meaning |
| --- | --- |
| Ctrl+C (once or twice) | Interactive interrupt, normally delivering SIGINT to the foreground process group on Linux. |
| SIGTERM | OS request to terminate the process. |
| Terminal closes (SIGHUP) | Terminal hangup that delivers SIGHUP to the process. |
| Stdin closes (EOF) | The input stream reaches its end. Bytes already delivered remain available to read. |
| Stdout pipe closes | The downstream reader drops the pipe before a write or flush. |
| Network drops | Connectivity is interrupted while the process runs. |
| Disk full or file inaccessible | The OS rejects an open, read, or write involving a file. |
| Source file changes | A file supplying stdin or weights is edited or replaced by another process. |
| Second invocation | Another redakt process runs at the same time. |
| Process killed outright | The OS stops the process without giving it a chance to finish, for example via SIGKILL. |
