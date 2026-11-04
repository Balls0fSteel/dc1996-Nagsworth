<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppRow, type Category, type CategoryKind } from "../lib/api";
  import { categoryColor, dateTime, relative } from "../lib/format";
  import { notify } from "../lib/notify.svelte";
  import { usePageToolbar } from "../lib/page.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Empty from "../lib/ui/Empty.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Segmented from "../lib/ui/Segmented.svelte";
  import Switch from "../lib/ui/Switch.svelte";

  type KindFilter = "all" | CategoryKind;
  type SortKey = "last_seen" | "name" | "category";

  const KINDS: readonly CategoryKind[] = ["Work", "Game", "Web", "Other"];
  const kindOptions: readonly { id: KindFilter; label: string }[] = [
    { id: "all", label: "All" },
    ...KINDS.map((k) => ({ id: k, label: k })),
  ];

  let apps = $state<AppRow[]>([]);
  let categories = $state<Category[]>([]);
  let loaded = $state(false);
  let loadError = $state<string | null>(null);
  let now = $state(Date.now());

  let query = $state("");
  let kind = $state<KindFilter>("all");
  let onlyGames = $state(false);
  let sort = $state<SortKey>("last_seen");

  /** App whose display name is being edited, and the draft text. */
  let editingId = $state<number | null>(null);
  let draft = $state("");
  /** Apps with a request in flight (controls are disabled meanwhile). */
  let busy = $state<Set<number>>(new Set());

  let newName = $state("");
  let newKind = $state<CategoryKind>("Work");
  let newColor = $state("");
  let colorTouched = $state(false);
  let adding = $state(false);

  const categoryById = $derived(new Map(categories.map((c) => [c.id, c])));

  const appCounts = $derived.by(() => {
    const m = new Map<number, number>();
    for (const a of apps) m.set(a.category_id, (m.get(a.category_id) ?? 0) + 1);
    return m;
  });

  const nameOf = (a: AppRow) => a.display_name ?? a.exe_name;

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = apps.filter(
      (a) =>
        (kind === "all" || a.category_kind === kind) &&
        (!onlyGames || a.is_game) &&
        (!q ||
          `${a.exe_name} ${a.display_name ?? ""} ${a.category} ${a.exe_path ?? ""}`
            .toLowerCase()
            .includes(q)),
    );
    const byName = (x: AppRow, y: AppRow) =>
      nameOf(x).localeCompare(nameOf(y), undefined, { sensitivity: "base" });
    if (sort === "name") list.sort(byName);
    else if (sort === "category")
      list.sort((x, y) => x.category.localeCompare(y.category) || byName(x, y));
    else list.sort((x, y) => y.last_seen - x.last_seen);
    return list;
  });

  const filtering = $derived(query.trim() !== "" || kind !== "all" || onlyGames);
  const gameCount = $derived(shown.filter((a) => a.is_game).length);
  const countLine = $derived(
    `${filtering ? `${shown.length} of ${apps.length}` : apps.length} ${apps.length === 1 && !filtering ? "app" : "apps"}, ` +
      `${gameCount} ${gameCount === 1 ? "game" : "games"}`,
  );

  function swatchOf(a: AppRow): string {
    return categoryById.get(a.category_id)?.color ?? categoryColor[a.category_kind];
  }

  /** The kind's token color as hex, for the color input's starting value. */
  function kindHex(k: CategoryKind): string {
    const v = getComputedStyle(document.documentElement)
      .getPropertyValue(`--cat-${k.toLowerCase()}`)
      .trim();
    return /^#[0-9a-f]{6}$/i.test(v) ? v : "#808080";
  }

  $effect(() => {
    if (!colorTouched) newColor = kindHex(newKind);
  });

  async function load() {
    try {
      [apps, categories] = await Promise.all([api.apps(), api.categories()]);
      now = Date.now();
      loadError = null;
      loaded = true;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    }
  }

  function setBusy(id: number, on: boolean) {
    const s = new Set(busy);
    if (on) s.add(id);
    else s.delete(id);
    busy = s;
  }

  async function update(a: AppRow, patch: Parameters<typeof api.updateApp>[1]) {
    setBusy(a.id, true);
    try {
      const updated = await api.updateApp(a.id, patch);
      apps = apps.map((x) => (x.id === updated.id ? updated : x));
      notify.ok("Saved");
    } catch (e) {
      notify.err(e);
    } finally {
      setBusy(a.id, false);
    }
  }

  function startEdit(a: AppRow) {
    editingId = a.id;
    draft = a.display_name ?? "";
  }

  function commitEdit(a: AppRow) {
    if (editingId !== a.id) return;
    editingId = null;
    const value = draft.trim() || null;
    if (value === a.display_name) return;
    void update(a, { display_name: value });
  }

  function editKey(e: KeyboardEvent, a: AppRow) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitEdit(a);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      editingId = null;
    }
  }

  function focusSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  async function addCategory(e: SubmitEvent) {
    e.preventDefault();
    const name = newName.trim();
    if (!name) return;
    adding = true;
    try {
      await api.createCategory(name, newKind, colorTouched ? newColor : undefined);
      newName = "";
      colorTouched = false;
      categories = await api.categories();
      notify.ok("Saved");
    } catch (err) {
      notify.err(err);
    } finally {
      adding = false;
    }
  }

  onMount(() => {
    void load();
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(t);
  });

  $effect(() => usePageToolbar(tools));
</script>

{#snippet tools()}
  <div class="search">
    <span class="search-icon"><Icon name="search" size={15} /></span>
    <input type="search" placeholder="Search apps" aria-label="Search apps" bind:value={query} />
  </div>
  <Segmented label="Category kind" options={kindOptions} value={kind} onchange={(k) => (kind = k)} />
  <Switch label="Only games" checked={onlyGames} onchange={(v) => (onlyGames = v)} />
  <select aria-label="Sort apps" bind:value={sort}>
    <option value="last_seen">Last seen</option>
    <option value="name">Name</option>
    <option value="category">Category</option>
  </select>
{/snippet}

<div class="page">
  <div class="count muted small num" aria-live="polite">
    {#if loaded}{countLine}{:else}&nbsp;{/if}
  </div>

  <Card
    padded={false}
    title="Apps"
    subtitle="New apps get a category automatically. A category you pick here is never overwritten."
  >
    {#if loadError && !loaded}
      <Empty icon="alert" title="Couldn't load apps" hint={loadError}>
        <button class="btn sm" onclick={load}><Icon name="refresh" size={14} /> Retry</button>
      </Empty>
    {:else if !loaded}
      <div class="skeleton" aria-busy="true" aria-label="Loading apps">
        {#each [0, 1, 2, 3, 4] as i (i)}
          <div class="sk-row"><span class="sk sk-a"></span><span class="sk sk-b"></span></div>
        {/each}
      </div>
    {:else if apps.length === 0}
      <Empty
        icon="apps"
        title="No apps yet"
        hint="Apps show up here once Nagsworth has seen them in the foreground."
      />
    {:else if shown.length === 0}
      <Empty icon="search" title="No matching apps" hint="Try another search or filter." />
    {:else}
      <div class="scroll">
        <table class="table">
          <thead>
            <tr>
              <th>App</th>
              <th class="w-cat">Category</th>
              <th class="w-game">Game</th>
              <th class="w-src">Source</th>
              <th class="w-seen">Last seen</th>
            </tr>
          </thead>
          <tbody>
            {#each shown as a (a.id)}
              {@const isBusy = busy.has(a.id)}
              <tr>
                <td class="app">
                  <div class="app-cell">
                    <span class="swatch" style:background={swatchOf(a)}></span>
                    <div class="app-text">
                      {#if editingId === a.id}
                        <input
                          class="name-input"
                          bind:value={draft}
                          placeholder={a.exe_name}
                          aria-label="Display name for {a.exe_name}"
                          onkeydown={(e) => editKey(e, a)}
                          onblur={() => commitEdit(a)}
                          {@attach focusSelect}
                        />
                      {:else}
                        <div class="name-row">
                          <button
                            class="name truncate"
                            title="Rename"
                            disabled={isBusy}
                            onclick={() => startEdit(a)}
                          >
                            {nameOf(a)}
                          </button>
                          <button
                            class="btn ghost icon sm pencil"
                            aria-label="Rename {nameOf(a)}"
                            disabled={isBusy}
                            onclick={() => startEdit(a)}
                          >
                            <Icon name="pencil" size={13} />
                          </button>
                        </div>
                      {/if}
                      <div class="exe muted small truncate mono" title={a.exe_path ?? a.exe_name}>
                        {a.exe_name}
                      </div>
                    </div>
                  </div>
                </td>
                <td>
                  <select
                    class="cat-select"
                    aria-label="Category for {nameOf(a)}"
                    value={a.category_id}
                    disabled={isBusy}
                    onchange={(e) => update(a, { category_id: Number(e.currentTarget.value) })}
                  >
                    {#each categories as c (c.id)}
                      <option value={c.id}>{c.name}</option>
                    {/each}
                  </select>
                </td>
                <td>
                  <Switch
                    checked={a.is_game}
                    disabled={isBusy}
                    title="{nameOf(a)} is a game"
                    onchange={(v) => update(a, { is_game: v })}
                  />
                </td>
                <td>
                  {#if a.category_source === "user"}
                    <span class="pill accent" title="Category picked by you">you</span>
                  {:else}
                    <span class="pill" title="Category picked automatically">auto</span>
                  {/if}
                </td>
                <td class="muted small nowrap" title={dateTime(a.last_seen)}>
                  {relative(a.last_seen, now)}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </Card>

  <Card
    title="Categories"
    subtitle="Rules match on the kind (Work, Game, Web, Other), so a custom category behaves like its kind."
  >
    {#if !loaded}
      <div class="cat-skeleton"></div>
    {:else}
      <ul class="cats">
        {#each categories as c (c.id)}
          {@const n = appCounts.get(c.id) ?? 0}
          <li>
            <span class="swatch" style:background={c.color ?? categoryColor[c.kind]}></span>
            <span class="cat-name truncate">{c.name}</span>
            <span class="pill">{c.kind}</span>
            <span class="muted small num cat-count">{n} {n === 1 ? "app" : "apps"}</span>
          </li>
        {/each}
      </ul>
    {/if}
    <form class="new-cat" onsubmit={addCategory}>
      <input
        class="new-name"
        placeholder="New category"
        aria-label="New category name"
        bind:value={newName}
      />
      <select aria-label="Kind of the new category" bind:value={newKind}>
        {#each KINDS as k (k)}
          <option value={k}>{k}</option>
        {/each}
      </select>
      <input
        type="color"
        aria-label="Color of the new category"
        title="Color"
        value={newColor}
        oninput={(e) => {
          newColor = e.currentTarget.value;
          colorTouched = true;
        }}
      />
      <button class="btn primary" type="submit" disabled={adding || !newName.trim()}>
        <Icon name="plus" size={14} /> Add
      </button>
    </form>
  </Card>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
  }
  .count {
    margin-top: calc(-1 * var(--sp-2));
    min-height: 18px;
  }

  /* Toolbar */
  .search {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .search-icon {
    position: absolute;
    left: 9px;
    display: grid;
    place-items: center;
    color: var(--muted);
    pointer-events: none;
  }
  .search input {
    width: 200px;
    padding-left: 30px;
  }

  /* Table */
  .scroll {
    overflow-x: auto;
  }
  .table {
    min-width: 640px;
  }
  .w-cat {
    width: 170px;
  }
  .w-game {
    width: 70px;
  }
  .w-src {
    width: 80px;
  }
  .w-seen {
    width: 120px;
  }
  .nowrap {
    white-space: nowrap;
  }
  .app-cell {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .app-text {
    flex: 1;
    min-width: 0;
  }
  .name-row {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    height: 26px;
  }
  .name {
    all: unset;
    cursor: text;
    font-weight: 500;
    min-width: 0;
    border-radius: 4px;
  }
  .name:focus-visible {
    box-shadow: var(--focus);
  }
  .pencil {
    opacity: 0;
    transition: opacity 0.12s;
  }
  tr:hover .pencil,
  .pencil:focus-visible {
    opacity: 1;
  }
  .name-input {
    width: 100%;
    max-width: 320px;
    height: 26px;
    padding: 2px 8px;
    font-weight: 500;
  }
  .exe {
    font-size: 11.5px;
  }
  .cat-select {
    width: 100%;
    padding-top: 3px;
    padding-bottom: 3px;
  }

  /* Loading placeholders keep the card's height steady. */
  .skeleton {
    padding: var(--sp-2) var(--sp-4);
  }
  .sk-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .sk-row:last-child {
    border-bottom: none;
  }
  .sk {
    display: block;
    height: 10px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--muted) 16%, transparent);
  }
  .sk-a {
    width: 32%;
  }
  .sk-b {
    width: 18%;
    height: 8px;
  }
  .cat-skeleton {
    min-height: 120px;
  }

  /* Categories */
  .cats {
    list-style: none;
    margin: 0 0 var(--sp-4);
    padding: 0;
  }
  .cats li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 0;
    border-bottom: 1px solid var(--border);
  }
  .cats li:last-child {
    border-bottom: none;
  }
  .cat-name {
    flex: 1;
    min-width: 0;
  }
  .cat-count {
    width: 64px;
    text-align: right;
  }
  .new-cat {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2);
  }
  .new-name {
    flex: 1;
    min-width: 160px;
    max-width: 280px;
  }
</style>
