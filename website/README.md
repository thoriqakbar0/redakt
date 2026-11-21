# redakt website

Astro builds the homepage and SolidJS renders it. The terminal displays
synthetic input and output from the Rust CLI.

From this directory, run the development server with Node.js 22.23.4 or later.

```sh
npm ci
npm run dev
```

## Deploy to Cloudflare Workers

Authenticate with `npx cf auth login` or set `CLOUDFLARE_API_TOKEN` in CI.
Build and deploy the Astro Worker with the [Cloudflare CLI](https://developers.cloudflare.com/cf/projects/).

```sh
npm run check
npx cf deploy --dry-run
npx cf deploy
```

`npm run deploy` uses the same deployment command. The Rust CLI is
distributed separately through Cargo and GitHub Releases.

The homepage and project information pages serve HTML by default and Markdown
when requested with `Accept: text/markdown`. The Worker sets `Vary: Accept` and
preserves the 404 status for missing pages in both formats.

With the development server running, run `SITE_URL=http://localhost:4321 npm test` to check
content negotiation. Use the deployed URL to check production.
