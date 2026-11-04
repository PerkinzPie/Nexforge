<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import RowMenu, { type Item } from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api, type Addon } from "../lib/api";
  import { plural, when } from "../lib/format";
  import { store } from "../lib/store.svelte";

  let { filterInput = $bindable() }: { filterInput?: HTMLInputElement } = $props();

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");
  const shown = $derived.by(() => {
    const q = store.filter.trim().toLowerCase();
    const list = q
      ? store.addons.filter((a) => a.name.toLowerCase().includes(q) || a.folders.some((f) => f.toLowerCase().includes(q)) || (a.author ?? "").toLowerCase().includes(q))
      : store.addons;
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
    <div class="page-title">
      <h1 class="h1">Installed</h1>
      <span class="dim">
        {store.installation?.label ?? "No installation"}
        {#if store.snap?.stateExists} · addons.toml is in sync{/if}
      </span>
    </div>
    <div class="search">
      <label for="filter" class="sr">Filter installed addons</label>
      <Icon name="search" size={15} colour="var(--text-3)" />
      <input id="filter" type="search" placeholder="Filter" bind:value={store.filter} bind:this={filterInput} />
    </div>
    <button type="button" class="btn" onclick={() => store.check()} disabled={store.checking || !store.current}>
      <Icon name="refresh" size={15} stroke={2.2} />{store.checking ? "Checking…" : "Check now"}
    </button>
    <button type="button" class="btn" class:btn-primary={updatable.length > 0} onclick={() => store.updateAll()} disabled={!updatable.length}>
      <Icon name="download" size={15} stroke={2.4} />Update all ({updatable.length})
    </button>
  </div>

  {#if !store.snap?.installations.length}
    <div class="notice attn">
      <Icon name="warn" size={16} />
      <span>No WoW installation found. <a href="#settings" onclick={(e) => { e.preventDefault(); store.view = "settings"; }}>Add one in Settings</a>.</span>
    </div>
  {:else if store.snap && !store.snap.addonsDirExists}
    <div class="notice attn"><Icon name="warn" size={16} /><span>This installation has no Interface\AddOns folder yet. Start the game once, or install an addon from Browse.</span></div>
  {/if}

  {#each store.sourceErrors as e (e.source)}
    <div class="notice err"><Icon name="warn" size={16} /><span><strong>{e.sourceLabel}:</strong> {e.message}</span></div>
  {/each}

  <div class="stats">
    <div class="stat"><span class="v">{store.addons.length}</span><span class="l">Installed</span></div>
    <div class="stat" style:border-color={store.updates.length ? "var(--accent-fill)" : null}>
      <span class="v" style:color={store.updates.length ? "var(--accent-text)" : null}>{store.checkedAt ? store.updates.length : "–"}</span><span class="l">Updates available</span>
    </div>
    <div class="stat"><span class="v" style:color="var(--info)">{store.pinnedCount}</span><span class="l">Pinned</span></div>
    <div class="stat"><span class="v" style:color="var(--attention)">{store.attentionCount}</span><span class="l">Need attention</span></div>
    <div class="stat wide">
      <span class="l">Last update run</span>
      {#if store.snap?.lastRun}
        <span class="lr">{when(store.snap.lastRun.at)} · {plural(store.snap.lastRun.count, "addon")}</span>
        <span class="dim small">Previous versions kept for rollback</span>
      {:else}
        <span class="lr">None yet</span><span class="dim small">Updates keep the previous version for rollback</span>
      {/if}
    </div>
  </div>

  <div class="panel tablewrap">
    {#if shown.length}
      <table>
        <thead>
          <tr>
            <th scope="col" style="width: 40%">Addon</th>
            <th scope="col">Version</th>
            <th scope="col">Source</th>
            <th scope="col">Status</th>
            <th scope="col" style="width: 46px"><span class="sr">Actions</span></th>
          </tr>
        </thead>
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
                    <span class="dim small">{plural(a.folders.length, "folder")} · {a.managed ? (a.author ? `by ${a.author}` : a.sourceLabel) : "found in AddOns, not managed yet"}</span>
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
                  {#if busy && a.id in store.progress}
                    <span class="bar" class:indeterminate={pct == null}><span style:width="{pct ?? 30}%"></span></span>
                  {/if}
                </div>
              </td>
              <td><SourceTag source={a.source} label={a.sourceLabel} /></td>
              <td>
                {#if busy}
                  <span class="dim state">{busy}{pct != null ? `… ${pct}%` : "…"}</span>
                {:else if !a.managed}
                  <a href="#identify" class="identify" onclick={(e) => { e.preventDefault(); store.dialog = { kind: "install", adopt: a }; }}>Identify source</a>
                {:else if a.pinned}
                  <span class="pin"><Icon name="pin" size={14} stroke={2.2} />Pinned</span>
                {:else if s?.kind === "available"}
                  <button type="button" class="btn btn-row" onclick={() => store.update(a.id)}>Update</button>
                {:else if s?.kind === "no_build"}
                  <span class="warn"><Icon name="warn" size={12} stroke={2.4} />No {flavour} build</span>
                {:else if s?.kind === "failed" || s?.kind === "unavailable"}
                  <span class="warn" title={s.message ?? ""}><Icon name="warn" size={12} stroke={2.4} />{s.kind === "failed" ? "Check failed" : "Source unavailable"}</span>
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
      <div class="empty">No installed addon matches “{store.filter}”.</div>
    {:else}
      <div class="empty">No addons here yet. Find some in <a href="#browse" onclick={(e) => { e.preventDefault(); store.view = "browse"; }}>Browse</a>.</div>
    {/if}
  </div>
</div>

<style>
  .stats { display: flex; flex-wrap: wrap; gap: 12px; }
  .stat.wide { flex: 1 1 220px; }
  .lr { font-size: 13px; font-weight: 600; }
  .small { font-size: 12px; }
  .tablewrap { overflow-x: auto; padding: 14px 4px 4px 4px; }
  .who { display: flex; align-items: center; gap: 12px; }
  .names { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .n { font-weight: 700; }
  .ver { display: flex; flex-direction: column; gap: 6px; }
  .ver strong { color: var(--text); font-weight: 600; }
  .state { font-size: 12.5px; font-weight: 600; }
  .identify { font-size: 12.5px; font-weight: 700; }
</style>
