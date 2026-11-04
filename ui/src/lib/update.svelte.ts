// The app's update state, shared by the shell's banner and the Settings card.
// Inside the app it comes from the `update_status` command and the
// `update-status` event; in a plain browser it stays `null`.

import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { UPDATE_EVENT, updateApi, updateStatus, type UpdateStatus } from "./api";
import { notify } from "./notify.svelte";

let status = $state<UpdateStatus | null>(null);
let started = false;

async function start() {
  if (started || !isTauri()) return;
  started = true;
  try {
    await listen<UpdateStatus>(UPDATE_EVENT, (e) => (status = e.payload));
    status = await updateStatus();
  } catch (e) {
    notify.err(e);
  }
}

async function run(action: () => Promise<void>, done?: string) {
  try {
    await action();
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
    run(updateApi.setAuto.bind(null, on), on ? "Checks for updates twice a day" : "No automatic update checks"),
  setBeta: (on: boolean) => run(updateApi.setBeta.bind(null, on)),
  openReleasePage: () => run(updateApi.openReleasePage),
};
