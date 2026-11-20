---
name: privacy-hygiene
description: Redact personal data and credentials locally from plain-text files and support logs. Use when the user asks to scrub a file or prepare a redacted copy for sharing.
---

# Redact text with privacy-hygiene

1. Identify the UTF-8 source file and the requested output. Use a fresh destination for the redacted copy. Leave the original intact.
2. Use a local `privacy-hygiene` CLI at version 0.0.3 or later. Check with `privacy-hygiene --version`. If needed, install it with `cargo install privacy-hygiene --locked` or build it from the privacy-hygiene repo with `cargo build --release --locked`. Set `binary` to the executable name or path you verified.
3. Set `source` and `redacted` to the chosen paths. Feed the source directly to the CLI. Keep file contents out of shell arguments, tool output, and external services.

   ```bash
   (set -C
    umask 077
    "$binary" < "$source" > "$redacted")
   ```

4. Return the output path only after a successful exit. Report errors without quoting source text.
5. Ask the user to review the draft locally before sharing. The detector can miss names without labels, private URLs, and addresses outside its supported formats. Keep the draft out of chat by default. Send a reviewed copy only with the user's authorization.
