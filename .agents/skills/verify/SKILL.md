---
name: verify
description: Verify privacy-hygiene's real Rust CLI, JSONL streaming, alternate weights, and Linux-to-macOS compilation after behavior or build changes.
---

# Verify privacy-hygiene

Run from the repository root. Read [the feature map](features/README.md) and drive every affected entry. The primary interface is text on stdin and redacted text on stdout. Library behavior also has Rust unit tests. No server, credentials, ports, or seed data are needed. Use synthetic inputs only.

## Launch

Use Bash, stable Rust, Cargo, and jq. Give each run separate build and evidence directories. Keep these variables in one shell through Cleanup.

```bash
set -euo pipefail
mkdir -p target/verification
evidence=$(mktemp -d "$PWD/target/verification/run.XXXXXX")
scratch=$(mktemp -d)
export CARGO_TARGET_DIR="$scratch/build"
trap 'rm -rf -- "$scratch"' EXIT
printf '%s\n' "$evidence"
(
  set -x
  rustc --version
  cargo --version
  git rev-parse HEAD
  git status --short
  cargo build --release --locked
) >"$evidence/build.log" 2>&1
binary="$CARGO_TARGET_DIR/release/privacy-hygiene"
```

Require a zero build exit status and a successful Doctor check before driving. No process stays running between CLI invocations. On failure, retain the logs and clean up scratch.

## Doctor

Run this read-only check when the binary looks stale or behaves unexpectedly.

```bash
expected_version=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "privacy-hygiene") | .version')
"$binary" --version >"$evidence/version.txt"
test "$(cat "$evidence/version.txt")" = "privacy-hygiene $expected_version"
```

## Drive

Exercise the real stdin path and verify the redacted text byte for byte, including its CRLF ending.

```bash
printf '🦊 123-45-6789\r\n' >"$evidence/stdin.txt"
printf '🦊 ███████████\r\n' >"$evidence/expected.txt"
printf '%s\n' 'privacy-hygiene < stdin.txt > stdout.txt' >"$evidence/action.txt"
"$binary" <"$evidence/stdin.txt" >"$evidence/stdout.txt" 2>"$evidence/stderr.txt"
cmp "$evidence/expected.txt" "$evidence/stdout.txt"
test ! -s "$evidence/stderr.txt"
cargo test --locked >"$evidence/tests.log" 2>&1
```

Read the matching feature files for additional journeys. Run the macOS compilation journey for cross-build tooling changes. Native tests do not prove cross-target runtime correctness.

## Evidence

Keep synthetic inputs, arguments, stdout, stderr, build logs, and test logs under `$evidence`. Record the exit status of each journey in the handoff; a nonzero command fails verification even if earlier checks passed. Exercise the real user path without internal setters or mocks. Verify side effects too. Alternate weights must remain unchanged, and cross-build artifacts must have the expected architecture and recorded hashes.

Report the evidence path, passed journeys, and concrete blockers. Linux cross-builds do not prove macOS execution, signing, notarization, or older OS compatibility. Do not replace native macOS release tests.

## Cleanup

CLI commands exit on EOF. The existing streaming test owns and reaps its child. Never kill by process name. Remove only the scratch directory created by this run, including after a failed iteration. Keep evidence.

```bash
rm -rf -- "$scratch"
trap - EXIT
unset CARGO_TARGET_DIR
test -s "$evidence/build.log"
test -s "$evidence/stdout.txt"
printf 'Evidence retained at %s\n' "$evidence"
```

On failure, check the available logs instead of requiring a missing stdout artifact.

## Helpers

No helper scripts are shipped. Use these shell commands and the existing integration tests in `tests/cli.rs`. Install cross-build tools using [the macOS guide](../../../docs/cross-compiling-macos.md).
