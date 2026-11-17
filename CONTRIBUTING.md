# Contribute to privacy-hygiene

This repository is an experimental detector with deliberately limited coverage. Describe the input format and observable behavior when proposing a change. Use public or synthetic examples only. Never submit real personal data, credentials, or production logs.

## Run checks

Install stable Rust and Python 3.10 or later. Install uv for the optional NumPy reference runtime. Run these commands from the repository root.

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
uv run --extra ternary python -m unittest discover -s tests -v
```

Build the release binary before the Python tests so native parity tests run. The optional `tests/opf` suite requires the separate baseline environment documented in the README. It is excluded from the lightweight CI suite because it requires PyTorch and the pinned upstream project.

The committed native model includes Unicode 13.0.0 classification tables. Use Python 3.10 when checking byte-identical native model regeneration. Other supported Python versions can run inference and parity tests.

## Submit changes

Open an issue or pull request with a synthetic reproducer and the checks you ran. Keep detection changes separate from benchmark claims. Add behavior tests when practical, update relevant documentation, and explain user-visible changes for the maintainer's changelog.

Keep discussion respectful and focused on the work. Do not post private information or harass other contributors. Report conduct concerns through the private reporting channel in SECURITY.md.
