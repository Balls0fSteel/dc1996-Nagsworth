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
  import {
    INPUT_WINDOW_MAX,
    INPUT_WINDOW_MIN,
    readHandsOn,
    withHandsOn,
    type HandsOnSettings,
  } from "../lib/toml";
  import { updates } from "../lib/update.svelte";
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

  /** The `[tracking]` hands-on keys from the config file; `undefined` while loading, `null` if it failed. */
  let handsOn = $state<HandsOnSettings | null | undefined>(undefined);
  let handsOnBusy = $state(false);

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
    api
      .rules()
      .then((r) => (handsOn = readHandsOn(r.toml)))
      .catch((e) => {
        handsOn = null;
        notify.err(e);
      });
    void updates.start();
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

  /** Edits `[tracking]` in the config file as it is now on disk and saves it. */
  async function saveHandsOn(patch: Partial<HandsOnSettings>, done: string) {
    if (handsOnBusy) return;
    handsOnBusy = true;
    try {
      const { toml } = await api.rules();
      const r = await api.saveRules(withHandsOn(toml, patch));
      handsOn = readHandsOn(r.toml);
      notify.ok(done);
    } catch (e) {
      notify.err(e);
      handsOn = await api
        .rules()
        .then((r) => readHandsOn(r.toml))
        .catch(() => handsOn ?? null);
    } finally {
      handsOnBusy = false;
    }
  }

  function setInputWindow(input: HTMLInputElement) {
    if (!handsOn) return;
    const n = Math.round(Number(input.value));
    if (!Number.isFinite(n) || n < INPUT_WINDOW_MIN || n > INPUT_WINDOW_MAX) {
      notify.err(`Seconds per input must be between ${INPUT_WINDOW_MIN} and ${INPUT_WINDOW_MAX}`);
      input.value = String(handsOn.input_window_s);
      return;
    }
    if (n === handsOn.input_window_s) {
      input.value = String(n);
      return;
    }
    void saveHandsOn({ input_window_s: n }, `Each input counts ${n} s; restart to apply`);
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

  // ---- Updates ----
  const upd = $derived(updates.status);
  const mb = (n: number) => `${(n / 1_048_576).toFixed(1)} MB`;
  const downloadText = $derived.by(() => {
    const d = upd?.download;
    if (!d) return "";
    return d.total
      ? `${Math.floor((d.got * 100) / Math.max(1, d.total))} % of ${mb(d.total)}`
      : mb(d.got);
  });
  const downloadFraction = $derived(
    upd?.download?.total ? Math.min(1, upd.download.got / upd.download.total) : 0,
  );
  /** One line on where things stand. */
  const updateText = $derived.by((): string => {
    if (!upd) return "";
    if (!upd.enabled) return "This build does not check for updates.";
    if (upd.checking) return "Checking for updates…";
    if (upd.ready) return `Nagsworth ${upd.ready} is downloaded and checked.`;
    if (upd.download) return `Downloading Nagsworth ${upd.available?.version ?? ""}…`;
    if (upd.available) return `Nagsworth ${upd.available.version} is available.`;
    if (upd.error) return "The last check failed.";
    if (upd.last_check) return `Nagsworth ${upd.current} is the newest version.`;
    return `Nagsworth ${upd.current}. Not checked yet.`;
  });
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

  <Card title="Tracking">
    {#if handsOn === undefined}
      <div class="line-placeholder"></div>
    {:else if handsOn === null}
      <p class="small muted line">The config file could not be read; edit it on the Rules tab.</p>
    {:else}
      <div class="line">
        <Switch
          checked={handsOn.hands_on_games}
          disabled={handsOnBusy}
          label="Count hands-on gaming time"
          onchange={(on) =>
            saveHandsOn(
              { hands_on_games: on },
              on ? "Game rules count hands-on time" : "Game rules count the whole time in front",
            )}
        />
      </div>
      <p class="small muted explain sub">
        Game rules count only the seconds after a key press, mouse move or controller input, not
        the whole time a game is in front.
      </p>
      <div class="window-row">
        <label for="input-window">Seconds per input</label>
        <input
          id="input-window"
          class="num window-input"
          type="number"
          min={INPUT_WINDOW_MIN}
          max={INPUT_WINDOW_MAX}
          step="1"
          value={handsOn.input_window_s}
          disabled={handsOnBusy}
          onchange={(e) => setInputWindow(e.currentTarget)}
        />
        <span class="small muted">Takes effect after a restart.</span>
      </div>
      <p class="small muted explain">
        Controllers: XInput on Windows. On Linux only keyboard and mouse count.
      </p>
    {/if}
  </Card>

  <Card title="Updates">
    {#if upd === null}
      <p class="small muted line">Updates are checked and installed inside the desktop app.</p>
    {:else}
      <div class="update-row">
        <span class="update-text" class:muted={!upd.available && !upd.ready}>{updateText}</span>
        {#if upd.enabled && !upd.download}
          <button class="btn sm" onclick={updates.check} disabled={upd.checking}>
            <Icon name="refresh" size={14} />
            Check now
          </button>
        {/if}
      </div>
      {#if upd.error}
        <p class="small update-error">{upd.error}</p>
      {/if}
      {#if upd.download}
        <div class="update-progress">
          <div class="bar update-bar" class:indeterminate={!upd.download.total}>
            <span style:width="{(upd.download.total ? downloadFraction : 1) * 100}%"></span>
          </div>
          <span class="small muted num">{downloadText}</span>
          <button class="btn sm ghost" onclick={updates.cancel}>Cancel</button>
        </div>
      {:else if upd.ready}
        <div class="buttons update-actions">
          <button class="btn primary" onclick={updates.install}>
            <Icon name="zap" size={16} />
            Install and restart
          </button>
          <button class="btn" onclick={updates.openReleasePage}>
            <Icon name="external-link" size={16} />
            What's new
          </button>
        </div>
        <p class="small muted explain">
          Nagsworth closes, the setup replaces it in place and starts the new version in the tray.
          Your data and settings stay.
        </p>
      {:else if upd.available}
        <div class="buttons update-actions">
          {#if upd.can_install}
            <button class="btn primary" onclick={updates.download}>
              <Icon name="download" size={16} />
              Download update
            </button>
            <button class="btn" onclick={updates.openReleasePage}>
              <Icon name="external-link" size={16} />
              What's new
            </button>
          {:else}
            <button class="btn primary" onclick={updates.openReleasePage}>
              <Icon name="external-link" size={16} />
              Open the release page
            </button>
          {/if}
        </div>
        {#if !upd.can_install}
          <p class="small muted explain">
            This copy was not installed by the setup, so it cannot update itself. Download the
            new version from the release page.
          </p>
        {/if}
      {/if}
      {#if upd.enabled}
        <div class="update-switches">
          <Switch
            checked={upd.auto_check}
            label="Check for updates automatically (twice a day)"
            onchange={updates.setAuto}
          />
          <Switch checked={upd.beta} label="Include pre-releases" onchange={updates.setBeta} />
        </div>
        <p class="small muted explain">
          A check asks GitHub for the list of releases{upd.last_check
            ? `; the last one answered ${relative(upd.last_check, snap?.now)}`
            : ""}. The setup is checked against the release's SHA256SUMS before it runs.
        </p>
      {/if}
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

  /* ---- Tracking ---- */
  .explain.sub {
    margin-top: var(--sp-1);
    padding-left: 40px;
  }
  .window-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    margin-top: var(--sp-4);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
  }
  .window-input {
    width: 80px;
  }

  /* ---- Updates ---- */
  .update-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-2) var(--sp-3);
    min-height: 28px;
  }
  .update-text {
    font-weight: 500;
  }
  .update-text.muted {
    font-weight: 400;
  }
  .update-error {
    margin: var(--sp-1) 0 0;
    color: var(--danger);
    word-break: break-word;
  }
  .update-progress {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
  }
  .update-bar {
    flex: 1;
    height: 6px;
  }
  .update-bar.indeterminate > span {
    opacity: 0.5;
  }
  .update-actions {
    margin-top: var(--sp-3);
  }
  .update-switches {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-4);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--border);
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
