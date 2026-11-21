import { defineMiddleware } from "astro:middleware";
import Negotiator from "negotiator";
import home from "./content/home.md?raw";
import setupPrompt from "./content/setup-prompt.txt?raw";
const homeMarkdown = `${home}\n## Setup prompt for coding assistants\n\n${setupPrompt}`;
const legacySections = new Map([
  ["/about", "/#local-model"],
  ["/contact", "/#support"],
  ["/privacy", "/#privacy"],
]);

const notFound = [
  "# Page not found",
  "",
  "This address does not point to a page on the redakt site.",
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
  const section = legacySections.get(path);
  if (section) return new Response(null, { status: 308, headers: { Location: section } });

  const body = path === "/" ? homeMarkdown : undefined;
  const accept = request.headers.get("accept") ?? "*/*";
  const format = new Negotiator({ headers: { accept } })
    .mediaType(["text/html", "text/markdown"]);

  if (body && format === "text/markdown") {
    return markdown(body, 200, method);
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
