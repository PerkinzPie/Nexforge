<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu, { type Item } from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api, type Addon } from "../lib/api";
  import { ago, plural } from "../lib/format";
  import { store, type InstalledTab } from "../lib/store.svelte";

  let { filterInput = $bindable() }: { filterInput?: HTMLInputElement } = $props();

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");

  // Counts are the filters (SPEC 4.2): All · Updates · Can't update · Pinned.
  const tabs = $derived<{ id: InstalledTab; label: string; n: number | null; tone?: string }[]>([
    { id: "all", label: "All", n: store.addons.length },
    { id: "updates", label: "Updates", n: store.checkedAt ? store.updates.length : null, tone: "var(--accent-text)" },
    { id: "attention", label: "Can't update", n: store.attentionCount, tone: "var(--attention)" },
    { id: "pinned", label: "Pinned", n: store.pinnedCount },
  ]);
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  // Left/Right move between tabs, as in any tab list.
  function tabKeys(e: KeyboardEvent) {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const i = tabs.findIndex((t) => t.id === store.tab);
    const next = tabs[(i + (e.key === "ArrowRight" ? 1 : tabs.length - 1)) % tabs.length];
    store.tab = next.id;
    queueMicrotask(() => document.getElementById(`tab-${next.id}`)?.focus());
  }

  const shown = $derived.by(() => {
    const q = store.filter.trim().toLowerCase();
    const list = store.addons
      .filter((a) => store.inTab(a, store.tab))
      .filter((a) => !q || a.name.toLowerCase().includes(q) || a.folders.some((f) => f.toLowerCase().includes(q)) || (a.author ?? "").toLowerCase().includes(q));
    // Things to do first, then the rest by name.
    const rank = (a: Addon) => {
      if (a.id in store.busy) return 0;
      const k = store.status(a.id)?.kind;
      return k === "available" ? 1 : !a.managed || k === "no_build" || k === "failed" ? 2 : 3;
    };
    return [...list].sort((x, y) => rank(x) - rank(y) || x.name.localeCompare(y.name));
  });
  const updatable = $derived(store.updates.filter((a) => !a.pinned));

  function menu(a: Addon): Item[] {
    const s = store.status(a.id);
    const items: Item[] = [];
    if (a.managed) items.push({ label: a.pinned ? "Unpin" : "Pin (never update)", run: () => store.setPinned(a, !a.pinned) });
    if (s?.release && (s.kind === "available" || s.kind === "pinned"))
      items.push({ label: `Skip ${s.release.version}`, run: () => store.skip(a, s.release!.version) });
    items.push({ label: a.managed ? "Change source…" : "Identify source…", run: () => (store.dialog = { kind: "install", adopt: a }) });
    items.push({ label: "Open folder", run: () => api.open("addon", store.current!, a.id).catch((e) => store.say("err", e.message)) });
    if (a.website) items.push({ label: "Open website", run: () => api.openUrl(a.website!).catch((e) => store.say("err", e.message)) });
    items.push({ label: "Remove…", danger: true, run: () => (store.dialog = { kind: "remove", addon: a }) });
    return items;
  }
</script>

<div class="page">
  <div class="page-head">
    <h1 class="h1 title">Installed</h1>
    <button type="button" class="btn" onclick={() => store.check()} disabled={store.checking || !store.current}>
      <Icon name="refresh" size={15} stroke={2.2} />{store.checking ? "Checking…" : "Check now"}
    </button>
    <button type="button" class="btn" class:btn-primary={updatable.length > 0} onclick={() => store.updateAll()} disabled={!updatable.length}>
      <Icon name="download" size={15} stroke={2.4} />{updatable.length ? `Update all ${updatable.length}` : "Update all"}
    </button>
  </div>

  {#if !store.snap?.installations.length}
    <div class="notice attn">
      <Icon name="warn" size={16} />
      <span>No WoW installation found. <a href="#settings" onclick={(e) => { e.preventDefault(); store.view = "settings"; }}>Add one</a></span>
    </div>
  {:else if store.snap && !store.snap.addonsDirExists}
    <div class="notice attn"><Icon name="warn" size={16} /><span>No AddOns folder yet. Start the game once.</span></div>
  {/if}

  {#each store.sourceErrors as e (e.source)}
    <div class="notice err"><Icon name="warn" size={16} /><span><strong>{e.sourceLabel}:</strong> {e.message}</span></div>
  {/each}

  <div class="panel list">
    <div class="tabs">
      <div class="tablist" role="tablist" aria-label="Show addons" tabindex="-1" onkeydown={tabKeys}>
        {#each tabs as t (t.id)}
          <button type="button" role="tab" id="tab-{t.id}" class="tab" class:on={store.tab === t.id}
            aria-selected={store.tab === t.id} aria-controls="addon-list" tabindex={store.tab === t.id ? 0 : -1}
            onclick={() => (store.tab = t.id)}>
            {t.label}
            {#if t.n !== null}<span class="n" style:color={t.n > 0 && t.tone ? t.tone : null}>{t.n}</span>{/if}
          </button>
        {/each}
      </div>
      <div class="tools">
        {#if store.checking}<span class="checked">Checking…</span>{:else if store.checkedAt}<span class="checked">Checked {ago(store.checkedAt, now)}</span>{/if}
        <div class="search">
          <label for="filter" class="sr">Filter installed addons</label>
          <Icon name="search" size={15} colour="var(--text-3)" />
          <input id="filter" type="search" placeholder="Filter" bind:value={store.filter} bind:this={filterInput} />
        </div>
      </div>
    </div>
    {#if shown.length}
      <table id="addon-list" aria-labelledby="tab-{store.tab}">
        <tbody>
          {#each shown as a (a.id)}
            {@const s = store.status(a.id)}
            {@const busy = store.busy[a.id]}
            {@const pct = store.progress[a.id]}
            {@const next = s?.release && (s.kind === "available" || s.kind === "pinned") ? s.release.version : null}
            <tr>
              <td>
                <div class="who">
                  <Tile name={a.name} iconUrl={a.iconUrl} unknown={!a.managed} />
                  <div class="names">
                    <span class="n">{a.name}</span>
                    <span class="dim small">{plural(a.folders.length, "folder")}{a.managed && a.author ? ` · ${a.author}` : ""}</span>
                  </div>
                </div>
              </td>
              <td>
                <div class="ver">
                  <span class="mono" class:dim={!a.managed}>
                    {a.version || "–"}
                    {#if next && !a.pinned}<span class="dim">→</span> <strong>{next}</strong>{/if}
                    {#if a.pinned && next}<span class="dim">· {next} held</span>{:else if a.skippedVersion && s?.kind === "skipped"}<span class="dim">· {a.skippedVersion} skipped</span>{/if}
                  </span>
                </div>
              </td>
              <td><SourceTag source={a.source} label={a.sourceLabel} /></td>
              <td>
                {#if busy === "Updating" || busy === "Installing"}
                  <ProgressButton label="Update" {busy} {pct} onclick={() => {}} />
                {:else if busy}
                  <span class="dim state">{busy}…</span>
                {:else if !a.managed}
                  <a href="#identify" class="identify" onclick={(e) => { e.preventDefault(); store.dialog = { kind: "install", adopt: a }; }}>Identify source</a>
                {:else if a.pinned}
                  <span class="pin"><Icon name="pin" size={14} stroke={2.2} />Pinned</span>
                {:else if s?.kind === "available"}
                  <button type="button" class="btn btn-row" onclick={() => store.update(a.id)}>Update</button>
                {:else if s?.kind === "no_build"}
                  <span class="warn"><Icon name="warn" size={12} stroke={2.4} />No {flavour} build</span>
                {:else if s?.kind === "failed" || s?.kind === "unavailable"}
                  <span class="warn" title={s.message ?? ""}><Icon name="warn" size={12} stroke={2.4} />{s.kind === "failed" ? "Check failed" : "Needs key"}</span>
                {:else if s?.kind === "up_to_date" || s?.kind === "skipped"}
                  <span class="ok"><Icon name="check" size={14} stroke={2.6} />Up to date</span>
                {:else if a.flavourMismatch}
                  <span class="warn"><Icon name="warn" size={12} stroke={2.4} />No {flavour} TOC</span>
                {:else}
                  <span class="dim state">{store.checking ? "Checking…" : "Not checked"}</span>
                {/if}
              </td>
              <td><RowMenu label="More actions for {a.name}" items={menu(a)} /></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else if store.filter}
      <div class="empty">No matches.</div>
    {:else if store.tab === "updates"}
      <div class="empty">{store.checkedAt ? "All up to date." : "Not checked yet."}</div>
    {:else if store.tab !== "all"}
      <div class="empty">None.</div>
    {:else}
      <div class="empty">No addons yet. <a href="#browse" onclick={(e) => { e.preventDefault(); store.view = "browse"; }}>Browse</a></div>
    {/if}
  </div>
</div>

<style>
  .title { margin-right: auto; }
  .list { overflow: hidden; }
  .tabs { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; padding: 6px 8px 0 8px; border-bottom: 1px solid var(--border); }
  .tablist { display: flex; align-items: center; gap: 4px; }
  .tab { position: relative; display: inline-flex; align-items: center; gap: 8px; height: 44px; padding: 0 12px; border: 0; background: none; color: var(--text-2); font: 600 13px var(--sans); cursor: pointer; }
  .tab:hover { color: var(--text); }
  .tab .n { font: 600 12px var(--display); color: var(--text-3); font-variant-numeric: tabular-nums; }
  .tab.on { color: var(--text); }
  .tab.on::after { content: ""; position: absolute; left: 10px; right: 10px; bottom: -1px; height: 2px; border-radius: 2px; background: var(--text); }
  .tools { margin-left: auto; display: flex; align-items: center; gap: 12px; padding-bottom: 6px; }
  .checked { font-size: 12px; color: var(--text-3); white-space: nowrap; }
  table { margin: 0; }
  tbody tr:first-child td { border-top: 0; }
  td { height: 58px; }
  .small { font-size: 12px; }
  .who { display: flex; align-items: center; gap: 12px; }
  .names { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .n { font-weight: 700; }
  .ver { display: flex; flex-direction: column; gap: 6px; }
  .ver strong { color: var(--text); font-weight: 600; }
  .state { font-size: 12.5px; font-weight: 600; }
  .identify { font-size: 12.5px; font-weight: 700; }
</style>
