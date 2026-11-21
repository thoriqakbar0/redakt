import cloudflare from "@astrojs/cloudflare";
import solid from "@astrojs/solid-js";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://redakt.ta-0.com",
  output: "server",
  adapter: cloudflare({ imageService: "passthrough" }),
  session: false,
  integrations: [solid()],
});
