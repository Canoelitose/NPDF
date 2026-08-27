#!/usr/bin/env node
/**
 * Fetches the public sample PDFs the integration tests use.
 *
 * The files are not in the repository, so a clone stays small and no licence
 * question travels with the history. Every file has a pinned checksum, so an
 * upstream change fails here rather than quietly changing what a test measures.
 *
 * Tests that need a fixture skip themselves when it is not there, so running
 * without network access still gives a green, if smaller, test run.
 */

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const manifestPath = join(root, "tests/fixtures/manifest.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const targetDir = join(root, "tests/fixtures/downloads");

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

async function fetchOne(entry) {
  const destination = join(targetDir, entry.name);
  if (existsSync(destination)) {
    const digest = sha256(readFileSync(destination));
    if (digest === entry.sha256) {
      process.stdout.write(`  ${entry.name.padEnd(24)} bereits vorhanden\n`);
      return;
    }
    process.stdout.write(`  ${entry.name.padEnd(24)} Pruefsumme falsch, wird neu geladen\n`);
  }

  const url = `${manifest.baseUrl}${entry.path}`;
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`${url} answered ${response.status} ${response.statusText}`);
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  const digest = sha256(buffer);
  if (digest !== entry.sha256) {
    throw new Error(
      `checksum mismatch for ${entry.name}\n  expected ${entry.sha256}\n  received ${digest}`,
    );
  }
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, buffer);
  process.stdout.write(`  ${entry.name.padEnd(24)} ${buffer.length} Bytes\n`);
}

async function main() {
  mkdirSync(targetDir, { recursive: true });
  process.stdout.write(`Testdateien nach tests/fixtures/downloads, Quelle ${manifest.source}\n`);
  for (const entry of manifest.files) {
    await fetchOne(entry);
  }
  if (manifest.missing?.length) {
    process.stdout.write("\nNoch offen:\n");
    for (const item of manifest.missing) {
      process.stdout.write(`  - ${item}\n`);
    }
  }
}

main().catch((error) => {
  process.stderr.write(`\nfetch-fixtures failed: ${error.message}\n`);
  process.exit(1);
});
