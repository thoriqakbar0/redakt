# Public-source readiness

## Audit performed on 2026-11-17

The owner confirmed that the project has handled only public or synthetic data, with no credentials, production data, or client-confidential material.

Gitleaks 8.30.1 reported no secrets in the full fetched branch and pull-request history, using `gitleaks git . --log-opts='--all --full-history' --redact`. Its scan summary reported 34 commits. A working-tree scan also reported no secrets. Automated detection cannot establish the provenance of data or prove the absence of secrets.

The GitHub inventory contained five pull requests, no issues, two Actions runs, no Actions artifacts, no releases, and no enabled wiki. The downloaded Actions logs, PR bodies, issue comments, review comments, and PR reviews were included in the secret scan. No secrets were detected. These results cover available GitHub records, not unknown clones, forks, or deleted records.

Recorded benchmark evidence intentionally retains measurement-time filesystem paths and host characteristics. These are reproducibility context, not portable setup instructions. The corpus contains synthetic personal-data examples. Never replace them with production or client data.

## Before changing visibility

- Review and land the readiness changes. Run CI on Linux, macOS, and Windows with Python 3.10 and 3.14. Local Linux checks do not prove the other platforms.
- Confirm Apache-2.0 matches the copyright holders' intent. Upstream OpenAI Privacy Filter uses Apache-2.0; its separately downloaded checkpoint and dependencies retain their own terms.
- Enable private vulnerability reporting and test the link in SECURITY.md.
- Configure a default-branch ruleset requiring PRs and the CI checks, with force pushes disabled. GitHub currently returns HTTP 403 for rulesets and branch protection on this private repository under its current plan. Configure protection when the repository becomes public, or upgrade the plan first.
- Enable secret scanning and push protection after publication, and verify both settings.

## Local verification

The locked release build and README stdin example passed in a clean clone with the proposed files copied over. All 13 Python behavior and native parity tests passed there on Python 3.12 with NumPy installed. The same 13 tests passed on Linux with Python 3.10 and 3.14, with no skips. Rust formatting, Clippy, Rust tests, and local API documentation generation passed. Actionlint 1.7.12 and zizmor 1.32.0 reported no workflow findings. Zizmor used its default offline mode. The macOS and Windows jobs remain unverified until CI runs.

## Deliberate limits

This is a source-distributed research prototype. Cargo publishing remains disabled. No package, tag, release, deployment, or announcement is created by readiness preparation. Binary distribution and trusted publishing need a separate release decision.

The repository documents CLI usage and model architecture. `cargo doc --locked --no-deps` generates local Rust API documentation. A hosted API site is deferred until the library is offered as a supported API.

CI enforces Rust formatting and Clippy and runs the existing Python behavior and native parity suite with NumPy installed. It does not run the heavyweight upstream PyTorch adapter suite. Python formatting and static typing remain unconfigured; adding them should be a focused follow-up rather than a repository-wide style rewrite during publication preparation. No coverage threshold is claimed.

The committed native artifact uses Unicode 13.0.0 data. Python 3.10 is required for byte-identical native regeneration; inference tests run on newer Python versions too.
