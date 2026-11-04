<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    api,
    apiInfo,
    downloadExport,
    getAutostart,
    setAutostart,
    type Health,
    type StateResponse,
  } from "../lib/api";
  import { addDays, clock, dateTime, relative } from "../lib/format";
  import { notify } from "../lib/notify.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Segmented from "../lib/ui/Segmented.svelte";
  import Switch from "../lib/ui/Switch.svelte";

  let { snap }: { snap: StateResponse | null } = $props();

  const CODE_VALID_MS = 5 * 60_000;
  const ONLINE_MS = 2 * 60_000;
  const REPO_URL = "https://github.com/Balls0fSteel/dc1996-Nagsworth";

  const ranges: readonly { id: number; label: string }[] = [
    { id: 1, label: "Today" },
    { id: 7, label: "7 days" },
    { id: 30, label: "30 days" },
    { id: 365, label: "Last year" },
  ];

  let baseUrl = $state<string | null>(null);
  let health = $state<Health | null>(null);

  let code = $state<{ code: string; expires_at: number } | null>(null);
  let pairing = $state(false);

  /** `undefined` while loading, `null` outside the app. */
  let autostart = $state<boolean | null | undefined>(undefined);
  let autostartBusy = $state(false);

  let exportDays = $state(30);
  let exporting = $state<"csv" | "json" | null>(null);
  let backingUp = $state(false);
  let backupPath = $state<string | null>(null);

  // Server clock minus local clock, so the pairing countdown ticks every second.
  let offset = $state(0);
  let localNow = $state(Date.now());
  const now = $derived(localNow + offset);
  let timer: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    if (snap) offset = snap.now - Date.now();
  });

  onMount(() => {
    timer = setInterval(() => (localNow = Date.now()), 500);
    apiInfo()
      .then((i) => (baseUrl = i.base_url))
      .catch(() => (baseUrl = ""));
    api
      .health()
      .then((h) => (health = h))
      .catch(() => (health = null));
    getAutostart()
      .then((on) => (autostart = on))
      .catch((e) => {
        autostart = null;
        notify.err(e);
      });
  });
  onDestroy(() => clearInterval(timer));

  const left = $derived(code ? code.expires_at - now : 0);
  const expired = $derived(code !== null && left <= 0);
  const fraction = $derived(code ? Math.max(0, Math.min(1, left / CODE_VALID_MS)) : 0);

  const browsers = $derived(
    [...(snap?.extensions ?? [])].sort((a, b) => b[1].last_seen - a[1].last_seen),
  );

  async function pair() {
    if (pairing) return;
    pairing = true;
    try {
      code = await api.startPairing();
    } catch (e) {
      notify.err(e);
    } finally {
      pairing = false;
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      notify.ok("Copied");
    } catch (e) {
      notify.err(e);
    }
  }

  async function toggleAutostart(on: boolean) {
    autostartBusy = true;
    try {
      await setAutostart(on);
      autostart = await getAutostart();
      notify.ok(on ? "Starts when you log in" : "No longer starts at login");
    } catch (e) {
      notify.err(e);
    } finally {
      autostartBusy = false;
    }
  }

  function today(): string {
    if (snap) return snap.today;
    const d = new Date();
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  }

  async function exportData(format: "csv" | "json") {
    if (exporting) return;
    exporting = format;
    try {
      const to = today();
      await downloadExport(format, addDays(to, -(exportDays - 1)), to);
    } catch (e) {
      notify.err(e);
    } finally {
      exporting = null;
    }
  }

  async function backup() {
    if (backingUp) return;
    backingUp = true;
    try {
      backupPath = (await api.backup()).path;
      notify.ok("Database backed up");
    } catch (e) {
      notify.err(e);
    } finally {
      backingUp = false;
    }
  }

  const capitalize = (s: string) => (s ? s[0].toUpperCase() + s.slice(1) : s);
</script>

<div class="settings">
  <Card title="Browser extension" subtitle="Tracks time per website in Chrome, Edge and Firefox.">
    <ol class="steps">
      <li>Install the Nagsworth extension and open its options page.</li>
      <li>Show a pairing code here and type it in. Each code works once and expires after 5 minutes.</li>
    </ol>

    <div class="pairing">
      {#if code === null}
        <button class="btn primary" onclick={pair} disabled={pairing}>
          <Icon name="zap" size={16} />
          Show pairing code
        </button>
      {:else}
        <div class="code-row">
          <span class="code num" class:expired aria-label="Pairing code {code.code.split('').join(' ')}">
            {code.code}
          </span>
          {#if !expired}
            <button class="btn ghost icon" title="Copy code" aria-label="Copy code" onclick={() => code && copy(code.code)}>
              <Icon name="copy" size={16} />
            </button>
          {/if}
        </div>
        <div class="bar code-bar" class:expired>
          <span style:width="{fraction * 100}%"></span>
        </div>
        <div class="code-foot">
          {#if expired}
            <span class="small muted">This code has expired.</span>
            <button class="btn sm" onclick={pair} disabled={pairing}>
              <Icon name="refresh" size={14} />
              New code
            </button>
          {:else}
            <span class="small muted num" aria-live="off">expires in {clock(left)}</span>
          {/if}
        </div>
      {/if}
    </div>

    <h3 class="sub-head">Paired browsers</h3>
    {#if browsers.length === 0}
      <p class="small muted none">No extension has checked in since the app started.</p>
    {:else}
      <ul class="browsers">
        {#each browsers as [id, c] (id)}
          {@const online = snap !== null && snap.now - c.last_seen < ONLINE_MS}
          <li>
            <Icon name="web" size={16} />
            <span class="browser-name">{capitalize(c.browser)}</span>
            {#if c.version}<span class="pill num">{c.version}</span>{/if}
            <span class="small muted push" title={dateTime(c.last_seen)}>
              {relative(c.last_seen, snap?.now)}
            </span>
            {#if online}<span class="pill ok">online</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  </Card>

  <Card title="Startup">
    {#if autostart === undefined}
      <div class="line-placeholder"></div>
    {:else if autostart === null}
      <p class="small muted line">Start at login can only be changed inside the desktop app.</p>
    {:else}
      <div class="line">
        <Switch
          checked={autostart}
          disabled={autostartBusy}
          label="Start when you log in (hidden, in the tray)"
          onchange={toggleAutostart}
        />
      </div>
    {/if}
  </Card>

  <Card title="Your data" subtitle="Everything stays on this computer. Export a range or back up the whole database.">
    <div class="data-row">
      <Segmented options={ranges} value={exportDays} onchange={(d) => (exportDays = d)} label="Export range" />
      <div class="buttons">
        <button class="btn" onclick={() => exportData("csv")} disabled={exporting !== null}>
          <Icon name="download" size={16} />
          Export CSV
        </button>
        <button class="btn" onclick={() => exportData("json")} disabled={exporting !== null}>
          <Icon name="download" size={16} />
          Export JSON
        </button>
        <button class="btn" onclick={backup} disabled={backingUp}>
          <Icon name="shield" size={16} />
          Back up database
        </button>
      </div>
    </div>
    {#if backupPath}
      <div class="path-row">
        <span class="small muted">Saved to</span>
        <code class="mono truncate" title={backupPath}>{backupPath}</code>
        <button class="btn ghost icon sm" title="Copy path" aria-label="Copy backup path" onclick={() => backupPath && copy(backupPath)}>
          <Icon name="copy" size={14} />
        </button>
      </div>
    {/if}
  </Card>

  <Card title="Local API">
    <div class="path-row first">
      {#if baseUrl === null}
        <code class="mono url-placeholder">&nbsp;</code>
      {:else}
        <code class="mono truncate" title={baseUrl}>{baseUrl || "unavailable"}</code>
        {#if baseUrl}
          <button class="btn ghost icon sm" title="Copy URL" aria-label="Copy API URL" onclick={() => baseUrl && copy(baseUrl)}>
            <Icon name="copy" size={14} />
          </button>
        {/if}
      {/if}
    </div>
    <p class="small muted explain">
      The service listens on localhost only. Every route except <code>/health</code> and
      <code>/pair</code> needs the bearer token. The idle threshold, title storage, retention and
      other settings live in the config file; edit it on the
      <a href="#rules" onclick={() => (location.hash = "rules")}>Rules tab</a>.
    </p>
  </Card>

  <Card title="About">
    <dl class="about">
      <dt>Name</dt>
      <dd>{health?.name ?? "Nagsworth"}</dd>
      <dt>Version</dt>
      <dd class="num">{health?.version ?? "-"}</dd>
      <dt>Source</dt>
      <dd>
        <a href={REPO_URL} target="_blank" rel="noopener noreferrer">
          github.com/Balls0fSteel/dc1996-Nagsworth
          <Icon name="external-link" size={13} />
        </a>
      </dd>
      <dt>License</dt>
      <dd>MIT OR Apache-2.0</dd>
    </dl>
  </Card>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    max-width: 760px;
  }
  .push {
    margin-left: auto;
  }

  /* ---- Pairing ---- */
  .steps {
    margin: 0 0 var(--sp-3);
    padding-left: 20px;
    color: var(--text-2);
  }
  .steps li + li {
    margin-top: 2px;
  }
  .pairing {
    min-height: 96px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: var(--sp-2);
    padding: var(--sp-3) var(--sp-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--panel-2);
  }
  .code-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
  }
  .code {
    font-family: var(--mono);
    font-size: 36px;
    line-height: 44px;
    font-weight: 700;
    letter-spacing: 0.3em;
    color: var(--text);
  }
  .code.expired {
    color: var(--muted);
    text-decoration: line-through;
  }
  .code-bar {
    width: min(320px, 100%);
    height: 4px;
  }
  .code-bar.expired > span {
    background: var(--muted);
  }
  .code-foot {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-height: 24px;
  }
  .sub-head {
    margin: var(--sp-4) 0 var(--sp-2);
  }
  .none {
    margin: 0;
  }
  .browsers {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .browsers li {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    padding: 7px 0;
    border-bottom: 1px solid var(--border);
    color: var(--text-2);
  }
  .browsers li:last-child {
    border-bottom: none;
  }
  .browser-name {
    font-weight: 500;
    color: var(--text);
  }

  /* ---- Startup ---- */
  .line,
  .line-placeholder {
    min-height: 22px;
    margin: 0;
    display: flex;
    align-items: center;
  }

  /* ---- Data ---- */
  .data-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  .path-row {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin-top: var(--sp-3);
    min-width: 0;
  }
  .path-row.first {
    margin-top: 0;
  }
  .path-row code {
    min-width: 0;
    padding: 2px 6px;
  }
  .url-placeholder {
    min-width: 260px;
  }
  .explain {
    margin: var(--sp-3) 0 0;
  }

  /* ---- About ---- */
  .about {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px var(--sp-5);
    margin: 0;
  }
  .about dt {
    color: var(--muted);
  }
  .about dd {
    margin: 0;
    min-width: 0;
  }
  .about a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
</style>
