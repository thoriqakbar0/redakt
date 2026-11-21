# OpenAI-compatible text filter

The example masks supported OpenAI-style requests and restores the completed
reply for local output. It uses the same CLI recovery mapping across turns.

## Driving it with shell

Complete Launch and Doctor. Run the recording loopback provider with
the release binary. The test creates its own scratch mapping and closes its
HTTP server on exit. It needs Python 3 and no API credentials.

```bash
REDAKT_BIN="$binary" python3 tests/openai_filter.py >"$evidence/openai-filter.log" 2>&1
```

The mock checks raw requests for synthetic secrets across replayed history,
file text, tool results, and a byte-identical retry. It splits a placeholder
over multiple Chat Completions stream events and checks that a truncated
stream prints no restored text. It also exercises a non-streamed Responses
reply and verifies that unsupported request fields never reach the server.

This journey exercises the supported text shapes against a local mock.
Responses streaming and live vendor-specific behavior require separate checks.
