---
layout: ../layouts/Page.astro
title: Privacy and data handling
description: How the website demo and CLI handle your data.
---
# Privacy and data handling

The website demo shows synthetic examples and accepts no file uploads. Cloudflare handles the requests needed to serve the website. External sites have their own privacy policies.

The CLI reads and redacts your text on your machine, even offline. If you save the output, use a new file and restrict access to your account. Review the copy locally for anything the detector missed before you share it.

Run redakt before you send a prompt or attachment. Any text you submit is handled under the AI provider's policies, and redakt cannot recall text you already sent.

After a successful redaction with stderr attached to a terminal, redakt can request release metadata from GitHub at most once every 24 hours per local cache. The request contains no document contents. Set `REDAKT_UPDATE_NOTIFY=0` to disable automatic checks.
