<script lang="ts">
  import { Tabs, mergeProps } from "bits-ui";
  import { untrack } from "svelte";
  import Icon from "../components/Icon.svelte";
  import NoBuild from "../components/NoBuild.svelte";
  import Tip from "../components/Tip.svelte";
  import PageHead from "../components/PageHead.svelte";
  import UpdateActions from "../components/UpdateActions.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import type { Addon } from "../lib/api";
  import { longDate, plural } from "../lib/format";
  import { addonMenu, clickRow, openDetails } from "../lib/menu";
  import { store, type InstalledTab } from "../lib/store.svelte";

  let { filterInput = $bindable() }: { filterInput?: HTMLInputElement } = $props();

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");

  // Counts are the filters: All · Updates · Issues (no source, needs key, no build, check failed) · Pinned.
  const tabs = $derived<{ id: InstalledTab; label: string; n: number | null; tone?: string }[]>([
    { id: "all", label: "All", n: store.addons.length },
    { id: "updates", label: "Updates", n: store.checkedAt ? store.updates.length : null, tone: "var(--accent-text)" },
    { id: "attention", label: "Issues", n: store.attentionCount, tone: "var(--attention)" },
    { id: "pinned", label: "Pinned", n: store.pinnedCount },
  ]);

  const shown = $derived.by(() => {
    const q = store.filter.trim().toLowerCase();
    const list = store.addons
      .filter((a) => store.inTab(a, store.tab))
      .filter((a) => !q || a.name.toLowerCase().includes(q) || a.folders.some((f) => f.toLowerCase().includes(q)) || (a.author ?? "").toLowerCase().includes(q));
    // Things to do first, then the rest by name, in the order of the last
    // check: rows never jump while updates run.
    const rank = (a: Addon) => ranks[a.id] ?? 3;
    return [...list].sort((x, y) => rank(x) - rank(y) || x.name.localeCompare(y.name));
  });

  // Ranks are taken when a check finishes or the list of addons changes, not
  // as each update starts and ends.
  let ranks = $state<Record<string, number>>({});
  // Deriveds, so a fresh snapshot with the same client and addons (as after
  // every update) does not count as a change.
  const members = $derived(store.addons.map((a) => a.id).sort().join("|"));
  const client = $derived(store.current);
  $effect(() => {
    void store.checkedAt;
    void client;
    void members;
    untrack(() => {
      const r: Record<string, number> = {};
      for (const a of store.addons) {
        const k = store.status(a.id)?.kind;
        r[a.id] = k === "available" ? 1 : !a.managed || k === "no_build" || k === "failed" ? 2 : 3;
      }
      ranks = r;
    });
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

  {#if store.snap?.stateError}
    <div class="notice err">
      <Icon name="warn" size={16} />
      <span class="grow">{store.snap.stateError}</span>
      <button type="button" class="btn btn-sm" onclick={() => store.resetState()}>Re-read AddOns</button>
    </div>
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
        <!-- Fixed column widths: a status changing (Update → progress → Up to
             date) never resizes the columns. -->
        <colgroup><col /><col class="c-ver" /><col class="c-src" /><col class="c-st" /><col class="c-act" /></colgroup>
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
              <td class="st">
                {#if busy === "Updating" || busy === "Installing"}
                  <ProgressButton label="Update" {busy} {pct} onclick={() => {}} />
                {:else if busy}
                  <span class="dim state">{busy}…</span>
                {:else if !a.managed}
                  <button type="button" class="btn btn-quiet ident" onclick={() => (store.dialog = { kind: "install", adopt: a })}>Identify</button>
                {:else if a.pinned}
                  <span class="pin"><Icon name="pin" size={14} stroke={2.2} />Pinned</span>
                {:else if s?.kind === "available" && store.failed[a.id]}
                  <!-- The update failed: the reason is on the button, which tries again. -->
                  <Tip text={store.failed[a.id]}>
                    {#snippet children({ props })}
                      <button {...mergeProps(props, { onclick: () => store.update(a.id) })} type="button" class="warn"><Icon name="warn" size={12} stroke={2.4} />Retry</button>
                    {/snippet}
                  </Tip>
                {:else if s?.kind === "available"}
                  <ProgressButton label="Update" busy={null} onclick={() => store.update(a.id)} />
                {:else if s?.kind === "no_build"}
                  <NoBuild text="May not load in {flavour}." />
                {:else if s?.kind === "failed"}
                  <Tip text={s.message}>
                    {#snippet children({ props })}
                      <button {...props} type="button" class="warn" data-row-open><Icon name="warn" size={12} stroke={2.4} />Check failed</button>
                    {/snippet}
                  </Tip>
                {:else if s?.kind === "unavailable"}
                  <!-- The fix is in Settings (add a key, or turn the source on): go there. -->
                  <Tip text={s.message}>
                    {#snippet children({ props })}
                      <button {...mergeProps(props, { onclick: () => (store.view = "settings") })} type="button" class="warn"><Icon name="warn" size={12} stroke={2.4} />{a.source === "curseforge" && !store.snap?.settings.keyCurseforge ? "Needs key" : "Source off"}</button>
                    {/snippet}
                  </Tip>
                {:else if s?.kind === "up_to_date" || s?.kind === "skipped"}
                  {#if a.installedAt}
                    <Tip text="{a.kept.length ? 'Updated' : 'Installed'} {longDate(a.installedAt)}">
                      {#snippet children({ props })}
                        <span {...props} class="quiet dated">Up to date</span>
                      {/snippet}
                    </Tip>
                  {:else}
                    <span class="quiet">Up to date</span>
                  {/if}
                {:else if a.flavourMismatch}
                  <NoBuild text="May not load in {flavour}." />
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
      <div class="empty">{!store.checkedAt ? "Not checked yet." : (store.unchecked ?? "All up to date.")}</div>
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
  .tab .n { min-width: 2ch; font: 600 12px var(--display); color: var(--text-3); font-variant-numeric: tabular-nums; }
  .tab.on { color: var(--text); }
  .tab.on::after { content: ""; position: absolute; left: 10px; right: 10px; bottom: -1px; height: 2px; border-radius: 2px; background: var(--text); }
  .tools { margin-left: auto; display: flex; align-items: center; gap: 12px; padding-bottom: 4px; }
  table { margin: 0; }
  tbody tr:first-child td { border-top: 0; }
  .small { font-size: 11px; }
  table { table-layout: fixed; }
  .c-ver { width: 200px; }
  .c-src { width: 136px; }
  .c-st { width: 124px; }
  /* Update button, progress and status text share one centre line. */
  td.st { text-align: center; }
  .c-act { width: 48px; }
  .ver .num { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .names .n, .names .small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .who { max-width: 100%; display: flex; align-items: center; gap: 10px; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .open-row { cursor: pointer; }
  .open-row:hover td { background: var(--raised); }
  .quiet { font-size: 12px; color: var(--text-3); }
  .ident { width: 96px; }
  .dated { text-decoration: underline dashed; text-underline-offset: 3px; text-decoration-thickness: 1px; }
  .pillskel { display: block; width: 76px; height: 14px; margin: 0 auto; }
  .names { display: flex; flex-direction: column; gap: 0; min-width: 0; line-height: 1.3; }
  .n { font-weight: 700; }
  .ver { display: flex; flex-direction: column; gap: 6px; }
  .ver strong { color: var(--text); font-weight: 600; }
  .state { font-size: 12px; font-weight: 600; }
</style>
