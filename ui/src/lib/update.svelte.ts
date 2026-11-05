// The update state, shared by the shell's banner and the Settings card. It
// comes from the service (`/update`) and follows the event stream's `update`
// events, so it works the same in the app and in a plain browser.

import { subscribeEvents, updateApi, updateStatus, type UpdateStatus } from "./api";
import { notify } from "./notify.svelte";

let status = $state<UpdateStatus | null>(null);
let started = false;

async function reload() {
  try {
    status = await updateStatus();
  } catch {
    // The service is not answering; the stream reconnects and reloads.
  }
}

function start() {
  if (started) return;
  started = true;
  subscribeEvents(
    ["update"],
    (e) => {
      if (e.source === "update") status = e.event as UpdateStatus;
    },
    () => void reload(),
  );
}

async function run(action: () => Promise<unknown>, done?: string) {
  try {
    const next = await action();
    if (next && typeof next === "object" && "current" in next) status = next as UpdateStatus;
    if (done) notify.ok(done);
  } catch (e) {
    notify.err(e);
  }
}

export const updates = {
  /** Subscribes once; safe to call from every page. */
  start,
  get status(): UpdateStatus | null {
    return status;
  },
  check: () => run(updateApi.check),
  download: () => run(updateApi.download),
  cancel: () => run(updateApi.cancel),
  install: () => run(updateApi.install),
  dismiss: () => run(updateApi.dismiss),
  setAuto: (on: boolean) =>
    run(() => updateApi.setAuto(on), on ? "Checks for updates twice a day" : "No automatic update checks"),
  setBeta: (on: boolean) => run(() => updateApi.setBeta(on)),
  openReleasePage: () => {
    const url = status?.available?.url;
    return url ? run(() => updateApi.openReleasePage(url)) : Promise.resolve();
  },
};
