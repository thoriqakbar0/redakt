# Compiled ternary SSN and phone circuits

The Rust executable and optional Python backend implement a small weight-space program inspired by [codehygiene's compiler](https://github.com/brianbell-x/codehygiene/blob/main/compile.py). It compiles explicit rules into numerical tensors without training data, gradient descent, or model downloads. The implementation is independent of codehygiene's source.

This backend covers two of the seven rule families. It detects hyphenated US SSNs and separated North American phone numbers. It does not detect email, payment cards, IBANs, names, or addresses. It does not reproduce OpenAI Privacy Filter or provide learned contextual generalization. Use it as a circuit experiment, not a complete anonymization tool.

## Compile and run

Run these commands from the repository root with Python 3.10 or later. NumPy is required only for compilation and Python inference. The Rust executable embeds `models/weights.bin` and runs without Python or NumPy.

```bash
uv run --extra ternary python -m experiments.compile_ternary --native-output models/weights.bin
cargo build --release --locked
printf '123-45-6789' | target/release/privacy-hygiene
printf 'SSN 123-45-6789; phone +1 (212) 555-0123' | uv run --extra ternary python -m experiments.ternary
uv run --extra ternary python -m unittest discover -s tests -v
```

The CLI emits the same `spans` and `redacted` fields as the rules detector. Offsets count Python Unicode characters, not UTF-8 bytes. Imported inference caches the default model across calls.

```python
from experiments.ternary import detect
from detector import redact

text = "Call +1 (212) 555-0123."
spans = detect(text)
masked = redact(text, spans)
```

`python -m experiments.compile_ternary --output /tmp/weights.npz` builds another archive. `python -m experiments.ternary --weights /tmp/weights.npz` loads that archive for stdin inference. The archive is a compiler artifact, not a general interchange format for arbitrary models.

## Numerical architecture

The compiler emits three multidimensional weight tensors. Every entry is in `{-1, 0, +1}`.

| Tensor | Shape | Purpose |
| --- | --- | --- |
| Character embedding | 130 × 12 | ASCII character classes and Unicode word/nonword categories |
| Window kernels | 12 × 19 × 12 | Seven syntax heads and five SSN exclusion gates |
| Readout | 7 × 12 | Accept syntax heads and subtract SSN veto gates |

The seven syntax heads encode one SSN layout and six phone layout families. Separator character classes combine space, dot, and hyphen alternatives. The phone heads cover bare or parenthesized area codes, with no country prefix, `+1`, or `+1` followed by one separator. Area and exchange codes begin with digits 2 through 9.

At each possible start position, inference multiplies a character-feature window by the compiled kernels. Integer thresholds turn scores into Boolean gates. The readout accepts syntax gates and subtracts SSN exclusions for `000`, `666`, and `9xx` area groups, `00` middle groups, and `0000` final groups. Negative kernel weights enforce Unicode word boundaries; SSN boundaries also reject adjacent hyphens. These decisions use numerical gates rather than regex matching. Python encodes character categories and selects leftmost, longest, nonoverlapping matches separately for each label.

The Python runtime reuses the original `Span` class and redaction function. It does not call the rules detector for inference. It processes at most 4,096 start positions per numerical batch, with neighboring characters included across batch boundaries. Input text, returned spans, and candidate bookkeeping still require memory proportional to the input.

The tensors contain 4,380 trits. Packing five base-three digits per byte uses 876 bytes before archive overhead and compression. The checked-in `models/weights.npz` is 2,015 bytes, including shapes, format version, one-dimensional thresholds, lengths, and labels. This size excludes Python, NumPy, source code, and interpreter memory. It is not an end-to-end deployment size.

## Reproduce the evaluation

```bash
uv run --extra ternary python -m experiments.ternary --corpus data/synthetic.jsonl > /tmp/ternary-predictions.jsonl
python3 -m experiments.evaluate --predictions /tmp/ternary-predictions.jsonl --output /tmp/ternary-scores.json
```

On the existing 64-document synthetic corpus, this backend produces eight spans, all eight exact typed matches. Overall span recall is 8/50, or 16%, because most labeled spans are outside its scope. It misses 688 of 786 sensitive characters and adds no unnecessary redaction on this fixture. These are synthetic fixture results, not an estimate of production accuracy. The existing rule and Privacy Filter measurements remain unchanged. The evaluator's `artifact` section still describes the original rules detector even with external predictions; use the tensor and archive sizes above for this backend.

The tests also compare supported-label spans against the original detector on the full corpus and 250 deterministic generated inputs. Independent examples cover invalid SSNs, Unicode boundaries and offsets, phone layouts, empty input, and matches crossing numerical batch boundaries. Artifact tests check ternary membership, packing round trips, byte-identical recompilation, and execution from a freshly compiled archive.

The native runtime precomputes nonzero kernel terms and executes the same integer embedding, threshold, and readout operations. It processes character positions directly, with neighboring characters and nonword tail padding. It uses the same per-label leftmost-longest selection. The public Rust `redact` function validates every supplied span before masking their union.

No speed advantage is claimed. Dense integer kernels can cost more than the original regex rules, and NumPy adds a runtime dependency. Variable-length patterns and checksums need additional circuits before this approach can cover the other rule families.

## Native artifact format

`compile_weights(path, native_output=path)` writes both artifacts from the same `compile_tensors()` result. The native path is explicit. Compiling a temporary NPZ alone never changes the checked-in native file. `--weights PATH` accepts the same format as the embedded model.

All integers use little-endian order. The fixed circuit format contains these fields in order.

| Field | Encoding |
| --- | --- |
| Magic and version | Four bytes `PHYT`, then u16 version 1 |
| Dimensions | Six u16 values, embedding rows and columns, kernel count and width, readout rows and columns |
| Unicode version | u8 byte length and ASCII version string |
| Alphanumeric ranges | u32 count, then inclusive non-ASCII start and end pairs of u32 values |
| Embedding, kernels, readout | Row-major flattened tensors concatenated and packed into 876 bytes |
| Thresholds | Twelve signed i16 values |
| Head lengths | Seven positive i16 values |
| Labels | Seven bytes, 0 for SSN and 1 for phone |

Each packed byte stores five base-three digits, least significant digit first. A digit of 2 represents -1. The loader rejects bytes above 242, nonzero padding, unsupported dimensions, invalid ranges, invalid lengths or labels, truncation, and trailing data. Custom weights still cannot return spans beyond the input. This format supports the fixed circuit, not arbitrary network architectures.

The checked-in artifact uses Unicode 13.0.0 from Python 3.10. The compiler embeds sorted ranges from Python `str.isalnum()` and records `unicodedata.unidata_version`. Rust uses these ranges rather than Rust's broader `char::is_alphanumeric`, so combining marks and numeric characters follow the compiling Python's boundary behavior. Recompile with the same Python Unicode version for byte-identical artifacts. A newer Python may change classification ranges. Compare Python and native inference with artifacts generated by that same interpreter when evaluating new Unicode characters.

Build the release executable before running the tests to compare it against Python on the corpus and 250 generated inputs. Independent examples exercise all seven syntax heads, SSN vetoes, Unicode offsets and boundaries, CRLF, and the Python batch boundary. Tests also load alternate threshold and readout tensors to prove native predictions come from model execution, and reject malformed models and invalid UTF-8 without partial JSON.
