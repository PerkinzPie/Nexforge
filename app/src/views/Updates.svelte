<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import UpdateActions from "../components/UpdateActions.svelte";
  import { clickRow, openDetails } from "../lib/menu";
  import ProgressButton from "../components/ProgressButton.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api } from "../lib/api";
  import { shortDate, size, when } from "../lib/format";
  import { store } from "../lib/store.svelte";

  const cards = $derived(store.addons.filter((a) => store.status(a.id)?.kind === "available" || (store.busy[a.id] === "Updating")));

  // Bulk update checks carry no notes for WoWInterface or CurseForge; fetch
  // them once per card (keyed by addon and version).
  let fetched = $state<Record<string, { lines: string[]; hasMore: boolean }>>({});
  const asked = new Set<string>();
  $effect(() => {
    const inst = store.current;
    if (!inst) return;
    for (const a of cards) {
      const r = store.status(a.id)?.release;
      const key = `${a.id}@${r?.version}`;
      if (!r || r.notes.length || asked.has(key)) continue;
      asked.add(key);
      api.releaseNotes(inst, a.id).then((n) => (fetched[key] = n)).catch(() => (fetched[key] = { lines: [], hasMore: false }));
    }
  });
</script>

<div class="page">
  <PageHead title="Updates"><UpdateActions /></PageHead>

  {#each store.sourceErrors as e (e.source)}
    <div class="notice err narrow"><Icon name="warn" size={16} /><span>{#if !e.message.startsWith(e.sourceLabel)}<strong>{e.sourceLabel}:</strong> {/if}{e.message}</span></div>
  {/each}

  <div class="list narrow">
    {#each cards as a (a.id)}
      {@const s = store.status(a.id)}
      {@const r = s?.release}
      {@const busy = store.busy[a.id]}
      {@const pct = store.progress[a.id]}
      {@const extra = fetched[`${a.id}@${r?.version}`]}
      {@const notes = r?.notes.length ? r.notes : (extra?.lines ?? [])}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (the name button opens it from the keyboard) -->
      <div class="card open-card" class:active={!!busy} onclick={(e) => clickRow(e, () => openDetails(a))}>
        <div class="top">
          <Tile name={a.name} iconUrl={a.iconUrl} size={36} />
          <button type="button" class="meta" data-row-open aria-label="Details for {a.name}">
            <span class="n">{a.name}</span>
            <span class="line">
              <span class="num">{a.version} {#if r}<span class="dim">→</span> <strong>{r.version}</strong>{/if}</span>
              <SourceTag source={a.source} label={a.sourceLabel} />
              {#if r && (r.publishedAt || r.size)}<span class="dim small">{[r.publishedAt ? `Released ${shortDate(r.publishedAt)}` : "", size(r.size)].filter(Boolean).join(" · ")}</span>{/if}

            </span>
          </button>
          {#if r && !busy}<button type="button" class="btn btn-quiet" onclick={() => store.skip(a, r.version)}>Skip {r.version}</button>{/if}
          {#if r || busy}<ProgressButton label="Update" busy={busy ?? null} {pct} onclick={() => store.update(a.id)} />{/if}
        </div>
        {#if notes.length}
          <ul class="notes">{#each notes as n}<li>{n}</li>{/each}</ul>
        {:else if r && !fetched[`${a.id}@${r.version}`]}
          <div class="skels" aria-label="Loading release notes"><span class="skel" style:width="62%"></span><span class="skel" style:width="44%"></span></div>
        {/if}
        {#if a.website && (r?.hasMoreNotes || extra?.hasMore || !notes.length)}
          <a href={a.website} class="more" onclick={(e) => { e.preventDefault(); api.openUrl(a.website!).catch((err) => store.say("err", err.message)); }}>Full release notes</a>
        {/if}
      </div>
    {:else}
      <div class="panel empty">
        {#if !store.checkedAt}Not checked yet.{:else}<span class="ok"><Icon name="check" size={14} stroke={2.6} />All up to date</span>{/if}
      </div>
    {/each}
  </div>

  {#if store.recent.length}
    <div class="recent narrow">
      <h2 class="h2">Recently updated</h2>
      <div class="panel">
        {#each store.recent as r (r.id)}
          <div class="row">
            <Tile name={r.name} size={32} />
            <div class="rmeta">
              <span class="rn">{r.name}</span>
              <span class="dim small">{r.from} → {r.to} · {when(r.at)}</span>
            </div>
            <span class="tag">{r.from} kept</span>
            <button type="button" class="btn btn-quiet" disabled={!!store.busy[r.id]} onclick={() => store.rollback(r.id, r.name)}>
              {store.busy[r.id] === "Rolling back" ? "Rolling back…" : "Roll back"}
            </button>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .list { display: flex; flex-direction: column; gap: 12px; }
  .top { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; }
  .meta { display: flex; flex-direction: column; gap: 2px; margin-right: auto; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .open-card { cursor: pointer; }
  .open-card:hover { background: var(--raised); }
  .skels { display: flex; flex-direction: column; gap: 8px; padding: 2px 0 2px 18px; }
  .skels .skel { height: 12px; }
  .n { font-size: 15px; font-weight: 700; }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .line strong { color: var(--text); font-weight: 600; }
  .small { font-size: 11px; }
  .more { font-size: 12px; font-weight: 700; }
  .recent { display: flex; flex-direction: column; gap: 8px; margin-top: 4px; }
  .rmeta { display: flex; flex-direction: column; gap: 0; margin-right: auto; }
  .rn { font-size: 13px; font-weight: 700; }
</style>
