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
  assert.match(await page.text(), /<h1[^>]*>Redact your LLM calls\./);

  const document = await request("/", markdown);
  assert.equal(document.status, 200);
  assertFormat(document, markdown);
  assert.match(await document.text(), /^# redakt/);
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
