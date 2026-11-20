# Independent detection-quality evaluation

Run the hand-labelled synthetic corpus through the JSONL CLI.

```bash
cargo test --locked --test accuracy evaluate_accuracy -- --ignored --nocapture
```

The report lists every missed and extra span by document ID, followed by true positives (TP), false positives (FP), false negatives (FN), precision, and recall for each category and for all categories together. The command succeeds when the evaluation completes correctly. It does not require perfect detection or enforce an accuracy threshold. Ordinary `cargo test --locked` validates the annotations and scoring independently but skips this report.

## Corpus and annotation policy

[accuracy.jsonl](../tests/fixtures/accuracy.jsonl) contains 45 synthetic documents with 33 private-value annotations. The annotations were written by hand from the intended meaning of the text, without copying detector predictions or the frozen rules oracle. Each annotation names a literal value and a category. The harness requires one occurrence of that value in its document, then converts its byte position to Unicode code-point offsets. Empty, ambiguous, duplicate, and unknown-category annotations are rejected.

The policy treats contact details, identity and financial identifiers, people, home addresses, credentials, birth dates, and a URL explicitly described as a private profile as private. Public documentation URLs, release dates, product names, and explicitly public product identifiers should remain visible. A checksum-valid number can therefore be a false positive under this policy. This intentionally tests contextual privacy decisions that the current format-based rules cannot always make. Invalid-checksum strings explicitly described as invalid examples are benign under this policy. All examples are synthetic, including identifiers that pass checksums.

The corpus includes supported formats, benign prose, invalid checksums, Unicode and CRLF offsets, mixed fields, and known coverage gaps. Its category counts and examples are deliberately selected, not sampled from real traffic. It is neither a held-out statistical benchmark nor evidence of general precision or recall. Use the individual failures to select the next detection improvement, and add examples from the intended domain before relying on the detector there.

## Scoring

A true positive requires an exact match of document, label, start, and exclusive end. A prediction with the wrong boundary or label counts as both an extra prediction and a missed annotation. Overlapping spans with different labels remain separate predictions. Duplicate predictions are rejected instead of improving the score.

Precision is TP / (TP + FP), and recall is TP / (TP + FN). An empty denominator is reported as `n/a`. The micro result sums category counts before dividing. It includes unsupported categories such as private URLs so missing capabilities stay visible. It does not measure whether the redaction union happened to cover an incorrectly labelled span. Separately, the harness checks response IDs, counts, labels, bounds, and that redacted text equals the union of returned spans.

## Observed baseline

A deterministic local run on 2026-11-17 produced these counts, before the CLI enabled cue-based recognition by default. The model-only column is retained as a historical baseline. No sampling or statistical inference is involved.

| Category | Model-only TP / FP / FN | With cue-based recognition TP / FP / FN |
| --- | --- | --- |
| Email | 4 / 0 / 0 | 4 / 0 / 0 |
| Phone | 4 / 0 / 0 | 4 / 0 / 0 |
| SSN | 2 / 0 / 0 | 2 / 0 / 0 |
| Card | 2 / 1 / 0 | 2 / 1 / 0 |
| IBAN | 2 / 0 / 0 | 2 / 0 / 0 |
| Name | 1 / 0 / 4 | 4 / 1 / 1 |
| Address | 1 / 0 / 2 | 2 / 0 / 1 |
| Secret | 0 / 0 / 4 | 4 / 0 / 0 |
| Account number | 0 / 0 / 2 | 2 / 0 / 0 |
| Birth date | 0 / 0 / 4 | 3 / 0 / 1 |
| Private URL | 0 / 0 / 1 | 0 / 0 / 1 |
| All categories | 16 / 1 / 17 | 29 / 2 / 4 |

Model-only micro precision was 0.941 and recall was 0.485. With cue-based recognition, precision was 0.935 and recall was 0.879 on this corpus only.

Both runs masked the explicitly public card-shaped product identifier. Cue-based recognition also masked `Maple Editor` in `Product name: Maple Editor` and missed an uncued person, an Indonesian address, a slash-formatted birth date, and the private profile URL. The model-only run additionally missed the lowercase and Unicode cues and supplemental categories listed in the report.

The current CLI finds all four annotated dates, including slash-formatted dates. Its totals are 30 TP, 2 FP, and 3 FN (precision 0.938, recall 0.909) on the same corpus.
