import assert from "node:assert/strict";
import test from "node:test";

const site = process.env.SITE_URL;
assert.ok(site, "Set SITE_URL to the website under test.");
const html = "text/html";
const markdown = "text/markdown";

async function request(path, accept, method = "GET") {
  return fetch(new URL(path, site), { headers: { Accept: accept }, method });
}

function assertFormat(response, type) {
  assert.equal(response.headers.get("content-type")?.split(";")[0], type);
  assert.match(response.headers.get("vary") ?? "", /\bAccept\b/i);
}

test("the same homepage URL serves HTML and Markdown", async () => {
  const page = await request("/", html);
  assert.equal(page.status, 200);
  assertFormat(page, html);
  const markup = await page.text();
  assert.match(markup, /<h1[^>]*><span[^>]*>redakt<\/span> before you prompt\./);
  assert.match(markup, /src="\/redaction-stamp\.html"/);
  assert.match(markup, /id="local-model"/);
  assert.match(markup, /100,000 synthetic English documents/);
  assert.match(markup, /https:\/\/redakt\.ta-0\.com\/install\.sh/);
  assert.match(markup, /PASSWORD=<mark\b[^>]*>\[SECRET_1\]<\/mark>/);
  assert.match(markup, /redakt --reverse --recovery session\.recovery/);
  assert.match(markup, /mktemp \.env\.redacted\.XXXXXX/);
  assert.match(markup, /<summary>Read the setup prompt<\/summary>/);

  const document = await request("/", markdown);
  assert.equal(document.status, 200);
  assertFormat(document, markdown);
  const body = await document.text();
  assert.match(body, /^# redakt/);
  assert.match(body, /built-in model runs offline on your CPU/);
  assert.match(body, /mktemp \.env\.redacted\.XXXXXX\) && redakt --irreversible < \.env > "\$copy"/);
  assert.match(body, /redakt --reverse --recovery session\.recovery/);
  assert.match(body, /cargo install redakt --locked/);
  assert.match(body, /Version 0\.0\.6 and later include the trained detector/);
});

test("a missing URL keeps its 404 status in either format", async () => {
  const path = "/missing-test-page";
  for (const type of [html, markdown]) {
    const missing = await request(path, type);
    assert.equal(missing.status, 404);
    assertFormat(missing, type);
    assert.match(await missing.text(), /Page not found/i);
  }
});

test("the HTML homepage offers prompt and install copy controls", async () => {
  const page = await request("/", html);
  const markup = await page.text();
  assert.match(markup, /<button[^>]*>\s*Copy prompt/);
  assert.match(markup, /aria-label="Copy Windows command"/);
  assert.match(markup, /aria-label="Copy Cargo command"/);
  assert.match(markup, /Use AGENTS\.md for Codex or Cursor, CLAUDE\.md for Claude Code, or GEMINI\.md/);
  for (const logo of ["openai", "claude", "cursor", "gemini"]) {
    const path = `/assistant-logos/${logo}.svg`;
    assert.ok(markup.includes(`src="${path}"`));
    const response = await request(path, "image/svg+xml");
    assert.equal(response.status, 200);
    assert.match(response.headers.get("content-type") ?? "", /^image\/svg\+xml\b/);
  }
});

test("Accept priorities are respected for HEAD requests", async () => {
  const accept = "text/markdown;q=0.5, text/html;q=0.9";
  const preferred = await request("/", accept, "HEAD");
  assert.equal(preferred.status, 200);
  assertFormat(preferred, html);
  assert.equal(await preferred.text(), "");

  const unsupported = await request("/", "application/json", "HEAD");
  assert.equal(unsupported.status, 406);
  assert.match(unsupported.headers.get("vary") ?? "", /\bAccept\b/i);
});

test("former information pages point to their homepage sections", async () => {
  const sections = { "/about": "local-model", "/contact": "support", "/privacy": "privacy" };
  for (const [path, section] of Object.entries(sections)) {
    for (const accept of [html, markdown]) {
      const response = await fetch(new URL(path, site), {
        headers: { Accept: accept },
        redirect: "manual",
      });
      assert.equal(response.status, 308);
      assert.equal(response.headers.get("location"), `/#${section}`);
    }
  }
});
