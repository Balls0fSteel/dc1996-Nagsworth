// Zips dist/chrome and dist/firefox for the Chrome Web Store and AMO:
//   dist/nagsworth-chrome-<version>.zip
//   dist/nagsworth-firefox-<version>.zip
// A small stored+deflate zip writer, so packaging needs no extra dependency.

import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { deflateRawSync } from "node:zlib";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function files(dir, skip) {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (skip(p)) return [];
    return statSync(p).isDirectory() ? files(p, skip) : [p];
  });
}

/** Zips every file under `dir` for which `skip(path)` is false. */
export function zip(dir, skip = () => false) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  // Fixed timestamp (1980-01-01) keeps builds reproducible.
  const time = 0;
  const date = (0 << 9) | (1 << 5) | 1;
  for (const file of files(dir, skip).sort()) {
    const name = Buffer.from(relative(dir, file).split("\\").join("/"));
    const data = readFileSync(file);
    const packed = deflateRawSync(data);
    const crc = crc32(data);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0, 6);
    local.writeUInt16LE(8, 8);
    local.writeUInt16LE(time, 10);
    local.writeUInt16LE(date, 12);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(packed.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(name.length, 26);
    local.writeUInt16LE(0, 28);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(0, 8);
    central.writeUInt16LE(8, 10);
    central.writeUInt16LE(time, 12);
    central.writeUInt16LE(date, 14);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(packed.length, 20);
    central.writeUInt32LE(data.length, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(offset, 42);
    locals.push(local, name, packed);
    centrals.push(central, name);
    offset += local.length + name.length + packed.length;
  }
  const centralSize = centrals.reduce((n, b) => n + b.length, 0);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(centrals.length / 2, 8);
  end.writeUInt16LE(centrals.length / 2, 10);
  end.writeUInt32LE(centralSize, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, ...centrals, end]);
}

// Run as a script (not when imported by source.mjs).
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  for (const target of ["chrome", "firefox"]) {
    const out = join(dist, `nagsworth-${target}-${version}.zip`);
    writeFileSync(out, zip(join(dist, target)));
    console.log(`packed ${out}`);
  }
}
