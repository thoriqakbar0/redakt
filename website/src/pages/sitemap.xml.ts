import type { APIRoute } from "astro";

const pages = ["/", "/about", "/contact", "/privacy"];
const lastModified = "2026-11-21";

export const prerender = true;

export const GET: APIRoute = ({ site }) => {
  const entries = pages.map((path) => {
    const location = new URL(path, site);
    return `<url><loc>${location}</loc><lastmod>${lastModified}</lastmod></url>`;
  });

  const xml = [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
    ...entries,
    '</urlset>',
  ].join("\n");

  return new Response(xml, { headers: { "Content-Type": "application/xml; charset=utf-8" } });
};
