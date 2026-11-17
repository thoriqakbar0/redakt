# Contribute to privacy-hygiene

This repository is an experimental Rust detector with deliberately limited coverage. Describe the input format and observable behavior when proposing a change. Use public or synthetic examples only. Never submit real personal data, credentials, or production logs.

## Run checks

Install stable Rust and run these commands from the repository root.

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo doc --locked --no-deps
```

The native model in `models/weights.bin` embeds Unicode 13.0.0 classification tables. The Rust build embeds this artifact directly. No Python environment or model compiler is required or included.

## Submit changes

Open an issue or pull request with a synthetic reproducer and the checks you ran. Keep detection changes separate from benchmark claims. Add behavior tests when practical, update relevant documentation, and explain user-visible changes for the maintainer's changelog.

Keep discussion respectful and focused on the work. Do not post private information or harass other contributors. Report conduct concerns through the private reporting channel in SECURITY.md.
