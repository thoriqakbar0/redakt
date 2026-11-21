---
layout: ../layouts/Page.astro
title: About redakt
description: What the redakt CLI detects, how it runs, and where to find its source.
---
# About redakt

redakt helps you prepare a copy of a log or document for sharing. Its built-in model runs on your CPU, so you can redact files offline after installation.

Redakt uses rules to find values such as email addresses, payment card numbers, and labeled passwords. It replaces matches with numbered labels like `[EMAIL_1]`. Repeated matches for the same value and category share a label within a document. Use plain text for a single file or JSONL for multiple documents and match offsets. Review the output for missed or unintended redactions.

## Redact personal data in prose

Redakt 0.0.6 and later include a trained model that looks for names and addresses in English prose. The model runs locally alongside the rules in both plain text and JSONL.

```sh
cargo install redakt --locked
```

Run `redakt update --check` to look for a newer GitHub release. To upgrade a Cargo installation, rerun the install command. See the [update guide](https://github.com/thoriqakbar0/redakt#update-redakt) for standalone binaries.

Read [how the detectors work](https://github.com/thoriqakbar0/redakt#how-the-detectors-work) for coverage, training, and evaluation.

The project is licensed under Apache 2.0. See the [public repository](https://github.com/thoriqakbar0/redakt) for source, detection coverage, release notes, and contribution instructions.
