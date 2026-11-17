# Alternate native weights

Users can load a native artifact instead of the embedded model.

## Sub-features

- Load `--weights PATH`.
- Reject missing and malformed models.
- Preserve the model file while detecting.

## How to get to it (user POV)

Pass `--weights models/weights.bin` to the CLI.

## Driving it with shell

Preconditions. Complete the parent's stdin Drive first.

```bash
sha256sum models/weights.bin >"$evidence/model-before.sha256"
printf '%s\n' 'privacy-hygiene --weights models/weights.bin < stdin.txt' >"$evidence/weights-action.txt"
"$binary" --weights models/weights.bin <"$evidence/stdin.txt" >"$evidence/weights.json" 2>"$evidence/weights-stderr.txt"
cmp "$evidence/stdout.json" "$evidence/weights.json"
test ! -s "$evidence/weights-stderr.txt"
sha256sum --check "$evidence/model-before.sha256" >"$evidence/model-check.txt"
cargo test --locked --test cli model >"$evidence/model-tests.log" 2>&1
cargo test --locked --test cli weights >"$evidence/weights-tests.log" 2>&1
```

Require identical embedded and file-loaded output, an unchanged checksum, and passing malformed-model and weights tests.

## Gotchas

Version 1 weights need recompilation. On macOS use `shasum -a 256` and compare before and after hashes instead of GNU `sha256sum`.
