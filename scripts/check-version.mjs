#!/usr/bin/env node
// Fails unless package.json, Cargo.toml and tauri.conf.json agree on one version (and the git tag, if given).
// Usage: node scripts/check-version.mjs [--root DIR] [--tag v1.2.3]
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const root = resolve(opt("--root") ?? new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"));
const tag = opt("--tag") ?? process.env.GITHUB_REF_NAME;

const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8")).version;
const cargoText = readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8");
const cargo = /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargoText)?.[1];

const found = { "package.json": pkg, "src-tauri/Cargo.toml": cargo, "src-tauri/tauri.conf.json": conf };
const semver = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
const problems = [];
for (const [file, v] of Object.entries(found)) {
  if (!v) problems.push(`${file}: no version found`);
  else if (!semver.test(v)) problems.push(`${file}: "${v}" is not a version like 1.2.3`);
}
if (new Set(Object.values(found)).size > 1) problems.push(`versions differ: ${JSON.stringify(found)}`);
if (tag && /^v\d/.test(tag) && tag.slice(1) !== pkg) problems.push(`git tag ${tag} does not match version ${pkg}`);

if (problems.length) {
  console.error("Version check failed:\n - " + problems.join("\n - "));
  process.exit(1);
}
console.log(`Version check passed: ${pkg}`);
