import { findPort, pair } from "./api";
import { ext, settings } from "./browser";
import { load } from "./queue";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

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
  const inc = await ext.storage.local.get("includeIncognito");
  $<HTMLInputElement>("incognito").checked = inc.includeIncognito === true;
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
});

void render();
