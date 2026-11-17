# Changelog

## Unreleased

- Group the optional compiler, runtime, and packed weights under `ternary/`; run inference with `python -m ternary` and compilation with `python -m ternary.compile`.
- Add an optional, training-free ternary compiler and numerical SSN/phone detector with packed weights, Unicode spans, and reproducible parity tests.
- Add a dependency-free, training-free PII rules detector and character-preserving redaction CLI.
- Add 64 labeled synthetic scenarios, span and character-level scoring, behavioral tests, and a pinned official Privacy Filter CPU comparison.
- Record accuracy, runtime, artifact sizes, and contextual and international coverage limits.
- Optimize rule scanning and masking with unchanged measured outputs; add an optional grouped-expert CPU adapter and reproducible performance measurements.
- Group experiment tooling and optional dependencies under `experiments/`, with the detailed report under `docs/`.
