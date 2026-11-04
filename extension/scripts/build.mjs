// Builds the extension for Chrome and Firefox:
//   dist/chrome   background.service_worker
//   dist/firefox  background.scripts + browser_specific_settings.gecko
// Each script is bundled into one self-contained IIFE so both browsers load
// it without module support.

import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const staging = join(dist, "build");

rmSync(dist, { recursive: true, force: true });

for (const entry of ["background", "options"]) {
  await build({
    root,
    configFile: false,
    logLevel: "warn",
    build: {
      outDir: staging,
      emptyOutDir: false,
      target: "es2022",
      minify: false,
      lib: {
        entry: join(root, "src", `${entry}.ts`),
        formats: ["iife"],
        name: entry,
        fileName: () => `${entry}.js`,
      },
    },
  });
}
cpSync(join(root, "static"), staging, { recursive: true });
cpSync(join(root, "icons"), join(staging, "icons"), { recursive: true });

const base = JSON.parse(readFileSync(join(root, "manifest.base.json"), "utf8"));
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
base.version = pkg.version;

const targets = {
  chrome: { ...base, background: { service_worker: "background.js" } },
  firefox: {
    ...base,
    background: { scripts: ["background.js"] },
    browser_specific_settings: {
      gecko: { id: "nagsworth@nagsworth.org", strict_min_version: "115.0" },
    },
  },
};

for (const [name, manifest] of Object.entries(targets)) {
  const out = join(dist, name);
  mkdirSync(out, { recursive: true });
  cpSync(staging, out, { recursive: true });
  writeFileSync(join(out, "manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
  console.log(`built ${out}`);
}
rmSync(staging, { recursive: true, force: true });
