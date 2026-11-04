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

for (const entry of ["background", "options", "popup"]) {
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
writeFileSync(join(staging, "privacy.html"), markdownPage(readFileSync(join(root, "store", "privacy.md"), "utf8")));

// The privacy text ships inside the extension (linked from the options page)
// from the same source as the store listing. Only the Markdown that file
// uses: headings, paragraphs, "- " lists, `code`, **bold** and bare URLs.
function markdownPage(md) {
  const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const inline = (s) =>
    esc(s)
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/(https:\/\/[^\s<]+[^\s<.,)])/g, '<a href="$1">$1</a>');
  const out = [];
  let title = "Privacy";
  for (const block of md.trim().split(/\n\s*\n/)) {
    const lines = block.split("\n");
    const h = /^(#+) (.*)$/.exec(lines[0]);
    if (h) {
      if (h[1].length === 1) title = h[2];
      out.push(`<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`);
    } else if (lines[0].startsWith("- ")) {
      const items = block.split(/\n(?=- )/).map((it) => it.replace(/^- /, "").replace(/\s*\n\s*/g, " "));
      out.push(`<ul>${items.map((it) => `<li>${inline(it)}</li>`).join("")}</ul>`);
    } else {
      out.push(`<p>${inline(lines.join(" "))}</p>`);
    }
  }
  return `<!doctype html>
<html lang="en">
<head><meta charset="UTF-8" /><title>${esc(title)}</title><link rel="stylesheet" href="common.css" />
<style>body { padding: 16px 24px; max-width: 680px; } h1 { font-size: 20px; } h2 { font-size: 14px; margin-top: 20px; text-transform: none; letter-spacing: 0; color: var(--text); } p, li { margin: 6px 0; }</style>
</head>
<body>
${out.join("\n")}
</body>
</html>
`;
}

const base = JSON.parse(readFileSync(join(root, "manifest.base.json"), "utf8"));
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
base.version = pkg.version;

const targets = {
  // storage.session (the retry deadline across worker restarts) needs
  // Chrome 102 and Firefox 115.
  chrome: { ...base, minimum_chrome_version: "102", background: { service_worker: "background.js" } },
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
