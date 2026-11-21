import { defineMiddleware } from "astro:middleware";
import Negotiator from "negotiator";
import home from "./content/home.md?raw";
import setupPrompt from "./content/setup-prompt.txt?raw";
import about from "./pages/about.md?raw";
import contact from "./pages/contact.md?raw";
import privacy from "./pages/privacy.md?raw";

const markdownPages = new Map([
  ["/", `${home}\n## Setup prompt for Codex or Claude Code\n\n${setupPrompt}`],
  ["/about", about],
  ["/contact", contact],
  ["/privacy", privacy],
]);

const notFound = [
  "# Page not found",
  "",
  "This address does not point to a page on the privacy-hygiene site.",
  "",
  "Use the [homepage](/), [sitemap](/sitemap.xml), or [agent guide](/llms.txt).",
  "",
].join("\n");

function markdown(body: string, status: number, method: string) {
  return new Response(method === "HEAD" ? null : body, {
    status,
    headers: {
      "Content-Type": "text/markdown; charset=utf-8",
      Vary: "Accept",
    },
  });
}

export const onRequest = defineMiddleware(async ({ request, url }, next) => {
  const { method } = request;
  if (method !== "GET" && method !== "HEAD") return next();

  const path = url.pathname.replace(/\/$/, "") || "/";
  const body = markdownPages.get(path);
  const accept = request.headers.get("accept") ?? "*/*";
  const format = new Negotiator({ headers: { accept } })
    .mediaType(["text/html", "text/markdown"]);

  if (body && format === "text/markdown") {
    return markdown(body.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n+/, ""), 200, method);
  }

  if (body && !format) {
    return new Response(null, { status: 406, headers: { Vary: "Accept" } });
  }

  const response = await next();
  if (response.status === 404 && format === "text/markdown") {
    return markdown(notFound, 404, method);
  }

  if (body || response.status === 404) response.headers.append("Vary", "Accept");
  return response;
});
