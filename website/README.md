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
Sign in to `gh` with an account that can read the repository. The deploy
command stages and verifies all published Redakt binary releases before
uploading the site. It keeps the versioned URLs available and serves the
current checksum manifest at `/releases/latest/download/SHA256SUMS`.

```sh
npm run check
npm run deploy -- --dry-run
npm run deploy
```

The Rust CLI is also distributed through Cargo and GitHub Releases.

The homepage and project information pages serve HTML by default and Markdown
when requested with `Accept: text/markdown`. The Worker sets `Vary: Accept` and
preserves the 404 status for missing pages in both formats.

With the development server running, run `SITE_URL=http://localhost:4321 npm test` to check
content negotiation. Use the deployed URL to check production.
