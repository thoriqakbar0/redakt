"""Send a masked text request and print the locally restored reply."""

import argparse
import http.client
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


class InvalidExchange(ValueError):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, location):
        return None


def filter_text(arguments, text, reverse=False):
    command = [arguments.redakt, "--recovery", arguments.recovery]
    if arguments.code_file:
        command.extend(["--code-file", arguments.code_file])
    if reverse:
        command.append("--reverse")
    result = subprocess.run(
        command,
        input=text.encode("utf-8"),
        capture_output=True,
        env={**os.environ, "REDAKT_UPDATE_NOTIFY": "0"},
        check=True,
    )
    return result.stdout.decode("utf-8")


def message_fields(message, responses):
    if not isinstance(message, dict):
        raise InvalidExchange()
    role = message.get("role")
    if responses and "role" not in message:
        if message.keys() != {"type", "call_id", "output"}:
            raise InvalidExchange()
        if message["type"] != "function_call_output":
            raise InvalidExchange()
        check_call_id(message["call_id"])
        yield message, "output"
        return
    if role not in ("developer", "system", "user", "assistant", "tool"):
        raise InvalidExchange()
    if not responses and role == "tool":
        if message.keys() != {"role", "content", "tool_call_id"}:
            raise InvalidExchange()
        check_call_id(message["tool_call_id"])
    elif message.keys() != {"role", "content"}:
        raise InvalidExchange()
    if responses and role == "tool":
        raise InvalidExchange()

    content = message["content"]
    if isinstance(content, list) and content:
        for part in content:
            part_type = "input_text" if responses else "text"
            if not isinstance(part, dict) or part.keys() != {"type", "text"}:
                raise InvalidExchange()
            if part["type"] != part_type:
                raise InvalidExchange()
            yield part, "text"
    else:
        yield message, "content"


def check_call_id(value):
    if not isinstance(value, str) or not re.fullmatch(r"call_[0-9A-Za-z_-]{1,64}", value):
        raise InvalidExchange()


def mask_text(arguments, text):
    if not isinstance(text, str):
        raise InvalidExchange()
    return filter_text(arguments, text)


def mask_request(arguments, request, responses):
    field = "input" if responses else "messages"
    allowed = {field, "stream", "instructions"} if responses else {field, "stream"}
    if not isinstance(request, dict) or request.keys() - allowed or field not in request:
        raise InvalidExchange()
    if "stream" in request and not isinstance(request["stream"], bool):
        raise InvalidExchange()
    fields = [(request, "instructions")] if "instructions" in request else []
    messages = request[field]
    if responses and isinstance(messages, str):
        fields.append((request, field))
    elif isinstance(messages, list) and messages:
        for message in messages:
            fields.extend(message_fields(message, responses))
    else:
        raise InvalidExchange()

    if len(fields) > 1:
        for container, key in fields:
            mask_text(arguments, container[key])
    for container, key in fields:
        container[key] = mask_text(arguments, container[key])
    request["model"] = arguments.model
    return json.dumps(request, ensure_ascii=False).encode("utf-8")


def endpoint_protocol(url):
    endpoint = urllib.parse.urlsplit(url)
    if endpoint.username or endpoint.password or endpoint.query or endpoint.fragment:
        raise InvalidExchange()
    loopback = endpoint.hostname in ("localhost", "127.0.0.1", "::1")
    if endpoint.scheme != "https" and (endpoint.scheme != "http" or not loopback):
        raise InvalidExchange()
    if endpoint.path.endswith("/chat/completions"):
        return False
    if endpoint.path.endswith("/responses"):
        return True
    raise InvalidExchange()


def send(arguments, body):
    headers = {"Content-Type": "application/json"}
    credential = os.environ.get("OPENAI_API_KEY")
    if credential:
        headers["Authorization"] = f"Bearer {credential}"
    request = urllib.request.Request(arguments.url, body, headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    try:
        return opener.open(request, timeout=30)
    except urllib.error.HTTPError as error:
        error.close()
        if error.code not in (429, 502, 503, 504):
            raise
    return opener.open(request, timeout=30)


def events(response):
    data = []
    for wire_line in response:
        line = wire_line.decode("utf-8").rstrip("\r\n")
        if line.startswith("data:"):
            data.append(line.removeprefix("data:").removeprefix(" "))
        elif not line and data:
            yield "\n".join(data)
            data.clear()
    if data:
        raise InvalidExchange()


def read_stream(response, responses):
    chunks = []
    for payload in events(response):
        if not responses and payload == "[DONE]":
            return ["".join(chunks)]
        event = json.loads(payload)
        if "error" in event or event.get("type") in (
            "error", "response.failed", "response.incomplete"
        ):
            raise InvalidExchange()
        if not responses:
            choices = event["choices"]
            if not choices:
                continue
            if len(choices) != 1 or choices[0]["index"] != 0:
                raise InvalidExchange()
            chunks.append(choices[0]["delta"].get("content") or "")
        elif event["type"] == "response.completed":
            return response_parts(event["response"])
    raise InvalidExchange()


def response_parts(payload):
    return [
        part["text"]
        for item in payload["output"] if item["type"] == "message"
        for part in item["content"] if part["type"] == "output_text"
    ]


def read_reply(response, responses):
    if response.headers.get_content_type() == "text/event-stream":
        return read_stream(response, responses)
    payload = json.load(response)
    if responses:
        if payload.get("status") != "completed":
            raise InvalidExchange()
        return response_parts(payload)
    choices = payload["choices"]
    if len(choices) != 1:
        raise InvalidExchange()
    return [choices[0]["message"]["content"]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--redakt", default="redakt")
    parser.add_argument("--recovery", required=True)
    parser.add_argument("--code-file")
    parser.add_argument("--url", required=True)
    parser.add_argument("--model", required=True)
    arguments = parser.parse_args()
    try:
        responses = endpoint_protocol(arguments.url)
        body = mask_request(arguments, json.load(sys.stdin.buffer), responses)
        with send(arguments, body) as response:
            parts = read_reply(response, responses)
        restored = [filter_text(arguments, part, reverse=True) for part in parts]
        sys.stdout.buffer.write("".join(restored).encode("utf-8"))
        sys.stdout.buffer.flush()
    except (
        OSError, ValueError, KeyError, TypeError, AttributeError,
        http.client.HTTPException, subprocess.SubprocessError,
    ):
        print("OpenAI filter could not complete the exchange", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
