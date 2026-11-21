# Persistent JSONL

Users stream records through one process and receive a flushed response for each record.

## Sub-features

- Preserve IDs, count span offsets in Unicode code points, and ignore blank lines.
- Flush responses before EOF.
- Reject malformed records without exposing their input.
- Detect learned PII in default JSONL records. Explicit native weights select rules and cue-based detection.

## How to get to it (user POV)

Run `redakt --jsonl` and write one object containing exactly `id` and `text` per line.

## Driving it with shell

Preconditions. Run Launch and Doctor in the parent skill.

```bash
printf '%s\n' '{"id":"one","text":"🦊 Sarah called this morning."}' '' '{"id":"two","text":"clean"}' >"$evidence/records.jsonl"
printf '%s\n' 'redakt --jsonl < records.jsonl' >"$evidence/jsonl-action.txt"
"$binary" --jsonl <"$evidence/records.jsonl" >"$evidence/responses.jsonl" 2>"$evidence/jsonl-stderr.txt"
jq -se '. == [{id:"one",spans:[{start:2,end:7,label:"name"}],redacted:"🦊 [NAME_1] called this morning."},{id:"two",spans:[],redacted:"clean"}]' "$evidence/responses.jsonl"
test ! -s "$evidence/jsonl-stderr.txt"
cargo test --locked --test cli jsonl >"$evidence/jsonl-tests.log" 2>&1
```

Require both exact responses and passing streaming and private-error tests.

Run `cargo test --locked --test contextual` for the contextual JSONL and alternate-weights journeys. The supplemental policy must remain active when the supplied model's original categories are disabled. Loading alternate weights must not silently load the embedded seven-category model instead.

## Gotchas

A pipe that closes stdin cannot prove immediate flushing alone. Keep `jsonl_flushes_each_record_before_eof` in the checks. Earlier complete responses remain visible when a later record fails.
