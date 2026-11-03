<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AppRow, type Category, type CategoryKind } from "../lib/api";
  import { categoryColor, dateTime } from "../lib/format";

  let apps = $state<AppRow[]>([]);
  let categories = $state<Category[]>([]);
  let filter = $state("");
  let error = $state<string | null>(null);
  let newName = $state("");
  let newKind = $state<CategoryKind>("Work");

  const shown = $derived(
    apps.filter((a) =>
      `${a.exe_name} ${a.display_name ?? ""} ${a.category}`.toLowerCase().includes(filter.toLowerCase()),
    ),
  );

  async function load() {
    try {
      [apps, categories] = await Promise.all([api.apps(), api.categories()]);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function update(a: AppRow, patch: Parameters<typeof api.updateApp>[1]) {
    try {
      const updated = await api.updateApp(a.id, patch);
      apps = apps.map((x) => (x.id === updated.id ? updated : x));
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function addCategory(e: SubmitEvent) {
    e.preventDefault();
    if (!newName.trim()) return;
    try {
      await api.createCategory(newName.trim(), newKind);
      newName = "";
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  onMount(load);
</script>

<section class="panel">
  <div class="head">
    <h2>Apps</h2>
    <input placeholder="Filter" bind:value={filter} />
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <p class="muted">
    New apps get a category automatically (known games, launcher folders, browsers, common work apps).
    A category you pick here is never overwritten.
  </p>
  <table>
    <thead>
      <tr><th>App</th><th>Display name</th><th>Category</th><th>Game</th><th>Last seen</th></tr>
    </thead>
    <tbody>
      {#each shown as a (a.id)}
        <tr>
          <td title={a.exe_path ?? ""}>
            <span class="swatch" style:background={categoryColor[a.category_kind]}></span>
            {a.exe_name}
          </td>
          <td>
            <input
              value={a.display_name ?? ""}
              placeholder={a.exe_name}
              onchange={(e) => update(a, { display_name: e.currentTarget.value.trim() || null })}
            />
          </td>
          <td>
            <select
              value={a.category_id}
              onchange={(e) => update(a, { category_id: Number(e.currentTarget.value) })}
            >
              {#each categories as c (c.id)}
                <option value={c.id}>{c.name}</option>
              {/each}
            </select>
            {#if a.category_source === "auto"}<span class="muted small">auto</span>{/if}
          </td>
          <td>
            <input
              type="checkbox"
              checked={a.is_game}
              onchange={(e) => update(a, { is_game: e.currentTarget.checked })}
            />
          </td>
          <td class="muted">{dateTime(a.last_seen)}</td>
        </tr>
      {:else}
        <tr><td class="muted" colspan="5">No apps yet.</td></tr>
      {/each}
    </tbody>
  </table>
</section>

<section class="panel" style="margin-top: 16px">
  <h2>Categories</h2>
  <p class="muted">
    Rules match on the kind (Work, Game, Web, Other), so a custom category behaves like its kind.
  </p>
  <ul>
    {#each categories as c (c.id)}
      <li><span class="swatch" style:background={c.color ?? categoryColor[c.kind]}></span> {c.name} <span class="muted">({c.kind})</span></li>
    {/each}
  </ul>
  <form onsubmit={addCategory}>
    <input placeholder="New category" bind:value={newName} />
    <select bind:value={newKind}>
      {#each ["Work", "Game", "Web", "Other"] as k (k)}
        <option value={k}>{k}</option>
      {/each}
    </select>
    <button type="submit">Add</button>
  </form>
</section>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    margin-right: 6px;
  }
  .small {
    font-size: 12px;
    margin-left: 4px;
  }
  ul {
    list-style: none;
    padding: 0;
  }
  form {
    display: flex;
    gap: 8px;
  }
</style>
