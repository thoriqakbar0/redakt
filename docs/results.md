# Experiment results

On this 64-document synthetic fixture, deterministic rules have higher exact precision and structured-identifier recall. Privacy Filter has higher contextual recall and misses fewer sensitive characters. Both inference optimizations preserve all measured spans and redactions.

These results describe one small, hand-authored corpus. They do not establish general accuracy, model equivalence, or complete anonymization. The rules use no weights; Privacy Filter uses its real frozen checkpoint without additional training. Reproduction commands are in the [README](../README.md).

The current seven-category compiled matcher has a separate [matched comparison](fair-benchmark.md). The historical timings below use their original boundaries and must not be mixed with that comparison.

## Accuracy

The official CPU baseline completed five passes without errors or tokenizer warnings. Optimized outputs match the original outputs on every fixture document.

| System | Slice | Exact precision | Exact recall |
| --- | --- | ---: | ---: |
| Rules | Structured | 100.0% | 79.3% |
| Rules | Contextual | 90.9% | 47.6% |
| Rules | All | 89.2% | 66.0% |
| Privacy Filter | Structured | 76.9% | 69.0% |
| Privacy Filter | Contextual | 65.2% | 71.4% |
| Privacy Filter | All | 64.8% | 70.0% |

Typed exact precision and recall equal these figures in every reported slice under the [label mapping](#metric-definitions).

| System | Missed sensitive characters | Unnecessary redacted characters | Exact-boundary documents |
| --- | ---: | ---: | ---: |
| Rules | 256/786, 32.6% | 47/1,088, 4.3% | 44/64 |
| Privacy Filter | 97/786, 12.3% | 100/1,088, 9.2% | 43/64 |

Many baseline false positives occur on invalid identifiers that this fixture labels negative. Those errors reflect the fixture's policy; they do not prove that the model's native policy is wrong. Per-label support, complete boundaries, and masks are retained in the [score reports](#evidence).

## Inference optimization

### Rules

Interval slicing replaces per-character sets, required-marker checks skip impossible regex families, and redundant span deduplication is removed. The regex policy stays unchanged. Source size grows from 3,400 to 4,091 bytes, with zero weights or third-party dependencies.

Seven alternating trials measured warm detection plus redaction. Each short-fixture sample averages 100 complete corpus passes; larger samples average two passes. Ranges show the minimum and maximum samples. Profiling is excluded from timing.

| Workload | Before median, range | After median, range | Measured ratio |
| --- | ---: | ---: | ---: |
| 64 documents | 2.269 ms, 1.937 to 2.723 | 1.256 ms, 1.173 to 1.882 | 1.81x |
| 193,700 characters, mixed | 155.743 ms, 142.382 to 174.330 | 101.350 ms, 91.404 to 107.623 | 1.54x |
| 275,000 characters, clean | 96.835 ms, 91.116 to 111.309 | 4.123 ms, 3.496 to 5.181 | 23.49x |

All three API timing ranges separate in the recorded run. Fresh-process CLI ranges overlap for the fixture and mixed document, so those changes remain inconclusive. The clean-document CLI ranges separate. CLI timings include startup, imports, stdin, JSON output, and subprocess transport. Keeping the detector imported avoids those per-request costs. Regex scanning remains a main cost after optimization.

Differential checks preserve spans and masks on the fixture, both large workloads, 256 randomized text concatenations, and 1,980 overlapping or unsorted interval cases. No output cache is used. Samples, source hashes, and profiles are in [rules-optimization.json](../results/rules-optimization.json).

### Privacy Filter

The original profile attributes 52.6% of self CPU time to copies and 26.9% to expert indexing. Its CPU fallback gathers expert matrices per token and repeatedly converts them. The adapter caches expert tensors as FP32 once and groups rows by expert, avoiding those repeated full-matrix gathers and conversions.

The adapter preserves top-k routing, slot order, activation functions, gating scaling, decoder calibration, and bfloat16 attention, embedding, and residual dtypes. It changes no thresholds, prunes no experts, and exports no weights. The upstream checkout stays unchanged. This alternate implementation depends on private internals of the pinned OPF version.

| Workload | Original median, range | Grouped median, range |
| --- | ---: | ---: |
| Four probes, five rotated trials | 8.542 s, 8.501 to 9.424 | 1.467 s, 1.331 to 1.561 |
| Full corpus, five passes | 141.101 s, 135.600 to 153.572 | 24.942 s, 21.258 to 30.892 |

The rotated probe test measures a 5.82x improvement with nonoverlapping ranges and unchanged predictions. Full-corpus runs were not interleaved, so the probe test is stronger evidence for attributing the gain. All 64 span lists and masks remain unchanged, and repeated grouped predictions are identical. Small-block numerical tests use a tolerance; these checks do not establish exact logit equality.

The optimized profile shifts the main cost to matrix multiplication, with copies at 3.3% of self CPU time. Sweeps covering 1, 2, 4, 8, 16, and 32 compute threads retain four threads. Higher-thread changes fail the nonoverlapping-range acceptance gate; 32 threads is substantially slower. Preliminary whole-model FP32 tests were superseded by the targeted cache strategy. Their interrupted log carries no final performance claim.

### Memory tradeoff

Cached FP32 experts increase parameter storage from 2,798,995,648 to 5,319,510,208 bytes. The on-disk checkpoint stays unchanged. The adapter itself is 2,697 source bytes and still requires the trained checkpoint and PyTorch.

A separate grouped CLI run over all 64 documents records peak RSS of 5,964,524 KiB and load plus conversion time of 23.195 seconds. The original process peak RSS is 3,914,160 KiB. These are single-run observations, not distributions. The comparison benchmark's peak RSS includes both resident models and must not be treated as a standalone footprint.

GPU inference and batched-document throughput were not evaluated. Further batching needs separate boundary and numerical checks. These measurements do not establish a globally optimal configuration or production throughput.

## Artifact size

Sizes are uncompressed logical file bytes. Runtime-inclusive totals describe the measured installations, not standalone executables or minimal bundles.

| System | Program and checkpoint excluding runtime, bytes | Including runtime estimate, bytes |
| --- | ---: | ---: |
| Rules | 4,091 | 80,399,846 |
| Official Privacy Filter | 2,799,296,829 | 3,806,114,507 |
| Grouped Privacy Filter | 2,799,299,526 | 3,806,117,204 |

The official checkpoint contains 2,798,989,275 bytes, including 2,798,984,088 weight bytes. The table also counts upstream package source and, for grouped inference, the adapter.

The runtime estimate includes the resolved CPython executable, standard library excluding site-packages and bytecode, and installed libpython, even if the interpreter is statically linked. Model totals also include the installed environment's library tree, including bytecode. Exclusions are OS libraries, external caches, uv, Git, evaluation scripts, and corpus. [size.json](../results/size.json) records components and `ldd` output for uncounted system libraries. Disk size, parameter storage, and peak RSS are separate measurements.

## Corpus and labeling policy

[data/synthetic.jsonl](../data/synthetic.jsonl) contains 27 structured-identifier documents, 21 contextual name or address documents, and 16 negative documents. It has 50 gold spans, 786 sensitive characters, and 1,088 nonsensitive characters. Its SHA-256 is `9b6ddec0bad424e3cbe8917495d4ef2f54bfd21104b38da43311dc193ffc91a9`.

Scenarios use fabricated contexts, example names and addresses, test domains, payment test numbers, and public example identifiers. One SSN is a historical published example. No private records were collected. Synthetic strings can coincide with real identifiers and must not be used as valid personal records.

Gold spans include whole values, internal spaces and punctuation, and apartment or locality components where supplied. They exclude field cues and sentence-ending punctuation. Surrounding parentheses are excluded unless part of a phone number. International formats remain sensitive even when the rules cannot detect them.

Negative controls include invalid checksums, invalid SSN forms, a declared public support mailbox, an organization, a song title, and a catalog code. Privacy Filter may apply a broader policy to these strings. There are no gold dates, URLs, or secrets, so their recall cannot be assessed.

Rules and fixtures were built for this experiment. Detection policy was frozen before inference optimization, which preserved outputs. The corpus is not a random sample, independent benchmark, held-out real-world dataset, or demographic assessment.

## Metric definitions

Both systems use the same evaluator, not Privacy Filter's native containment-based scorer. Inputs require matching text and IDs, valid integer offsets, nonempty labels, unique spans, and in-bounds positions. Offsets are half-open Unicode code-point positions. Tokenizer round-trip warnings abort the model runner.

| Metric | Definition |
| --- | --- |
| Exact precision | Identical gold and predicted boundary pairs divided by predicted pairs |
| Exact recall | Identical boundary pairs divided by gold pairs |
| Typed exact precision and recall | The same calculations with a matching label also required |
| Missed sensitive characters | Gold-covered positions outside the prediction union; rate divides by sensitive positions |
| Unnecessary redaction | Predicted positions outside the gold union; rate divides by nonsensitive positions |

Partial spans earn no exact match. Overlaps are unioned only for character coverage and masking; boundary and typed sets remain separate. Character counts include whitespace, not bytes or graphemes. Undefined ratios are JSON `null`. Each slice counts every prediction in its documents, including wrong-category detections.

For typed scoring, SSN, card, and IBAN map to `account_number`. Model labels `private_email`, `private_phone`, `private_person`, and `private_address` map to email, phone, name, and address. Other model labels remain unchanged. This mapping loses identifier subtypes and does not establish taxonomy equivalence.

## Boundary behavior

The rules preserve punctuation around `(a+b@example.test),` and code-point offsets after an emoji. They completely miss the fixture's international phone number, phone and SSN without separators, lowercase IBAN, international or quoted email local parts, and several contextual names and addresses.

For `Address: 42 Elm Street, Apt 5.`, prediction `[9,22)` misses the apartment suffix in gold `[9,29)`. That gives zero exact matches, seven exposed sensitive characters, and no unnecessary redaction. `Patient: John O'Neil.` is missed entirely. `Name: Maple Street is a song title.` wrongly triggers name masking.

Additional [boundary probes](../results/boundary-probes.json) show that appending ` USD` makes the IBAN candidate greedily consume extra text and fail validation. The otherwise valid IBAN is missed. These probes sit outside the scored corpus and do not alter its scores. The reports retain all gold spans, predictions, and masks for inspection.

## Real-world document failures

A spot check on 2026-11-17 ran the rule detector on two public specifications, a blog post, and its author's about page. This check evaluated the rules only; Privacy Filter and the ternary backend were not run on these pages. These documents have no complete gold annotations, so this check establishes individual failures rather than precision or recall. It does not change the synthetic scores above.

- In [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110.txt), the detector incorrectly classified `1234567890987` in a `Content-Length` header example as a card. The number passes Luhn validation but represents a body length. Six email occurrences were also detected.
- In the extracted text of [this coding blog post](https://simonwillison.net/2025/Mar/11/using-llms-for-code/) and the [author's about page](https://simonwillison.net/about/), the archive-year sequences `2002 2003 2004 2005` and `2010 2011 2012 2013` were incorrectly classified as cards. Both sequences pass Luhn validation after spaces are removed. The about page's contact email was detected, but the author's name was not.
- In [RFC 5322](https://www.rfc-editor.org/rfc/rfc5322.txt), the detector found 63 email occurrences and one phone number. Many email matches are illustrative examples, so these counts do not establish successful identification of private information.
- RFC 9110's author name `Roy T. Fielding` and postal address beginning `345 Park Ave` were missed. The name and address rules require explicit field cues that these author details lack.

Specifications were processed as UTF-8 plain text. HTML was parsed with Beautiful Soup; script, style, nav, and footer elements were removed, then remaining text nodes were joined with spaces. Archive links outside those elements remained. The year-list failures therefore concern extracted page text including site navigation, rather than the article body alone. Redaction preserved character length and masked every predicted span on all four inputs, but those checks do not establish detection accuracy.

These failures show that checksum validation cannot distinguish a card from unrelated numbers, and cue-based rules miss personal details in ordinary prose. The detector remains unsuitable for general anonymization without further validation and review.

## Runtime and verification

The measured environment is Linux x86-64 on a VMware VM, with an Intel Xeon E5-2697 v2 at 2.70 GHz, 32 visible CPUs, and CPython 3.12.16. No GPU is used. The baseline has four PyTorch compute threads, one interop thread, batch size one, a 512-token context limit, whitespace trimming, and Viterbi decoding with the artifact's default zero biases. All fixture documents fit the limit; long-context behavior is untested. Rules use one Python process.

| Component | Pinned revision |
| --- | --- |
| Official OPF source | `f7f00ca7fb869683eb732c010299d901457f19c3` |
| Hugging Face checkpoint | `7ffa9a043d54d1be65afb281eddf0ffbe629385b` |

The initial rules and model timings have different scopes and were not interleaved. They do not support a rules-versus-model speedup ratio. Warm inference measurements exclude network downloads, imports, and file I/O; CLI measurements include startup and transport. Profiling is excluded from timed trials. The shared VM has varying load, recorded with the environment artifacts.

Five dependency-free behavioral tests cover detection, checksum rejection, union masking, shifted boundaries, wrong labels, external input validation, Unicode offsets, fixture bounds, and the stdin CLI. Two optional Torch tests cover grouped arithmetic and unsupported modes. Codehygiene passes all created Python files. The checkpoint hash and original versus optimized outputs were verified.

No model was trained, published, or deployed. The prototype offers no complete anonymization, broad language coverage, adversarial robustness, or streaming guarantee.

## Evidence

Recorded artifacts retain their measurement-time contents. In particular, `source-manifest.json` records paths from before the directory cleanup; current experiment modules live under `experiments/`.

| Artifact | Contents |
| --- | --- |
| [rules-optimized.json](../results/rules-optimized.json) | Current rules scores and every document's spans and mask |
| [privacy-filter-scores.json](../results/privacy-filter-scores.json) | Official baseline scores, spans, and masks |
| [privacy-filter-optimized-scores.json](../results/privacy-filter-optimized-scores.json) | Grouped baseline scores, spans, and masks |
| [rules-optimization.json](../results/rules-optimization.json) | Seven-trial timing samples, differential checks, and profiles |
| [baseline-optimization.json](../results/baseline-optimization.json) | Rotated model trials, full-corpus checks, and parameter sizes |
| [baseline-thread-sweep.json](../results/baseline-thread-sweep.json) | Higher-thread timing samples and selection |
| [privacy-filter-grouped-cli.runtime.json](../results/privacy-filter-grouped-cli.runtime.json) | Standalone grouped CLI runtime and memory observation |
| [size.json](../results/size.json) | File-size components and exclusions |
| [source-manifest.json](../results/source-manifest.json) | Source and corpus hashes |
| [checkpoint-sha256.txt](../results/checkpoint-sha256.txt) | Downloaded weight checksum |
| [baseline-environment.txt](../results/baseline-environment.txt) | Installed dependency versions |
| [cpu.txt](../results/cpu.txt), [memory.txt](../results/memory.txt), [load.txt](../results/load.txt) | Machine and load snapshots |
