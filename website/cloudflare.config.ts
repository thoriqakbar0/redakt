import { defineConfig } from "cf/config";

export default defineConfig({
  worker: {
    name: "redakt",
    compatibilityDate: "2026-11-20",
    domains: ["redakt.ta-0.com"],
    workersDev: false,
    previewUrls: false,
  },
});
