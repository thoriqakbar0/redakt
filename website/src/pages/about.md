---
layout: ../layouts/Page.astro
title: About redakt
description: What the redakt CLI detects, how it runs, and where to find its source.
---
# About redakt

redakt is an open-source command-line tool for preparing logs and text before sending them to a language model, a support ticket, or another person. Its Rust binary contains the detection model, so it can process a local file without sending the contents to a hosted service.

The CLI replaces detected values with numbered labels. Repeat mentions of the same value and category share a label within a document. Use plain text to review a redacted copy, or JSONL to process separate documents while retaining match offsets. Detection covers selected formats and context cues, so review the output for any remaining sensitive information.

The project is licensed under Apache 2.0. See the [public repository](https://github.com/thoriqakbar0/redakt) for source, detection coverage, release notes, and contribution instructions.
