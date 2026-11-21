import cloudflare from "@astrojs/cloudflare";
import solid from "@astrojs/solid-js";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://privacy-hygiene.thoriqakbar00.workers.dev",
  output: "server",
  adapter: cloudflare({ imageService: "passthrough" }),
  session: false,
  integrations: [solid()],
});
