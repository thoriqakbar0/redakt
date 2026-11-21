import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer


ROOT = Path(__file__).resolve().parents[1]
BINARY = os.environ.get("REDAKT_BIN", str(ROOT / "target/release/redakt"))
TOKEN = re.compile(r"\[REDAKT_[0-9a-f]{32}\]")
EMAIL = "ops@example.test"
PASSWORD = "demo-private-password"


class RecordingProvider(BaseHTTPRequestHandler):
    def log_message(self, message, *arguments):
        pass

    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"]))
        self.server.received.append(body)
        if self.server.retry_once:
            self.server.retry_once = False
            self.send_error(503)
            return

        token = TOKEN.search(body.decode()).group()
        if self.path.endswith("/responses"):
            self.write_json({
                "status": "completed",
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": f"For {token}"}],
                }],
            })
        else:
            self.write_stream(token)

    def write_json(self, payload):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(payload).encode())

    def write_stream(self, token):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        for text in ["For ", *token, " and [REDAKT_unknown]"]:
            event = {"choices": [{"index": 0, "delta": {"content": text}}]}
            self.wfile.write(f"data: {json.dumps(event)}\r\n\r\n".encode())
        if not self.server.truncate_stream:
            self.wfile.write(b"data: [DONE]\r\n\r\n")


class FilterTest(unittest.TestCase):
    def setUp(self):
        self.state = tempfile.TemporaryDirectory()
        self.addCleanup(self.state.cleanup)
        self.provider = HTTPServer(("127.0.0.1", 0), RecordingProvider)
        self.provider.received = []
        self.provider.retry_once = False
        self.provider.truncate_stream = False
        thread = threading.Thread(target=self.provider.serve_forever)
        thread.start()
        self.addCleanup(self.stop_provider, thread)

    def stop_provider(self, thread):
        self.provider.shutdown()
        thread.join()
        self.provider.server_close()

    def exchange(self, request, route="chat/completions"):
        return subprocess.run(
            [
                sys.executable, str(ROOT / "examples/openai_filter.py"),
                "--redakt", BINARY,
                "--recovery", str(Path(self.state.name) / "session.recovery"),
                "--url", f"http://127.0.0.1:{self.provider.server_port}/v1/{route}",
                "--model", "synthetic",
            ],
            input=json.dumps(request).encode(),
            capture_output=True,
            check=False,
            env={key: value for key, value in os.environ.items() if key != "OPENAI_API_KEY"},
        )

    def test_masked_history_and_retry(self):
        self.provider.retry_once = True
        message = {"role": "user", "content": f"Email: {EMAIL}"}
        first = self.exchange({"messages": [message], "stream": True})
        self.assertEqual(first.returncode, 0, first.stderr)
        self.assertEqual(first.stdout.decode(), f"For {EMAIL} and [REDAKT_unknown]")
        first_body, retry_body = self.provider.received
        self.assertEqual(first_body, retry_body)

        second = self.exchange({
            "messages": [
                message,
                {"role": "assistant", "content": first.stdout.decode()},
                {"role": "user", "content": f"Use {PASSWORD} again."},
                {"role": "tool", "tool_call_id": "call_example", "content": f"Password: {PASSWORD}"},
                {"role": "user", "content": [{"type": "text", "text": f"File contents: {EMAIL}"}]},
            ],
            "stream": True,
        })
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(second.stdout, first.stdout)
        previous_tokens = TOKEN.findall(first_body.decode())
        current_tokens = TOKEN.findall(self.provider.received[-1].decode())
        self.assertEqual(current_tokens.count(previous_tokens[0]), 3)
        for body in self.provider.received:
            self.assertNotIn(EMAIL.encode(), body)
            self.assertNotIn(PASSWORD.encode(), body)

    def test_responses_filter(self):
        valid = self.exchange({"input": f"Email: {EMAIL}"}, "responses")
        self.assertEqual(valid.returncode, 0, valid.stderr)
        self.assertEqual(valid.stdout.decode(), f"For {EMAIL}")
        self.assertNotIn(EMAIL.encode(), self.provider.received[0])
        invalid = self.exchange({"input": "hello", "metadata": {"secret": EMAIL}}, "responses")
        self.assertNotEqual(invalid.returncode, 0)
        self.assertEqual(invalid.stdout, b"")
        self.assertEqual(len(self.provider.received), 1)

    def test_truncated_stream(self):
        self.provider.truncate_stream = True
        output = self.exchange({
            "messages": [{"role": "user", "content": f"Email: {EMAIL}"}],
            "stream": True,
        })
        self.assertNotEqual(output.returncode, 0)
        self.assertEqual(output.stdout, b"")
        self.assertNotIn(EMAIL.encode(), output.stderr)


if __name__ == "__main__":
    unittest.main()
