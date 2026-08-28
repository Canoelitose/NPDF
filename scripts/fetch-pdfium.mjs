#!/usr/bin/env node
/**
 * Fetches the PDFium library for one or more targets.
 *
 * PDFium is a C++ library, so it cannot be built by cargo. The prebuilt packages
 * from bblanchon/pdfium-binaries cover all five of our targets. PDFium itself is
 * BSD-3-Clause, the packaging is Apache-2.0, both are compatible with this
 * project.
 *
 * Usage:
 *   node scripts/fetch-pdfium.mjs                 the current machine
 *   node scripts/fetch-pdfium.mjs linux-x64       one target
 *   node scripts/fetch-pdfium.mjs --all           check every target, place none
 *   node scripts/fetch-pdfium.mjs --verify a b    check the named targets only
 *   node scripts/fetch-pdfium.mjs --list          what is configured
 *
 * Several targets share one destination on purpose, because the bundler needs
 * the library at a fixed path: linux-x64 and linux-arm64 both become
 * vendor/pdfium/lib/libpdfium.so, and only one of them belongs on a given
 * machine. Placing every target would therefore leave the wrong file behind,
 * so --all only downloads and checks, it never writes into the tree. Use it to
 * fill the checksum lock; name the target you actually need to install one.
 *
 * The checksum of every downloaded archive is recorded in scripts/pdfium-lock.json
 * on the first run and verified on every run after that. Commit that file.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const config = JSON.parse(readFileSync(join(here, "pdfium.config.json"), "utf8"));
const lockPath = join(here, "pdfium-lock.json");

function loadLock() {
  return existsSync(lockPath) ? JSON.parse(readFileSync(lockPath, "utf8")) : {};
}

function saveLock(lock) {
  writeFileSync(lockPath, `${JSON.stringify(lock, null, 2)}\n`);
}

function assetUrl(assetName) {
  const file = `${assetName}.tgz`;
  return `https://github.com/${config.repository}/releases/download/${config.release}/${file}`;
}

async function download(url) {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`${url} answered ${response.status} ${response.statusText}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

/**
 * The library file names of every platform we ship.
 *
 * Several targets install into the same directory, because the bundler expects
 * the library at a fixed path. Without this the leftovers of an earlier fetch
 * stay behind and get packaged: a Linux build would carry a macOS dylib and a
 * Windows DLL around with it, more than twenty megabytes of dead weight.
 */
const LIBRARY_NAMES = ["pdfium.dll", "libpdfium.dylib", "libpdfium.so", "libpdfium.a"];

function removeForeignLibraries(directory, keep) {
  for (const name of LIBRARY_NAMES) {
    if (name === keep) continue;
    const candidate = join(directory, name);
    if (existsSync(candidate)) {
      unlinkSync(candidate);
      process.stdout.write(`     entfernt: ${name}, gehoert zu einer anderen Plattform\n`);
    }
  }
}

function extract(archivePath, into) {
  mkdirSync(into, { recursive: true });
  // tar ships with macOS, Linux and Windows 10 and later, so no extra dependency.
  execFileSync("tar", ["-xzf", archivePath, "-C", into], { stdio: "inherit" });
}

async function fetchTarget(name, lock, place) {
  const target = config.targets[name];
  if (!target) {
    throw new Error(`unknown target ${name}, try --list`);
  }

  const url = assetUrl(target.asset);
  process.stdout.write(`  ${name}: ${url}\n`);
  const archive = await download(url);
  const digest = sha256(archive);

  const lockKey = `${config.release}/${target.asset}`;
  const known = lock[lockKey];
  if (known && known !== digest) {
    throw new Error(
      `checksum mismatch for ${lockKey}\n  expected ${known}\n  received ${digest}\n` +
        "Either the release was replaced or the download was tampered with. Do not use it.",
    );
  }
  lock[lockKey] = digest;

  if (!place) {
    process.stdout.write(`     geprueft, ${archive.length} Bytes, nicht abgelegt\n`);
    return;
  }

  const staging = mkdtempSync(join(tmpdir(), "npdf-pdfium-"));
  try {
    const archivePath = join(staging, "pdfium.tgz");
    writeFileSync(archivePath, archive);
    extract(archivePath, staging);

    const source = join(staging, target.library);
    if (!existsSync(source)) {
      throw new Error(
        `${target.library} is not in the archive for ${name}. ` +
          "The layout of the release changed, adjust scripts/pdfium.config.json.",
      );
    }
    const destination = join(root, target.into);
    mkdirSync(dirname(destination), { recursive: true });
    removeForeignLibraries(dirname(destination), basename(destination));
    copyFileSync(source, destination);
    process.stdout.write(`     -> ${target.into}\n`);
  } finally {
    rmSync(staging, { recursive: true, force: true });
  }
}

function defaultTargets() {
  const key = `${process.platform}-${process.arch}`;
  const found = config.hostDefaults[key];
  if (!found) {
    throw new Error(
      `no default target for ${key}. Name one explicitly, see --list.`,
    );
  }
  return found;
}

async function main() {
  const args = process.argv.slice(2);

  if (args.includes("--list")) {
    process.stdout.write(`PDFium ${config.release} from ${config.repository}\n\n`);
    for (const [name, target] of Object.entries(config.targets)) {
      process.stdout.write(`  ${name.padEnd(22)} ${target.into}\n`);
    }
    return;
  }

  const all = args.includes("--all");
  const names = all
    ? Object.keys(config.targets)
    : args.filter((arg) => !arg.startsWith("--"));
  const targets = names.length > 0 ? names : defaultTargets();
  // Several targets share a destination, so installing all of them would leave
  // the wrong library behind. --all therefore only checks.
  const place = !all && !args.includes("--verify");

  process.stdout.write(
    `PDFium ${config.release}${place ? "" : ", nur pruefen, nichts ablegen"}\n`,
  );
  const lock = loadLock();
  for (const name of targets) {
    await fetchTarget(name, lock, place);
  }
  saveLock(lock);
  process.stdout.write("done\n");
}

main().catch((error) => {
  process.stderr.write(`\nfetch-pdfium failed: ${error.message}\n`);
  process.exit(1);
});
