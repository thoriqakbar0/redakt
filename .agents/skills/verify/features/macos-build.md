# macOS compilation

Developers build Intel and Apple silicon executables from Linux.

## Sub-features

- Link both macOS release targets.
- Inspect Mach-O architecture and record binary hashes.
- Keep compilation evidence separate from macOS runtime evidence.

## How to get to it (user POV)

Install the pinned tools and Rust targets using `docs/cross-compiling-macos.md`. No release publication or CI dispatch is required.

## Driving it with shell

Preconditions. Complete Launch and Doctor. Preserve the isolated `CARGO_TARGET_DIR`; the tool environment still lives under the repository's `target/` directory.

```bash
source target/macos-toolchain/bin/activate
(
  set -x
  python -m ziglang version
  uv pip list --python target/macos-toolchain/bin/python
  cargo zigbuild --release --locked --target x86_64-apple-darwin --target aarch64-apple-darwin
  file "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/redakt"
  file "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/redakt"
  sha256sum "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/redakt" "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/redakt"
) >"$evidence/macos-build.log" 2>&1
file "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/redakt" | tee "$evidence/macos-intel.txt" | grep -q 'Mach-O 64-bit x86_64 executable'
file "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/redakt" | tee "$evidence/macos-arm.txt" | grep -q 'Mach-O 64-bit arm64 executable'
deactivate
```

Both linking and format checks must pass. Copy binaries out of scratch before Cleanup only if deliverables are requested. Logs and hashes survive cleanup. To prove execution, run each architecture on a matching Mac using the guide's version and synthetic redaction checks, and capture those actions and outputs separately.

## Gotchas

Missing tools or Rust targets block this journey. Report the missing prerequisite instead of skipping a target. Rust may warn about missing `xcrun` while linking succeeds. Apple frameworks may require `SDKROOT`. Linux cannot establish macOS runtime behavior, signing, notarization, or older OS compatibility. Keep the native macOS release tests.
