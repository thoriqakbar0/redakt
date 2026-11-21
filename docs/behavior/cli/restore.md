# Restore placeholders in a reply

Pipe the text you send to an LLM through `redakt` and pipe the reply through
`redakt --reverse` before showing it locally. Use a separate recovery file for
each conversation. The same exact value reuses its placeholder within a mapping.

```sh
redakt --recovery session.recovery < prompt.txt > masked.txt
redakt --reverse --recovery session.recovery < reply.txt > restored.txt
```

Reverse accepts arbitrary UTF-8 text. Each exact placeholder is replaced once
per occurrence. Altered, unknown, and unrelated placeholders pass through as
written. Redakt keeps unaffected bytes, including Unicode and line endings.

## Recovery data

### Default locations

Redakt chooses the first applicable directory in this order.

| Setting | Directory |
| --- | --- |
| `REDAKT_STATE_DIR` is set | The path you supplied, used as-is |
| `XDG_STATE_HOME` is set | `$XDG_STATE_HOME/redakt` |
| `LOCALAPPDATA` is set | `%LOCALAPPDATA%\redakt` on a typical Windows setup |
| `HOME` is set | `$HOME/.local/state/redakt` |
| None of these variables are set | Redakt asks you to set `REDAKT_STATE_DIR` and exits with an error |

Each default plain-text redaction creates a new pair in that directory.
The encrypted mapping is named `<id>.recovery` and its code file is named
`<id>.recovery.code`. The same randomly generated ID appears in both names.
The code stays in its file and redakt does not display it.

### Choose files explicitly

`--recovery PATH` selects the exact mapping path. When you supply it, redakt
uses that path regardless of the default directory. By default it appends
`.code` to the mapping path, so `--recovery session.recovery` pairs with
`session.recovery.code`. Use `--code-file PATH` to use a separate location.

Without `--recovery`, reverse uses the sole entry whose name ends in
`.recovery` in the default directory. With no candidate it reports
`No recovery file found`. With more than one candidate it asks you to select
one with `--recovery`. In either case stdout stays empty.

To restore on another machine, transfer the encrypted mapping and the
text to restore. Transfer the code separately through a secure channel.
The receiving machine can restore offline using those files.

Redakt commits the mapping before emitting masked output. A write failure
produces no masked stdout. An interrupted first write can leave a code file
without its mapping. Start with fresh recovery and code paths in that case.
An established mapping is atomically replaced after encryption. Concurrent
writers use its lock file to serialize updates.

Reverse authenticates the entire mapping before emitting any restored text.
A missing or incorrect code, a truncated mapping, or an invalid file fails
with a diagnostic and no restored stdout.

## Open questions and verification

The [recovery CLI tests](../../../tests/recovery.rs) exercise arbitrary replies,
multiple scopes, Unicode, transfer, authentication, and writer termination.
The [adapter tests](../../../tests/openai_filter.py) capture provider requests
and exercise retry, replayed history, tool results, and streamed replies.

The [recovery checklist](../verification/commands.md#restore) records the Linux
environment and path checks. Native Windows and macOS path behavior remains
unverified.

Verified against redakt commit `78fb4c7` for the Linux checks above.
