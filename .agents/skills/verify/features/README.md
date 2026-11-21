# Verification feature map

Use [verify](../SKILL.md) to launch, check readiness, capture evidence, and clean up.

| Feature | Journey |
| --- | --- |
| Plain text stdin redaction | [stdin.md](stdin.md) |
| Encrypted masking and restoration | [recovery.md](recovery.md) |
| OpenAI-compatible text filter | [openai-filter.md](openai-filter.md) |
| JSONL streaming, Unicode offsets, and private errors | [jsonl.md](jsonl.md) |
| Alternate native weights | [weights.md](weights.md) |
| Intel and Apple silicon compilation | [macos-build.md](macos-build.md) |

Read all journeys affected by a change. A skipped journey is unverified. Keep this map aligned with `src/main.rs`, `src/recovery.rs`, the OpenAI filter example, and the build documentation through `/maintain-verification-skill`.
