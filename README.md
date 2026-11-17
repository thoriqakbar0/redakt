# privacy-hygiene

An experiment in tiny, training-free PII detection and redaction, compared against [OpenAI Privacy Filter](https://huggingface.co/openai/privacy-filter).

The experiment asks how far explicit patterns, checksums, and contextual rules can go without learned weights. It does not assume accuracy equivalent to Privacy Filter or complete anonymization.

## Status

Project initialized. Detector implementation and measured comparison are pending.

The comparison will use synthetic text with labeled sensitive spans. It will report structured identifiers separately from contextual entities, count missed PII and unnecessary redactions, and record artifact sizes and runtime conditions.
