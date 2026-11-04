<script lang="ts">
  import { Tabs } from "bits-ui";
  import Icon from "../components/Icon.svelte";
  import NoBuild from "../components/NoBuild.svelte";
  import PageHead from "../components/PageHead.svelte";
  import UpdateActions from "../components/UpdateActions.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import type { Addon } from "../lib/api";
  import { plural } from "../lib/format";
  import { addonMenu, clickRow, openDetails } from "../lib/menu";
  import { store, type InstalledTab } from "../lib/store.svelte";

  let { filterInput = $bindable() }: { filterInput?: HTMLInputElement } = $props();

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");

  // Counts are the filters: All · Updates · Can't update · Pinned.
  const tabs = $derived<{ id: InstalledTab; label: string; n: number | null; tone?: string }[]>([
    { id: "all", label: "All", n: store.addons.length },
    { id: "updates", label: "Updates", n: store.checkedAt ? store.updates.length : null, tone: "var(--accent-text)" },
    { id: "attention", label: "Can't update", n: store.attentionCount, tone: "var(--attention)" },
    { id: "pinned", label: "Pinned", n: store.pinnedCount },
  ]);

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

</script>

<div class="page">
  <PageHead title="Installed"><UpdateActions /></PageHead>

  {#if !store.snap?.installations.length}
    <div class="notice attn">
      <Icon name="warn" size={16} />
      <span>No WoW installation found. <a href="#settings" onclick={(e) => { e.preventDefault(); store.view = "settings"; }}>Add one</a></span>
    </div>
  {:else if store.snap && !store.snap.addonsDirExists}
    <div class="notice attn"><Icon name="warn" size={16} /><span>No AddOns folder yet. Start the game once.</span></div>
  {/if}

  {#each store.sourceErrors as e (e.source)}
    <div class="notice err"><Icon name="warn" size={16} /><span>{#if !e.message.startsWith(e.sourceLabel)}<strong>{e.sourceLabel}:</strong> {/if}{e.message}</span></div>
  {/each}

  <Tabs.Root value={store.tab} onValueChange={(v) => (store.tab = v as InstalledTab)} class="panel list">
    <div class="tabs">
      <Tabs.List class="tablist" aria-label="Show addons">
        {#each tabs as t (t.id)}
          <Tabs.Trigger value={t.id}>
            {#snippet child({ props })}
              <button {...props} type="button" class="tab" class:on={store.tab === t.id}>
                {t.label}
                {#if t.n !== null}<span class="n" style:color={t.n > 0 && t.tone ? t.tone : null}>{t.n}</span>{/if}
              </button>
            {/snippet}
          </Tabs.Trigger>
        {/each}
      </Tabs.List>
      <div class="tools">
        <div class="search">
          <label for="filter" class="sr">Filter installed addons</label>
          <Icon name="search" size={15} colour="var(--text-3)" />
          <input id="filter" type="search" placeholder="Filter" bind:value={store.filter} bind:this={filterInput} />
        </div>
      </div>
    </div>
    <Tabs.Content value={store.tab}>
    {#if shown.length}
      <table>
        <tbody>
          {#each shown as a (a.id)}
            {@const s = store.status(a.id)}
            {@const busy = store.busy[a.id]}
            {@const pct = store.progress[a.id]}
            {@const next = s?.release && (s.kind === "available" || s.kind === "pinned") ? s.release.version : null}
            <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (the name button opens it from the keyboard) -->
            <tr class="open-row" onclick={(e) => clickRow(e, () => openDetails(a))}>
              <td>
                <button type="button" class="who" data-row-open aria-label="Details for {a.name}">
                  <Tile name={a.name} iconUrl={a.iconUrl} size={32} unknown={!a.managed} />
                  <span class="names">
                    <span class="n">{a.name}</span>
                    <span class="dim small">{plural(a.folders.length, "folder")}{a.managed && a.author ? ` · ${a.author}` : ""}</span>
                  </span>
                </button>
              </td>
              <td>
                <div class="ver">
                  <span class="num" class:dim={!a.managed}>
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
                  <NoBuild text="No {flavour} build listed. It may not load in {flavour}." />
                {:else if s?.kind === "failed" || s?.kind === "unavailable"}
                  <span class="warn" title={s.message ?? ""}><Icon name="warn" size={12} stroke={2.4} />{s.kind === "failed" ? "Check failed" : "Needs key"}</span>
                {:else if s?.kind === "up_to_date" || s?.kind === "skipped"}
                  <span class="quiet">Up to date</span>
                {:else if a.flavourMismatch}
                  <NoBuild text="Its TOC doesn't list {flavour}. It may not load." />
                {:else}
                  {#if store.checking}<span class="skel pillskel" aria-label="Checking"></span>{:else}<span class="quiet">Not checked</span>{/if}
                {/if}
              </td>
              <td class="actions"><RowMenu label="More actions for {a.name}" items={addonMenu(a)} /></td>
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
    </Tabs.Content>
  </Tabs.Root>
</div>

<style>
  :global(.panel.list) { overflow: hidden; }
  .tabs { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; padding: 6px 8px 0 8px; border-bottom: 1px solid var(--border); }
  :global(.tablist) { display: flex; align-items: center; gap: 4px; }
  .tab { position: relative; display: inline-flex; align-items: center; gap: 8px; height: 38px; padding: 0 12px; border: 0; background: none; color: var(--text-2); font: 600 13px var(--sans); cursor: pointer; }
  .tab:hover { color: var(--text); }
  .tab .n { font: 600 12px var(--display); color: var(--text-3); font-variant-numeric: tabular-nums; }
  .tab.on { color: var(--text); }
  .tab.on::after { content: ""; position: absolute; left: 10px; right: 10px; bottom: -1px; height: 2px; border-radius: 2px; background: var(--text); }
  .tools { margin-left: auto; display: flex; align-items: center; gap: 12px; padding-bottom: 4px; }
  table { margin: 0; }
  tbody tr:first-child td { border-top: 0; }
  .small { font-size: 11px; }
  .who { display: flex; align-items: center; gap: 10px; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .open-row { cursor: pointer; }
  .open-row:hover td { background: var(--raised); }
  .quiet { font-size: 12px; color: var(--text-3); }
  .pillskel { display: block; width: 76px; height: 14px; }
  .names { display: flex; flex-direction: column; gap: 0; min-width: 0; line-height: 1.3; }
  .n { font-weight: 700; }
  .ver { display: flex; flex-direction: column; gap: 6px; }
  .ver strong { color: var(--text); font-weight: 600; }
  .state { font-size: 12px; font-weight: 600; }
  .identify { font-size: 12px; font-weight: 700; }
</style>
