# Contribute to redakt

Describe the input format and observable behavior when proposing a detector change. Use public or synthetic examples only. Never submit real personal data, credentials, or production logs.

## Run checks

Use the project [verification skill](.agents/skills/verify/SKILL.md) for recorded CLI journeys and Linux-to-macOS compilation checks. Its feature map distinguishes build verification from runtime verification on a Mac.

Install stable Rust and run these commands from the repository root.

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
cargo doc --locked --no-deps
```

Local checks are the default verification for pull requests. Include their results in the PR description. GitHub Actions runs only when manually requested through the CI workflow's **Run workflow** button. Pushes and pull requests do not start it. The optional workflow retains the Linux, macOS, and Windows matrix. Local checks prove only the platform and versions you ran.

The v2 native model in `models/weights.bin` embeds Unicode 13.0.0 classification tables. The Rust build embeds this artifact directly. No Python environment or model compiler is required or included in the default tree. The separate reproduction source is linked from [the benchmark guide](docs/fair-benchmark.md). Frozen rules-oracle fixtures and independent known answers run through the native CLI using Rust alone.

## Submit changes

Open an issue or pull request with a synthetic reproducer and the checks you ran. Keep detection changes separate from benchmark claims. Add behavior tests when practical, update relevant documentation, and explain user-visible changes for the maintainer's changelog.

Keep discussion respectful and focused on the work. Do not post private information or harass other contributors. Report conduct concerns through the private reporting channel in SECURITY.md.
