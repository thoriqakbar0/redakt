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

The code file is generated alongside a new mapping, or at the path selected by
`--code-file`. Use a separate secure channel to transfer the code. The restoring
machine needs the code and encrypted mapping, plus the text to restore. It
does not need the original prompt or a network connection.

Redakt commits the mapping before emitting masked output. A write failure
produces no masked stdout. An interrupted first write can leave a code file
without its mapping. Start with fresh recovery and code paths in that case.
An established mapping is atomically replaced after encryption. Concurrent
writers use its lock file to serialize updates.

Reverse authenticates the entire mapping before emitting any restored text.
A missing or incorrect code, a truncated mapping, or an invalid file fails
with a diagnostic and no restored stdout. If there are several default
mappings, select one with `--recovery`.

## Verification

The [recovery CLI tests](../../../tests/recovery.rs) exercise arbitrary replies,
multiple scopes, Unicode, transfer, authentication, and writer termination.
The [adapter tests](../../../tests/openai_filter.py) capture provider requests
and exercise retry, replayed history, tool results, and streamed replies.
