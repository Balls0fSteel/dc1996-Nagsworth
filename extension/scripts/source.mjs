// Zips the extension's sources for the AMO source upload:
//   dist/nagsworth-extension-source-<version>.zip
// Everything under extension/ except node_modules, dist and test output.
// A reviewer rebuilds the add-on with `npm ci` and `npm run package`
// (Node 22); store/listing.md has the notes for reviewers.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { zip } from "./package.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const { version } = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const SKIP = new Set(["node_modules", "dist", "coverage", ".DS_Store"]);

mkdirSync(dist, { recursive: true });
const out = join(dist, `nagsworth-extension-source-${version}.zip`);
writeFileSync(out, zip(root, (p) => SKIP.has(basename(p))));
console.log(`packed ${out}`);
