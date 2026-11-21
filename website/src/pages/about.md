---
layout: ../layouts/Page.astro
title: About privacy-hygiene
description: What the privacy-hygiene CLI detects, how it runs, and where to find its source.
---
# About privacy-hygiene

privacy-hygiene is an open-source command-line tool for preparing logs and text before sending them to a language model, a support ticket, or another person. It runs offline after installation. Its Rust binary contains the detection model, so it can process a local file without sending the contents to a hosted service.

The CLI replaces detected values with numbered labels. Repeat mentions of the same value and category share a label within a document. Use plain text to review a redacted copy, or JSONL to process separate documents while retaining match offsets. Detection covers selected formats and context cues, so review the output for any remaining sensitive information.

The project is licensed under Apache 2.0. See the [public repository](https://github.com/thoriqakbar0/privacy-hygiene) for source, detection coverage, release notes, and contribution instructions.
