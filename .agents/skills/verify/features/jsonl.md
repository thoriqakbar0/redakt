# Persistent JSONL

Users stream records through one process and receive a flushed response for each record.

## Sub-features

- Preserve IDs and ignore blank lines.
- Flush responses before EOF.
- Reject malformed records without exposing their input.

## How to get to it (user POV)

Run `privacy-hygiene --jsonl` and write one object containing exactly `id` and `text` per line.

## Driving it with shell

Preconditions. Run Launch and Doctor in the parent skill.

```bash
printf '%s\n' '{"id":"one","text":"123-45-6789"}' '' '{"id":"two","text":"clean"}' >"$evidence/records.jsonl"
printf '%s\n' 'privacy-hygiene --jsonl < records.jsonl' >"$evidence/jsonl-action.txt"
"$binary" --jsonl <"$evidence/records.jsonl" >"$evidence/responses.jsonl" 2>"$evidence/jsonl-stderr.txt"
jq -se '. == [{id:"one",spans:[{start:0,end:11,label:"ssn"}],redacted:"███████████"},{id:"two",spans:[],redacted:"clean"}]' "$evidence/responses.jsonl"
test ! -s "$evidence/jsonl-stderr.txt"
cargo test --locked --test cli jsonl >"$evidence/jsonl-tests.log" 2>&1
```

Require both exact responses and passing streaming and private-error tests.

## Gotchas

A pipe that closes stdin cannot prove immediate flushing alone. Keep `jsonl_flushes_each_record_before_eof` in the checks. Earlier complete responses remain visible when a later record fails.
