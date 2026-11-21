# Publish a version

Cargo and GitHub Releases are separate publication steps. Ordinary pushes and pull requests do not run the distribution workflow or publish packages or binaries. The **Prepare release** workflow checks all five binary archives and creates a draft release only when manually dispatched.

## Prepare the source

1. Set the version in `Cargo.toml`. Run `cargo check --locked` and update `Cargo.lock` if Cargo requires it.
2. Move the version's entries from `Unreleased` into a dated changelog section. Keep an empty `Unreleased` section for future changes.
3. Run the checks in [CONTRIBUTING.md](../CONTRIBUTING.md), then run `cargo package --locked` and `cargo publish --dry-run --locked`. Inspect `cargo package --list` to confirm that the native model, license, and notice are included.
4. Land the reviewed changes and confirm that CI passes on Linux, macOS, and Windows.
5. Create and push an annotated tag that matches the Cargo version, such as `v0.0.1`, at that commit. Creating a tag does not publish a release.

## Build the archives

1. From the default branch's Actions page, manually run **Prepare release** with the existing tag as its `tag` input. The workflow validates the version and resolves the tag to one commit before building every target.
2. Confirm that all build jobs pass. Each job tests the Rust crate, builds an archive, extracts it, and checks the extracted executable's version and synthetic redaction output.
3. Review the resulting draft GitHub Release. It contains five platform archives and `SHA256SUMS`. The workflow refuses to replace an existing release. Remove an obsolete draft manually before rerunning the whole workflow.
4. Download an archive and `SHA256SUMS`. Verify the checksum, extract the archive, and run its executable with `--version` and synthetic stdin on the matching platform. The standalone updater depends on GitHub's SHA-256 asset digest and the archive's existing name and layout.
5. Replace the placeholder draft notes with the finalized changelog for this version. Describe detector coverage and any platform restrictions.

Linux binaries target Ubuntu 24.04's glibc, not musl. Both macOS architectures build on macOS 15. Windows builds use MSVC. Older operating systems require separate compatibility verification before support is claimed.

## Publish the package and release

Perform these steps only after the owner explicitly authorizes publication.

1. From a clean checkout of the tagged commit, run `cargo publish --locked` with your authenticated crates.io account. Cargo publication is manual. No crates.io credential is stored in GitHub Actions.
2. In a temporary directory, run `cargo install redakt --version <version> --locked --root <temporary-install-directory>`. Run the installed executable with `--version` and synthetic stdin to verify the registry package.
3. Publish the reviewed GitHub draft release. Confirm that its tag, archives, checksums, and notes match the version in `Cargo.toml` and `CHANGELOG.md`.
4. Update the README installation introduction to identify the published version instead of stating that publication is pending. Preserve the supported platform restrictions.
5. Verify that `Unreleased` follows the repository convention and that the published notes match the finalized changelog.
