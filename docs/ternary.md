# Compiled recurrent PII matcher

The native executable implements the same seven categories as the independent Python rules detector preserved in the reproduction source. It uses an ordered recurrent rule machine with ternary gates. It is not a learned neural detector or a dense tensor network, and adds no contextual understanding beyond those rules.

## Architecture

The former model used fixed 19-character windows for SSNs and phones. That shape cannot express unbounded email lengths or the repetitions and captures used by names and addresses. Version 2 instead stores packed character-class masks, an ordered instruction graph, seven category entries, capture metadata, Unicode classification ranges, and sparse weighted checksum transitions.

Each character gate compares a ternary mask weight against a threshold. Branch priority, input positions, capture tags, and search cursors are host bookkeeping. Negative assertion polarity and boundary differences also use ternary threshold decisions. Luhn and mod-97 state transitions are compiled into sparse weighted tables, rather than calling runtime arithmetic validators. Inference makes no regex-engine calls.

The graph uses accept, character, ordered split, capture, assertion, and anchor instructions. An explicit backtracking stack handles variable-length repetition without one call frame per character. Compiler-derived required masks skip categories when a necessary character class is absent. This retains the original patterns' potential pathological search costs. There is no tested worst-case time or memory guarantee for hostile input.

## Matching contract

Each category searches independently. Ordered branches preserve greedy and lazy regex choices rather than selecting a global longest match. Names and addresses return their value capture, but advance the cursor beyond the full match including its cue.

Card and IBAN checksum validation happens after a syntax match commits. A rejected checksum does not retry a shorter candidate or search inside the rejected match. This preserves original failures such as a greedy IBAN candidate consuming an uppercase suffix and then failing its checksum or length.

Offsets are half-open Unicode code-point positions in the original UTF-8 input. Redaction validates every span, masks their union with `█`, preserves character count, and leaves unrelated CRLF and lone CR characters unchanged.

The artifact uses Python 3.10's Unicode 13.0.0 alphanumeric and whitespace tables, with underscore included in word characters. Rust uses these embedded ranges rather than its own Unicode classification. The end anchor also accepts the position before a final newline, following the original regex policy.

An independent rules detector running under newer Python Unicode tables can classify newly added characters differently. Recompile with that interpreter when comparing such inputs. Classification agreement was checked for every character in the recorded benchmark corpus.

## Native format

All header integers use little-endian order. The artifact starts with four bytes `PHYT`, a u16 version of 2, and a u32 JSON payload length. The length must exactly match the remaining bytes. Version 1 artifacts are rejected and need recompilation.

The JSON object has these fields.

| Field | Content |
| --- | --- |
| `unicode_version` | Classification-table version |
| `word_ranges`, `space_ranges` | Sorted inclusive non-ASCII scalar ranges |
| `masks`, `word_mask` | Packed ternary character masks and boundary-mask index |
| `instructions` | Ordered four-integer matching instructions |
| `heads` | Category labels, entry indices, capture/checksum mode, required masks |
| `luhn`, `iban` | Sparse transition rows and packed acceptance vectors |
| `countries` | Country-character mask indices and required compact lengths |

Five base-three digits fit in each packed byte, least significant digit first. Digits 0 and 1 represent their weights; digit 2 represents -1. Packed values above 242 and nonzero padding are invalid. The loader validates dimensions, indices, Unicode ranges, weights, head metadata, zero-width cycles, recursive assertions, and assertion nesting before execution. The fixed limits include 2,000,000 artifact bytes, 10,000 instructions, 512 masks, and 64 assertion levels.

The current artifact contains 78 character masks, 468 instructions, 703 alphanumeric ranges, and eight whitespace ranges. It is 57,721 bytes. Its SHA-256 is `80eb9d586204777b508b3cfe991f30fc8ab620acc990765084161d8f5ba9d247`. The earlier 876-byte weight figure described the superseded two-category circuit.

Loading expands masks and creates runtime structures. Input text, character codes, backtracking state, spans, and redaction also consume RAM. The measured executable is 830,384 bytes with the model embedded. Neither figure is a RAM estimate. See [the matched measurements](fair-benchmark.md).

## Compiler and reproduction source

The default tree remains Rust-only. The compiler, Python reference, original rules, source fixtures, and complete benchmark reproduction are retained in the separate reproduction source linked from [the benchmark guide](fair-benchmark.md). That compiler parses regex definitions at compilation time and emits the checked-in native format; it is not needed by Cargo or native inference.
