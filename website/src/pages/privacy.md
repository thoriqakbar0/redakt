---
layout: ../layouts/Page.astro
title: Privacy and data handling
description: How the website demo and CLI handle your data.
---
# Privacy and data handling

The homepage terminal shows synthetic examples. It lets you switch between text and JSONL examples and copy commands or prompts, and it has no upload form. Cloudflare serves the website and processes the requests needed to deliver its pages and assets. External links lead to services with their own privacy policies.

After installation, the Rust CLI processes file contents on your machine without an internet connection. It reads the input you provide and writes the result to standard output. When saving a copy, use a separate path and restrict access to it. The detector can miss personal data outside supported formats and context cues, so review the output locally before sharing it with another person or service.

When stderr is attached to a terminal, the CLI can request release metadata from GitHub after redaction, at most once every 24 hours per local cache. The request contains no document contents. Set `REDAKT_UPDATE_NOTIFY=0` to disable these checks.
