#!/usr/bin/env node
// ecodex npm postinstall — downloads the platform-specific ecodex
// binary from the matching GitHub release tag (v$packageVersion) into
// vendor/<platform-slug>/ecodex.
//
// Skipped silently in dev (when the package directory contains a
// .git directory — the assumption is local development uses
// `cargo build --release` and points at codex-rs/target/release/).
//
// Failure does not abort npm install — instead it prints a clear
// message about how to run the binary manually. This avoids breaking
// `npm install` for users on platforms we don't yet ship binaries for.

import { existsSync, mkdirSync, chmodSync, createWriteStream, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { platform, arch } from "node:os";
import { get as httpsGet } from "node:https";
import { createRequire } from "node:module";

const __filename = fileURLToPath(import.meta.url);
const packageRoot = resolve(dirname(__filename), "..");

// Skip in dev — if the package is checked out from git, the user is
// almost certainly developing ecodex itself and has built binaries
// locally already.
if (existsSync(resolve(packageRoot, "..", ".git"))) {
  console.log("ecodex postinstall: dev checkout detected, skipping binary download");
  process.exit(0);
}

const require = createRequire(import.meta.url);
const { version } = require(resolve(packageRoot, "package.json"));

function platformSlug() {
  const os = platform();
  const cpu = arch();
  if (os === "linux" && cpu === "x64") return "linux-x86_64";
  if (os === "linux" && cpu === "arm64") return "linux-aarch64";
  if (os === "darwin" && cpu === "x64") return "macos-x86_64";
  if (os === "darwin" && cpu === "arm64") return "macos-aarch64";
  return null;
}

const slug = platformSlug();
if (slug === null) {
  console.error(
    `ecodex postinstall: no prebuilt for ${platform()}-${arch()}. ` +
      `Install from source: https://github.com/EmpiricaAI/ecodex#install`
  );
  // Exit 0 — don't fail the npm install; the wrapper will print a
  // clearer error if the user tries to run ecodex.
  process.exit(0);
}

// Asset naming convention: matches what release.sh --upload-assets
// produces (just `ecodex` for now; we'll extend to per-platform when
// we have actual cross-compile in CI). For v0, the asset name embeds
// the platform slug so the postinstall can pick the right one.
const assetName = `ecodex-${slug}`;
const url = `https://github.com/EmpiricaAI/ecodex/releases/download/v${version}/${assetName}`;
const destDir = resolve(packageRoot, "vendor", slug);
const destPath = resolve(destDir, "ecodex");

mkdirSync(destDir, { recursive: true });

console.log(`ecodex postinstall: downloading ${url}`);
console.log(`ecodex postinstall: → ${destPath}`);

function download(downloadUrl, destination, redirectsLeft = 5) {
  return new Promise((resolveP, rejectP) => {
    const handler = (response) => {
      // Handle redirects (gh release URLs typically 302 to S3).
      if (
        response.statusCode &&
        response.statusCode >= 300 &&
        response.statusCode < 400 &&
        response.headers.location
      ) {
        if (redirectsLeft <= 0) {
          rejectP(new Error("too many redirects"));
          return;
        }
        download(response.headers.location, destination, redirectsLeft - 1)
          .then(resolveP)
          .catch(rejectP);
        return;
      }
      if (response.statusCode !== 200) {
        rejectP(
          new Error(
            `unexpected ${response.statusCode} from ${downloadUrl} ` +
              `(asset may not exist for this version/platform yet)`
          )
        );
        return;
      }
      const file = createWriteStream(destination);
      response.pipe(file);
      file.on("finish", () => file.close(() => resolveP()));
      file.on("error", rejectP);
    };
    httpsGet(downloadUrl, handler).on("error", rejectP);
  });
}

try {
  await download(url, destPath);
  chmodSync(destPath, 0o755);
  const sizeBytes = statSync(destPath).size;
  console.log(
    `ecodex postinstall: ✓ installed ${(sizeBytes / 1024 / 1024).toFixed(1)} MiB binary`
  );
} catch (err) {
  console.error(`ecodex postinstall: download failed — ${err.message}`);
  console.error(
    `ecodex postinstall: install from source: https://github.com/EmpiricaAI/ecodex#install`
  );
  // Exit 0 — don't fail npm install; the wrapper surfaces the
  // missing-binary error with actionable text when the user runs ecodex.
  process.exit(0);
}
