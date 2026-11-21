import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const script = fileURLToPath(new URL("../scripts/stage-releases.sh", import.meta.url));
const targets = [
  "aarch64-apple-darwin.tar.gz",
  "aarch64-unknown-linux-gnu.tar.gz",
  "x86_64-apple-darwin.tar.gz",
  "x86_64-pc-windows-msvc.zip",
  "x86_64-unknown-linux-gnu.tar.gz",
];

test("staging preserves versioned downloads and rejects a corrupt release", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "redakt-stage-"));
  t.after(() => rm(root, { recursive: true, force: true }));

  const gh = join(root, "gh");
  await writeFile(gh, `#!/bin/sh
set -eu
case "$1:$2" in
  release:view) printf '%s\\n' v0.0.7 ;;
  api:--paginate) printf '%s\\n' v0.0.7 v0.0.6 ;;
  release:download)
    tag=$3
    while [ "$1" != --dir ]; do shift; done
    cp "$FIXTURES/$tag/"* "$2/"
    ;;
  *) exit 1 ;;
esac
`);
  await chmod(gh, 0o755);

  for (const tag of ["v0.0.6", "v0.0.7"]) {
    const release = join(root, tag);
    await mkdir(release);
    const entries = [];
    for (const target of targets) {
      const archive = `redakt-${tag}-${target}`;
      await writeFile(join(release, archive), archive);
      entries.push(`${createHash("sha256").update(archive).digest("hex")}  ${archive}\n`);
    }
    await writeFile(join(release, "SHA256SUMS"), entries.join(""));
  }

  const stage = () => spawnSync("sh", [script], {
    cwd: root,
    env: { ...process.env, PATH: `${root}:${process.env.PATH}`, FIXTURES: root },
    encoding: "utf8",
  });
  const first = stage();
  assert.equal(first.status, 0, first.stderr);
  const published = join(root, "public/releases");
  const latest = join(published, "latest/download/SHA256SUMS");
  const manifest = await readFile(latest, "utf8");
  assert.equal(manifest, await readFile(join(root, "v0.0.7/SHA256SUMS"), "utf8"));
  const olderArchive = "redakt-v0.0.6-x86_64-apple-darwin.tar.gz";
  assert.equal(await readFile(join(published, "download/v0.0.6", olderArchive), "utf8"), olderArchive);

  await writeFile(join(root, "v0.0.6", olderArchive), "corrupt");
  const corrupt = stage();
  assert.notEqual(corrupt.status, 0);
  assert.match(corrupt.stderr, /invalid archive checksums/);
  assert.equal(await readFile(latest, "utf8"), manifest);
});
