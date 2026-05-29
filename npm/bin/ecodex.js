#!/usr/bin/env node
// ecodex npm wrapper — execs into the platform-specific binary that
// `postinstall.js` downloaded to vendor/. Exit code propagates so
// shell pipelines see the binary's status, not node's.
//
// Mirrors the openai/codex npm wrapper pattern: thin process exec,
// no Node-side logic beyond binary selection + arg passthrough.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { platform, arch } from "node:os";

const __filename = fileURLToPath(import.meta.url);
const packageRoot = resolve(dirname(__filename), "..");

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
    `ecodex: no prebuilt binary for ${platform()}-${arch()}. ` +
      `Build from source: https://github.com/Nubaeon/ecodex#install`
  );
  process.exit(1);
}

const binaryPath = resolve(packageRoot, "vendor", slug, "ecodex");
if (!existsSync(binaryPath)) {
  console.error(
    `ecodex: binary missing at ${binaryPath}. ` +
      `Re-run \`npm install -g @nubaeon/ecodex\` to fetch, or build from source.`
  );
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  shell: false,
});

if (result.error) {
  console.error(`ecodex: failed to spawn binary: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
