# Build macOS binaries on Linux

Install stable Rust with rustup and [uv](https://docs.astral.sh/uv/getting-started/installation/). Run these commands from the repository root on Linux.

## Set up the build tools

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
uv venv target/macos-toolchain
uv pip install --python target/macos-toolchain/bin/python 'cargo-zigbuild==0.23.3' 'ziglang==0.13.0'
source target/macos-toolchain/bin/activate
```

The pinned build tools live under the ignored `target/` directory. Python supplies the build tools only. The detector and its executable remain Rust-only.

## Build both architectures

Activate the environment again in each new shell before building.

```bash
source target/macos-toolchain/bin/activate
cargo zigbuild --release --locked --target x86_64-apple-darwin --target aarch64-apple-darwin
file target/x86_64-apple-darwin/release/redakt
file target/aarch64-apple-darwin/release/redakt
```

Expect a Mach-O x86_64 executable and a Mach-O arm64 executable. [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) uses Zig as the linker. Both targets linked on Linux with Rust 1.100.0 and these tool versions without a separate macOS SDK.

Rust can warn that `xcrun` is unavailable while locating a macOS SDK. This warning occurred in the successful builds. If future dependencies need Apple frameworks or SDK headers, provide a macOS SDK through `SDKROOT` as described in cargo-zigbuild's documentation.

## Check execution on macOS

Copy the executable for your Mac's architecture to that Mac, then run it there.

```bash
chmod +x redakt
./redakt --version
printf 'SSN 123-45-6789' | ./redakt
```

Expect the version listed in `Cargo.toml` and the plain text output `SSN [SSN_1]`.

Linux cannot execute these binaries natively. A successful cross-build and Mach-O inspection do not verify macOS runtime behavior, compatibility with older macOS versions, signing, or notarization. Release builds continue to use the native macOS jobs in [the release procedure](releasing.md), which test and smoke-check the executables on macOS.
