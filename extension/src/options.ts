import { findPort, forgetPort, pair } from "./api";
import { browserName, ext, settings } from "./browser";
import { privateWindowHint } from "./format";
import { load } from "./queue";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

async function renderIncognitoHint() {
  const on = $<HTMLInputElement>("incognito").checked;
  const allowed = await ext.extension.isAllowedIncognitoAccess().catch(() => true);
  const hint = $("incognito-hint");
  hint.hidden = !on || allowed;
  hint.textContent = privateWindowHint(browserName());
}

async function render() {
  const s = await settings();
  const port = await findPort();
  const status = $("status");
  if (port === null) {
    status.textContent = "The Nagsworth app is not running on this computer.";
    status.className = "err";
  } else if (!s.token) {
    status.textContent = `App found on port ${port}. Not paired yet.`;
    status.className = "muted";
  } else {
    status.textContent = `Paired with the app on port ${port}.`;
    status.className = "ok";
  }
  $("pair").hidden = Boolean(s.token);
  $("paired-actions").hidden = !s.token;
  const queued = (await load()).length;
  $("queued").textContent = queued ? `${queued} events waiting to be sent` : "";
  const opts = await ext.storage.local.get(["includeIncognito", "badge"]);
  $<HTMLInputElement>("incognito").checked = opts.includeIncognito === true;
  $<HTMLInputElement>("badge").checked = opts.badge === true;
  await renderIncognitoHint();
}

$("pair-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const msg = $("pair-msg");
  try {
    await pair($<HTMLInputElement>("code").value.trim());
    msg.textContent = "Paired.";
    msg.className = "ok";
    await ext.runtime.sendMessage({ type: "paired" }).catch(() => undefined);
  } catch (err) {
    msg.textContent = err instanceof Error ? err.message : String(err);
    msg.className = "err";
  }
  await render();
});

$("unpair").addEventListener("click", async () => {
  await ext.storage.local.remove("token");
  await render();
});

$<HTMLInputElement>("incognito").addEventListener("change", async (e) => {
  await ext.storage.local.set({ includeIncognito: (e.target as HTMLInputElement).checked });
  await renderIncognitoHint();
});

$<HTMLInputElement>("badge").addEventListener("change", async (e) => {
  await ext.storage.local.set({ badge: (e.target as HTMLInputElement).checked });
  await ext.runtime.sendMessage({ type: "badge" }).catch(() => undefined);
});

// Two clicks instead of confirm(), which embedded options pages do not show.
let armed = false;
$("forget").addEventListener("click", async () => {
  const button = $("forget");
  if (!armed) {
    armed = true;
    button.textContent = "Click again to forget everything";
    setTimeout(() => {
      armed = false;
      button.textContent = "Forget everything";
    }, 5000);
    return;
  }
  armed = false;
  button.textContent = "Forget everything";
  await ext.storage.local.clear();
  await ext.storage.session.clear();
  forgetPort();
  await ext.action.setBadgeText({ text: "" });
  await ext.runtime.sendMessage({ type: "forgotten" }).catch(() => undefined);
  const msg = $("forget-msg");
  msg.textContent = "Done. Pair again to resume reporting.";
  msg.className = "hint ok";
  await render();
});

void render();
