# Compiled ternary SSN and phone circuits

The standalone Rust executable embeds `models/weights.bin`. It runs fixed numerical circuits without training, regex matching, or model downloads. It covers hyphenated US SSNs and separated North American phone numbers only. It does not provide learned contextual generalization.

Build with `cargo build --release --locked` and pipe UTF-8 text to `target/release/privacy-hygiene`. The CLI emits JSON spans and redacted text. `--weights PATH` accepts another artifact in the same fixed format.

## Numerical architecture

The model contains three multidimensional weight tensors. Every entry is in `{-1, 0, +1}`.

| Tensor | Shape | Purpose |
| --- | --- | --- |
| Character embedding | 130 × 12 | ASCII character classes and Unicode word/nonword categories |
| Window kernels | 12 × 19 × 12 | Seven syntax heads and five SSN exclusion gates |
| Readout | 7 × 12 | Accept syntax heads and subtract SSN veto gates |

The seven syntax heads encode one SSN layout and six phone layout families. Separator character classes combine space, dot, and hyphen alternatives. The phone heads cover bare or parenthesized area codes, with no country prefix, `+1`, or `+1` followed by one separator. Area and exchange codes begin with digits 2 through 9.

At each possible start position, inference multiplies a character-feature window by the compiled kernels. Integer thresholds turn scores into Boolean gates. The readout accepts syntax gates and subtracts SSN exclusions for `000`, `666`, and `9xx` area groups, `00` middle groups, and `0000` final groups. Negative kernel weights enforce Unicode word boundaries; SSN boundaries also reject adjacent hyphens. These decisions use numerical gates rather than regex matching. The runtime encodes character categories and selects leftmost, longest, nonoverlapping matches separately for each label.

The Rust runtime precomputes nonzero kernel terms and executes integer embedding, threshold, and readout operations. The public `redact` function validates supplied spans before masking their union. Input and span storage require memory proportional to input length.

The tensors contain 4,380 trits packed into 876 bytes. Artifact metadata, the executable, and runtime memory are additional. No speed or production-accuracy claim is made.

## Native artifact format

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

The checked-in artifact embeds Unicode 13.0.0 alphanumeric ranges originally generated with Python 3.10. Rust uses those ranges for boundary behavior rather than its own Unicode classification. The compiler and Python reference runtime are available only in Git history; this tree builds from the checked-in binary and does not regenerate it.
