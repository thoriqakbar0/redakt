# Reversible plain-text recovery

Masking saves a mapping before writing stdout. Reverse restores exact
placeholders in arbitrary UTF-8 text, including text edited after redaction.

## Driving it with shell

Complete Launch and Doctor. Use separate scratch directories for the producer
and receiver. Transfer the encrypted mapping and code as distinct files.

```bash
mkdir -p "$scratch/producer" "$scratch/receiver"
printf 'Email: ops@example.test\r\n' >"$evidence/recovery-input.txt"
REDAKT_STATE_DIR="$scratch/producer" "$binary" <"$evidence/recovery-input.txt" >"$evidence/masked.txt"
line=$(cat "$evidence/masked.txt")
token=${line#Email: }
token=${token%$'\r'}
[[ $token =~ ^\[REDAKT_[0-9a-f]{32}\]$ ]]
printf 'Email: %s\r\n' "$token" | cmp - "$evidence/masked.txt"
files=("$scratch/producer/"*.recovery)
test "${#files[@]}" -eq 1
cp "${files[0]}" "$scratch/receiver/session.recovery"
cp "${files[0]}.code" "$scratch/receiver/session.code"
printf 'Edited %s and [REDAKT_unknown] plus %s' "$token" "$token" >"$evidence/reply.txt"
printf 'Edited ops@example.test and [REDAKT_unknown] plus ops@example.test' >"$evidence/restored-expected.txt"
"$binary" --reverse --recovery "$scratch/receiver/session.recovery" \
  --code-file "$scratch/receiver/session.code" <"$evidence/reply.txt" >"$evidence/restored.txt"
cmp "$evidence/restored-expected.txt" "$evidence/restored.txt"
printf '%064d' 0 >"$scratch/wrong.code"
if "$binary" --reverse --recovery "$scratch/receiver/session.recovery" \
    --code-file "$scratch/wrong.code" <"$evidence/reply.txt" \
    >"$evidence/wrong-code.out" 2>"$evidence/wrong-code.err"; then
  exit 1
fi
test ! -s "$evidence/wrong-code.out"
cargo test --locked --test recovery >"$evidence/recovery-tests.log" 2>&1
```

The integration tests cover multiple mappings, wrong codes, corrupt or
truncated recovery data, failed writes, and a terminated waiting writer.
Keep mapping and code files in scratch. A transfer between directories
checks that the receiver needs only the transferred pair and masked text.
For an offline environment test, use two network-disabled containers with
separate mounts for the producer and receiver.
