# redakt website

Astro builds the homepage and SolidJS renders it. The terminal displays
synthetic input and output from the Rust CLI.

The homepage demos repeat when motion is enabled. Use **Pause animations**
to stop them. Selecting a terminal example keeps that example on screen.
The setup prompt works with Codex, Claude Code, Cursor, and Gemini CLI.
Open **Windows and Cargo** to copy either install command.

The [assistant logos](https://github.com/lobehub/lobe-icons/tree/9752d967494e48d8f83f2808feb2b0207bf70997/packages/static-svg/icons) are bundled with their [license](public/assistant-logos/LICENSE).

From this directory, run the development server with Node.js 22.23.4 or later.

```sh
npm ci
npm run dev
```

## Deploy to Cloudflare Workers

The `redakt` Worker serves the public site at <https://redakt.ta-0.com>.

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

The homepage serves HTML by default and Markdown
when requested with `Accept: text/markdown`. The Worker sets `Vary: Accept` and
preserves the 404 status for missing pages in both formats. Former information
page URLs redirect to their homepage sections.

With the development server running, run `SITE_URL=http://localhost:4321 npm test` to check
content negotiation. Use the deployed URL to check production.
