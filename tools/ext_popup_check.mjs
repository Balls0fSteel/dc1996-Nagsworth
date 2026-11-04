// Opens the extension's toolbar popup as a tab in a Chromium browser started
// with --remote-debugging-port, reads its text and saves a screenshot.
// Used by tools/ext_e2e.ps1 for Edge; Firefox has no equivalent hook.
//
//   node tools/ext_popup_check.mjs <devtools port> <png out> [expected text...]
//
// Passes (exit 0) when the popup shows the paired status and every expected
// text (today's hosts). Needs Node 22+ for the global WebSocket.

import { writeFileSync } from "node:fs";

const [port = "9333", out = "popup.png", ...expect] = process.argv.slice(2);
const http = `http://127.0.0.1:${port}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function json(path, method = "GET") {
  const res = await fetch(http + path, { method });
  if (!res.ok) throw new Error(`${method} ${path}: HTTP ${res.status}`);
  return res.json();
}

// The extension's id from its service worker target.
let id = null;
for (let i = 0; i < 40 && !id; i++) {
  const targets = await json("/json/list").catch(() => []);
  const sw = targets.find((t) => t.url?.startsWith("chrome-extension://") && t.url.endsWith("/background.js"));
  id = sw ? new URL(sw.url).host : null;
  if (!id) await sleep(500);
}
if (!id) {
  console.error("FAIL: no extension service worker among the DevTools targets");
  process.exit(1);
}
const url = `chrome-extension://${id}/popup.html`;
const target = await json(`/json/new?${url}`, "PUT");
console.log(`popup: ${url}`);

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", () => reject(new Error("DevTools WebSocket failed")), { once: true });
});
let nextId = 1;
const pending = new Map();
ws.addEventListener("message", (m) => {
  const msg = JSON.parse(String(m.data));
  const p = pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  if (msg.error) p.reject(new Error(msg.error.message));
  else p.resolve(msg.result);
});
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const n = nextId++;
    pending.set(n, { resolve, reject });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
const text = async () =>
  (await send("Runtime.evaluate", { expression: "document.body ? document.body.innerText : ''", returnByValue: true }))
    .result.value ?? "";

// The popup's own size; the toolbar would show it the same way.
await send("Emulation.setDeviceMetricsOverride", { width: 364, height: 640, deviceScaleFactor: 1, mobile: false });

let body = "";
const ready = () => body.includes("Paired as") && expect.every((e) => body.includes(e));
for (let i = 0; i < 30; i++) {
  body = await text().catch(() => "");
  if (ready()) break;
  // The popup renders once; reload it to pick up data that arrived since.
  if (i % 5 === 4) await send("Page.reload").catch(() => undefined);
  await sleep(1000);
}
await sleep(300);
const shot = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(out, Buffer.from(shot.data, "base64"));
ws.close();
await fetch(`${http}/json/close/${target.id}`).catch(() => undefined);

console.log("---- popup text ----");
console.log(body.trim());
console.log("--------------------");
console.log(`screenshot: ${out}`);
const missing = ["Paired as", ...expect].filter((e) => !body.includes(e));
if (missing.length) {
  console.error(`FAIL: popup text lacks ${missing.map((m) => JSON.stringify(m)).join(", ")}`);
  process.exit(1);
}
console.log("PASS (popup)");
