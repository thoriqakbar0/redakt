# Cue-based PII removal

The CLI automatically runs deterministic, cue-based recognition alongside the embedded seven-category detector. Rust callers can use `Detector::detect_contextual` for the combined results or `Detector::detect` for model-only results.

## Examples

```bash
printf 'API_KEY=synthetic_example_key_123456' | privacy-hygiene
printf '%s\n' '{"id":"one","text":"Date of birth: 1990-04-12"}' | privacy-hygiene --jsonl
```

JSONL responses include spans with Unicode code-point offsets into the original text. Redaction replaces overlapping model and cue-based spans with one numbered label. Exact duplicate spans appear once in the response.

`--weights PATH` changes the seven-category model. The supplemental contextual rules are built into the executable and operate independently of that artifact.

## Supplemental coverage

Cues match ASCII letters without regard to case. Values retain their original characters. A cue requires `:`, `=`, or the word `is` before its value. `ship to` and `lives at` also accept a space directly before the address. Keys and values can have matching single or double quotes.

| Output label | Cues | Value grammar |
| --- | --- | --- |
| `name` | `name`, `full name`, `my name`, `patient`, `customer` | One to four words with Unicode letters, apostrophes, or hyphens, up to 100 code points. A small English stop-word list rejects some obvious prose. |
| `address` | `address`, `ship to`, `lives at` | A numeric house number, alphabetic street words, and a supported street suffix, with three to eight whitespace-separated parts. |
| `secret` | `password`, `passwd`, `api_key`, `api key`, `access_token`, `access token`, `client_secret`, `client secret` | A nonempty unquoted token or a quoted passphrase. Periods, colons, and equals signs remain part of the value. |
| `account_number` | `bank account`, `account number` | Six to 34 ASCII digits, optionally separated by spaces, tabs, or hyphens. The first and last characters must be digits. |
| `date` | `dob`, `date of birth`, `birth date` | A valid Gregorian `YYYY-MM-DD`, `DD/MM/YYYY`, or `MM/DD/YYYY` date with a nonzero year. |

Supported street suffixes are `street`, `st`, `road`, `rd`, `avenue`, `ave`, `lane`, `ln`, `drive`, `dr`, `boulevard`, and `blvd`.

Supplemental values stop at line boundaries and have a 256-code-point limit. Unquoted values stop at field punctuation. Unquoted secrets and dates also stop at horizontal whitespace. Quoted secrets can contain spaces and the opposite quote character. Escape sequences and multiline values are unsupported. Oversized or malformed values are rejected rather than partially matched.

The supplemental address rule covers the street portion, not following city, region, or postal fields. Uncued names, private URLs, unlabelled secrets and account numbers, dates with two-digit years, and many international addresses remain unsupported by this pass. The original seven-category model still runs independently, with its existing coverage and limitations.

## Relationship to OpenAI Privacy Filter

[OpenAI Privacy Filter](https://openai.com/index/introducing-openai-privacy-filter/) is a learned, bidirectional token classifier. Its categories include people, addresses, emails, phones, URLs, dates, account numbers, and secrets. Its language model uses surrounding context to make decisions that deterministic cue rules cannot reproduce.

This CLI uses those categories as coverage inspiration. It does not include OpenAI's model or establish equivalent accuracy. The supplemental rules add bounded, explicit formats while preserving the standalone Rust executable. They cannot identify every private reference, distinguish public from private information, or infer arbitrary names in prose.

A local reference check on 2026-11-17 used the official implementation at revision `f7f00ca7fb869683eb732c010299d901457f19c3`, its existing checkpoint, CPU inference, a 512-token context window, and four compute threads. The first attempt terminated with an illegal instruction on the older Xeon CPU. The retry completed with `ATEN_CPU_CAPABILITY=default`, `ONEDNN_MAX_CPU_ISA=SSE41`, and `DNNL_MAX_CPU_ISA=SSE41`.

The model identified private spans in each of these synthetic examples. These examples were selected to expose known missing rules, not to estimate general precision or recall.

| Synthetic input | Gap exposed in the original CLI |
| --- | --- |
| `name: Nora Chen.` | Lowercase name cue |
| `My name is María García.` | Unicode name |
| `My password is orbit-fox-731!` | Labelled secret |
| `API_KEY=synthetic_example_key_123456` | Configuration secret |
| `Date of birth: 1990-04-12` | Private date |
| `Bank account: 123456789012` | Labelled account number |
| `address: 42 Maple Street.` | Lowercase address cue |
| `I met Nora Chen yesterday.` | Uncued name in prose |

The updated CLI masked the expected private value in the seven cued examples. The final, uncued example remains unsupported by its supplemental rules. OpenAI's model also used different address span boundaries. This check is neither an output-parity claim nor a benchmark against the model.

Evaluate representative examples for your language and domain, including examples that should remain visible.
