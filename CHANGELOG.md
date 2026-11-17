# Changelog

## Unreleased

- Keep only the standalone Rust detector and native ternary model; remove Python tooling, NPZ weights, rules and baseline experiments, datasets, and recorded results. Replace Python-dependent checks with native Rust CLI tests.

- Add the project logo in SVG and transparent PNG formats and display it in the README.
- Prepare public-source licensing, contributor and security guidance, cross-platform CI, and dependency updates.
- Compare decompressed model payloads across platforms while preserving byte-identical recompilation checks within one environment.
- Add a native Rust executable that embeds and executes the compiled ternary SSN and phone model, with Unicode code-point spans, model validation, and Python parity checks.
- Keep the Rust application in `src/`, compiled artifacts in `models/`, and the Python reference runtime and compiler in `experiments/`; run them with `python -m experiments.ternary` and `python -m experiments.compile_ternary`.
- Document real-world rule failures on public specifications and blog pages, including card false positives and missed names and addresses.
- Add an optional, training-free ternary compiler and numerical SSN/phone detector with packed weights, Unicode spans, and reproducible parity tests.
- Add a dependency-free, training-free PII rules detector and character-preserving redaction CLI.
- Add 64 labeled synthetic scenarios, span and character-level scoring, behavioral tests, and a pinned official Privacy Filter CPU comparison.
- Record accuracy, runtime, artifact sizes, and contextual and international coverage limits.
- Optimize rule scanning and masking with unchanged measured outputs; add an optional grouped-expert CPU adapter and reproducible performance measurements.
- Group experiment tooling and optional dependencies under `experiments/`, with the detailed report under `docs/`.
