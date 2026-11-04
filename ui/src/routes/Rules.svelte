<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import {
    api,
    type EscalationStep,
    type NagLevel,
    type NagLogEntry,
    type RuleDef,
    type RuleSnapshot,
    type RulesResponse,
    type StateResponse,
    type TriggerDef,
  } from "../lib/api";
  import { clock, dateTime, duration, relative } from "../lib/format";
  import { errorText, notify } from "../lib/notify.svelte";
  import { usePageToolbar } from "../lib/page.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Empty from "../lib/ui/Empty.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Segmented from "../lib/ui/Segmented.svelte";
  import Switch from "../lib/ui/Switch.svelte";

  type View = "rules" | "file";
  const views: readonly { id: View; label: string }[] = [
    { id: "rules", label: "Rules" },
    { id: "file", label: "Config file" },
  ];

  const POLL_MS = 2000;
  const NAGS_EVERY_MS = 10_000;
  /** Must match the editor's CSS line-height. */
  const LINE_PX = 20;

  let view = $state<View>("rules");

  // The file as the service has it, the editor text, and what parsed out of it.
  let saved = $state("");
  let text = $state("");
  let rules = $state<RuleDef[] | null>(null);
  /** The service's parse error for the file on disk (`error` in the response). */
  let fileError = $state<string | null>(null);
  /** A rejected save from the editor. */
  let saveError = $state<string | null>(null);
  let loadError = $state<string | null>(null);
  let saving = $state(false);

  let snap = $state<StateResponse | null>(null);
  /** Server clock minus local clock, so countdowns tick between polls. */
  let offset = $state(0);
  let localNow = $state(Date.now());
  const now = $derived(localNow + offset);

  let nags = $state<NagLogEntry[] | null>(null);
  let nagsAt = 0;

  let ta = $state<HTMLTextAreaElement | null>(null);
  let gutter = $state<HTMLPreElement | null>(null);
  /** After Escape, the next Tab leaves the editor instead of indenting. */
  let tabLeaves = false;

  let pollTimer: ReturnType<typeof setInterval> | undefined;
  let tickTimer: ReturnType<typeof setInterval> | undefined;

  const dirty = $derived(text !== saved);
  const lineCount = $derived(text.split("\n").length);
  const gutterText = $derived(
    Array.from({ length: lineCount }, (_, i) => String(i + 1)).join("\n"),
  );
  const snapshots = $derived(new Map((snap?.engine.rules ?? []).map((r) => [r.id, r])));
  const ruleNames = $derived(new Map((rules ?? []).map((r) => [r.id, r.name ?? r.id])));

  $effect(() => usePageToolbar(tools));

  // ---- Loading ----

  function apply(r: RulesResponse) {
    const wasDirty = text !== saved;
    saved = r.toml;
    if (!wasDirty) text = r.toml;
    rules = r.rules;
    fileError = r.error ?? null;
  }

  async function load() {
    try {
      apply(await api.rules());
      loadError = null;
    } catch (e) {
      loadError = errorText(e);
      rules ??= [];
    }
  }

  async function loadNags() {
    try {
      nags = await api.nags(50);
      nagsAt = Date.now();
    } catch {
      nags ??= [];
    }
  }

  async function poll() {
    try {
      const s = await api.state();
      offset = s.now - Date.now();
      snap = s;
    } catch {
      // The shell shows connection problems.
    }
    if (Date.now() - nagsAt >= NAGS_EVERY_MS) void loadNags();
  }

  onMount(() => {
    void load();
    void poll();
    pollTimer = setInterval(poll, POLL_MS);
    tickTimer = setInterval(() => (localNow = Date.now()), 1000);
  });
  onDestroy(() => {
    clearInterval(pollTimer);
    clearInterval(tickTimer);
  });

  // ---- TOML editing helpers ----

  interface Block {
    id: string | null;
    /** Line index of `[[rules]]`. */
    header: number;
    /** Line index of `id = ...`, -1 when missing. */
    idLine: number;
    /** First line after the block. */
    end: number;
  }

  const RULES_HEADER = /^\s*\[\[\s*rules\s*\]\]\s*(#.*)?$/;
  const ANY_HEADER = /^\s*\[\[?\s*[A-Za-z0-9_.\-"' ]+\s*\]\]?\s*(#.*)?$/;
  const ID_LINE = /^\s*id\s*=\s*(["'])(.*?)\1/;
  const ENABLED_LINE = /^(\s*enabled\s*=\s*)(true|false)\b/;

  function ruleBlocks(lines: string[]): Block[] {
    const out: Block[] = [];
    for (let i = 0; i < lines.length; i++) {
      if (!RULES_HEADER.test(lines[i])) continue;
      let end = i + 1;
      while (end < lines.length && !ANY_HEADER.test(lines[end])) end++;
      let id: string | null = null;
      let idLine = -1;
      for (let j = i + 1; j < end; j++) {
        const m = ID_LINE.exec(lines[j]);
        if (m) {
          id = m[2];
          idLine = j;
          break;
        }
      }
      out.push({ id, header: i, idLine, end });
      i = end - 1;
    }
    return out;
  }

  /** Sets or inserts `enabled = on` in the `[[rules]]` block with this id; null if not found. */
  function withEnabled(src: string, id: string, on: boolean): string | null {
    const lines = src.split("\n");
    const block = ruleBlocks(lines).find((b) => b.id === id);
    if (!block) return null;
    for (let j = block.header + 1; j < block.end; j++) {
      if (ENABLED_LINE.test(lines[j])) {
        lines[j] = lines[j].replace(ENABLED_LINE, `$1${on}`);
        return lines.join("\n");
      }
    }
    const idText = lines[block.idLine];
    const indent = /^\s*/.exec(idText)?.[0] ?? "";
    const eol = idText.endsWith("\r") ? "\r" : "";
    lines.splice(block.idLine + 1, 0, `${indent}enabled = ${on}${eol}`);
    return lines.join("\n");
  }

  async function toggle(rule: RuleDef, on: boolean) {
    const next = withEnabled(saved, rule.id, on);
    if (next === null) {
      notify.err(`Could not find the [[rules]] block with id "${rule.id}"`);
      return;
    }
    const editorNext = dirty ? withEnabled(text, rule.id, on) : null;
    saving = true;
    try {
      const r = await api.saveRules(next);
      if (editorNext !== null) text = editorNext;
      apply(r);
      notify.ok(`${rule.name ?? rule.id} ${on ? "enabled" : "disabled"}`);
      void poll();
    } catch (e) {
      notify.err(e);
      await load();
    } finally {
      saving = false;
    }
  }

  async function editInFile(id: string) {
    view = "file";
    await tick();
    if (!ta) return;
    const lines = text.split("\n");
    const block = ruleBlocks(lines).find((b) => b.id === id);
    const line = block?.header ?? 0;
    let start = 0;
    for (let i = 0; i < line; i++) start += lines[i].length + 1;
    const end = start + (lines[line]?.length ?? 0);
    ta.scrollIntoView({ block: "nearest" });
    ta.focus({ preventScroll: true });
    ta.setSelectionRange(start, end);
    const lh = parseFloat(getComputedStyle(ta).lineHeight) || LINE_PX;
    ta.scrollTop = Math.max(0, line * lh);
    syncScroll();
  }

  // ---- Editor ----

  async function save() {
    if (saving) return;
    saving = true;
    try {
      const r = await api.saveRules(text);
      saved = text;
      apply(r);
      text = r.toml;
      saveError = null;
      notify.ok("Saved and applied");
      void poll();
    } catch (e) {
      saveError = errorText(e);
    } finally {
      saving = false;
    }
  }

  function revert() {
    text = saved;
    saveError = null;
  }

  function syncScroll() {
    if (ta && gutter) gutter.scrollTop = ta.scrollTop;
  }

  function onEditorKey(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLTextAreaElement;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (dirty) void save();
      return;
    }
    if (e.key === "Escape") {
      tabLeaves = true;
      return;
    }
    if (e.key === "Tab" && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
      if (tabLeaves) {
        tabLeaves = false;
        return;
      }
      e.preventDefault();
      el.setRangeText("  ", el.selectionStart, el.selectionEnd, "end");
      text = el.value;
      return;
    }
    tabLeaves = false;
  }

  // ---- Wording ----

  function triggerText(t: TriggerDef): string {
    switch (t.kind) {
      case "interval":
        return `Every ${t.every}, then a ${t.break_for} break`;
      case "daily_total":
        return `${t.limit} per day, nags again every ${t.renag_every}`;
      case "continuous":
        return `At most ${t.limit} in one go, resets after ${t.reset_after} rest`;
      case "schedule":
        return t.windows.length ? `Blocked ${t.windows.join(", ")}` : "No blocked windows";
    }
  }

  const triggerIcon = {
    interval: "coffee",
    daily_total: "clock",
    continuous: "zap",
    schedule: "lock",
  } as const;

  function scopeLabel(r: RuleDef): { text: string; mono: boolean } {
    if (r.scope === "any") return { text: "Any app", mono: false };
    if ("category" in r.scope) return { text: r.scope.category, mono: false };
    return { text: r.scope.app, mono: true };
  }

  function idleLabel(r: RuleDef): { text: string; isDefault: boolean } {
    const isGame = r.scope !== "any" && "category" in r.scope && r.scope.category === "Game";
    const policy = r.idle_policy ?? (isGame ? "ignore" : "pause");
    return {
      text: policy === "pause" ? "idle pauses" : "idle counts",
      isDefault: r.idle_policy === null,
    };
  }

  const levelTone: Record<NagLevel, string> = { toast: "accent", overlay: "warn", lock: "danger" };

  function isZero(d: string): boolean {
    return /^\s*0+\s*[a-z]*\s*$/i.test(d);
  }

  function stepText(s: EscalationStep): string {
    return isZero(s.after) ? s.level : `${s.level} after ${s.after}`;
  }

  function snoozeText(r: RuleDef): string {
    const s = r.snooze ?? { duration: "5m", max_per_day: 3 };
    if (s.max_per_day === 0 || isZero(s.duration)) return "No snooze";
    return `Snooze ${s.duration}, ${s.max_per_day} per day`;
  }

  interface Live {
    text: string;
    tone: string;
  }

  function liveState(r: RuleDef, s: RuleSnapshot | undefined): Live | null {
    if (!r.enabled || s?.enabled === false) return { text: "Off", tone: "" };
    if (!s) return null;
    const st = s.state;
    switch (st.state) {
      case "idle":
        return { text: "Idle", tone: "" };
      case "pending":
        return { text: "Due", tone: "warn" };
      case "active":
        return { text: `Nagging ${st.level}`, tone: levelTone[st.level] };
      case "snoozed":
        return { text: `Snoozed ${clock(st.until - now)}`, tone: "accent" };
      case "in_break":
        return { text: `Break ${clock(st.until - now)}`, tone: "ok" };
      case "satisfied":
        return { text: "Over limit", tone: "danger" };
    }
  }

  const nagTone: Record<string, string> = {
    raise: "accent",
    escalate: "warn",
    dismiss: "ok",
    snooze: "",
    skip: "",
  };
</script>

{#snippet tools()}
  <Segmented options={views} value={view} onchange={(v) => (view = v)} label="Rules view" />
{/snippet}

{#if view === "rules"}
  {#if loadError}
    <div class="banner danger top" role="alert">
      <Icon name="alert" size={16} />
      <span>Could not load the rules: {loadError}</span>
      <button class="btn sm push" onclick={load}>Retry</button>
    </div>
  {/if}
  {#if fileError}
    <div class="banner danger top block-banner" role="alert">
      <div class="banner-head">
        <Icon name="alert" size={16} />
        <strong>The config file does not parse, so no rules are active.</strong>
        <button class="btn sm push" onclick={() => (view = "file")}>Open config file</button>
      </div>
      <pre class="err-text">{fileError}</pre>
    </div>
  {/if}

  {#if rules === null}
    <div class="grid" aria-busy="true">
      <div class="card skeleton"></div>
      <div class="card skeleton"></div>
    </div>
  {:else if rules.length === 0}
    {#if !fileError && !loadError}
      <Card>
        <Empty icon="rules" title="No rules yet" hint="Add a [[rules]] table to the config file; its comments explain every trigger.">
          <button class="btn primary" onclick={() => (view = "file")}>Open config file</button>
        </Empty>
      </Card>
    {/if}
  {:else}
    <div class="grid">
      {#each rules as rule (rule.id)}
        {@const s = snapshots.get(rule.id)}
        {@const live = liveState(rule, s)}
        {@const scope = scopeLabel(rule)}
        {@const idle = idleLabel(rule)}
        <Card title={rule.name ?? rule.id} subtitle={rule.name ? rule.id : undefined} class="rule-card {rule.enabled ? '' : 'off'}">
          {#snippet actions()}
            <Switch
              checked={rule.enabled}
              disabled={saving}
              title={rule.enabled ? `Disable ${rule.name ?? rule.id}` : `Enable ${rule.name ?? rule.id}`}
              onchange={(on) => toggle(rule, on)}
            />
          {/snippet}
          <div class="rule">
            <div class="trigger">
              <Icon name={triggerIcon[rule.trigger.kind]} size={16} />
              <span>{triggerText(rule.trigger)}</span>
            </div>

            <div class="chips">
              <span class="pill" class:mono={scope.mono} title="Scope">{scope.text}</span>
              <span class="pill" title={idle.isDefault ? "Idle policy (default)" : "Idle policy"}>{idle.text}</span>
            </div>

            <div class="ladder" aria-label="Escalation">
              {#each rule.escalation as step, i (i)}
                {#if i > 0}<span class="arrow" aria-hidden="true">→</span>{/if}
                <span class="pill {levelTone[step.level]}">{stepText(step)}</span>
              {:else}
                <span class="muted small">No escalation steps</span>
              {/each}
            </div>

            <div class="small muted snooze">
              <Icon name="snooze" size={14} />
              <span>
                {snoozeText(rule)}{#if s && rule.enabled && snoozeText(rule) !== "No snooze"}, {s.snoozes_left} left today{/if}
              </span>
            </div>

            {#if rule.message}
              <blockquote class="message">{rule.message}</blockquote>
            {/if}

            <div class="live">
              <div class="live-row">
                {#if live}
                  <span class="pill {live.tone} num">{live.text}</span>
                {:else}
                  <span class="pill">-</span>
                {/if}
                {#if s && rule.trigger.kind !== "schedule" && s.target_ms > 0}
                  <span class="small muted num progress-text">
                    {duration(s.accum_ms)} / {duration(s.target_ms)}
                  </span>
                {/if}
                <button class="btn ghost sm push" onclick={() => editInFile(rule.id)}>
                  <Icon name="pencil" size={14} />
                  Edit in file
                </button>
              </div>
              {#if rule.trigger.kind !== "schedule"}
                {@const frac = s && s.target_ms > 0 ? Math.min(1, s.accum_ms / s.target_ms) : 0}
                <div
                  class="bar"
                  class:over={frac >= 1}
                  role="progressbar"
                  aria-label="Progress toward the limit"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(frac * 100)}
                >
                  <span style:width="{frac * 100}%"></span>
                </div>
              {/if}
            </div>
          </div>
        </Card>
      {/each}
    </div>
  {/if}
{:else}
  <Card
    title="config.toml"
    subtitle="The whole config file. Saving validates it and applies it without a restart."
  >
    {#snippet actions()}
      {#if dirty}<span class="pill warn">Unsaved changes</span>{/if}
      <button class="btn" onclick={revert} disabled={!dirty || saving}>Revert</button>
      <button class="btn primary" onclick={save} disabled={!dirty || saving}>
        Save <span class="kbd save-kbd">Ctrl S</span>
      </button>
    {/snippet}
    {#if saveError ?? fileError}
      <div class="banner danger block-banner editor-error" role="alert">
        <div class="banner-head">
          <Icon name="alert" size={16} />
          <strong>{saveError ? "Not saved: the file has an error" : "The file on disk does not parse"}</strong>
        </div>
        <pre class="err-text">{saveError ?? fileError}</pre>
      </div>
    {/if}
    <div class="editor">
      <pre class="gutter" bind:this={gutter} aria-hidden="true">{gutterText}</pre>
      <textarea
        bind:this={ta}
        bind:value={text}
        onscroll={syncScroll}
        onkeydown={onEditorKey}
        spellcheck="false"
        autocomplete="off"
        wrap="off"
        aria-label="Config file"
      ></textarea>
    </div>
    <p class="small muted hint">
      Tab indents; press <span class="kbd">Esc</span> then <span class="kbd">Tab</span> to leave the
      editor.
    </p>
  </Card>
{/if}

<div class="history">
  <Card title="Nag history" subtitle="The latest 50 nags" padded={false}>
    {#if nags === null}
      <div class="nags-placeholder"></div>
    {:else if nags.length === 0}
      <Empty icon="snooze" title="No nags yet" hint="Raised, escalated and dismissed nags show up here." />
    {:else}
      <table class="table">
        <thead>
          <tr>
            <th>When</th>
            <th>Rule</th>
            <th>Event</th>
            <th>Detail</th>
          </tr>
        </thead>
        <tbody>
          {#each nags as n (n.id)}
            <tr>
              <td class="num nowrap" title={dateTime(n.at)}>{relative(n.at, now)}</td>
              <td>
                <span class="truncate rule-name" title={n.rule_id}>{ruleNames.get(n.rule_id) ?? n.rule_id}</span>
              </td>
              <td>
                <span class="pill {nagTone[n.kind] ?? ''}" class:quiet={n.kind === "snooze" || n.kind === "skip"}>
                  {n.kind.replaceAll("_", " ")}
                </span>
              </td>
              <td class="muted">{n.level ?? n.reason ?? ""}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </Card>
</div>

<style>
  .top {
    margin-bottom: var(--sp-4);
  }
  .push {
    margin-left: auto;
  }
  .block-banner {
    flex-direction: column;
    align-items: stretch;
    gap: var(--sp-2);
  }
  .banner-head {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--danger);
  }
  .banner-head strong {
    color: var(--text);
    font-weight: 600;
  }
  .err-text {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-size: 12.5px;
    color: var(--text);
  }
  .editor-error {
    margin-bottom: var(--sp-3);
  }

  /* ---- Rule cards ---- */
  .grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--sp-4);
  }
  @media (min-width: 900px) {
    .grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .grid :global(.card-sub) {
    font-family: var(--mono);
  }
  .grid :global(.rule-card.off .card-titles h2) {
    color: var(--muted);
  }
  .skeleton {
    min-height: 240px;
    background: var(--panel-2);
  }
  .rule {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100%;
  }
  .trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 500;
    color: var(--text);
  }
  .trigger :global(svg) {
    color: var(--muted);
  }
  .chips,
  .ladder {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .arrow {
    color: var(--muted);
    font-size: 12px;
  }
  .snooze {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .message {
    margin: 0;
    padding: 6px 10px;
    border-left: 3px solid var(--border-strong);
    background: var(--panel-2);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    color: var(--text-2);
    font-style: italic;
    overflow-wrap: anywhere;
  }
  .live {
    margin-top: auto;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .live-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 24px;
  }
  .progress-text {
    white-space: nowrap;
  }
  .bar.over > span {
    background: var(--danger);
  }

  /* ---- Editor ---- */
  .editor {
    display: flex;
    min-height: 60vh;
    height: 60vh;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--panel);
    overflow: hidden;
    resize: vertical;
  }
  .editor:focus-within {
    border-color: var(--accent);
    box-shadow: var(--focus);
  }
  .gutter,
  .editor textarea {
    font-family: var(--mono);
    font-size: 13px;
    line-height: 20px;
    padding-top: 10px;
    padding-bottom: 10px;
    margin: 0;
  }
  .gutter {
    flex: none;
    min-width: 3.5em;
    padding-left: 10px;
    padding-right: 10px;
    /* Room to follow the textarea when its horizontal scrollbar shows. */
    padding-bottom: 40px;
    text-align: right;
    color: var(--muted);
    background: var(--panel-2);
    border-right: 1px solid var(--border);
    overflow: hidden;
    user-select: none;
    font-variant-numeric: tabular-nums;
  }
  .editor textarea {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding-left: 12px;
    padding-right: 12px;
    border: none;
    border-radius: 0;
    resize: none;
    white-space: pre;
    overflow: auto;
    tab-size: 2;
    background: transparent;
  }
  .editor textarea:focus {
    box-shadow: none;
  }
  .save-kbd {
    background: transparent;
    border-color: color-mix(in srgb, var(--accent-text) 40%, transparent);
    color: inherit;
    opacity: 0.8;
  }
  .hint {
    margin: 8px 0 0;
  }

  /* ---- History ---- */
  .history {
    margin-top: var(--sp-5);
  }
  .nags-placeholder {
    min-height: 140px;
  }
  .nowrap {
    white-space: nowrap;
  }
  .rule-name {
    display: block;
    max-width: 320px;
  }
  .pill.quiet {
    color: var(--muted);
  }
</style>
