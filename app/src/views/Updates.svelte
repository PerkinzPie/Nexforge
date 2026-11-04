<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api } from "../lib/api";
  import { plural, shortDate, size, when } from "../lib/format";
  import { store } from "../lib/store.svelte";

  const cards = $derived(store.addons.filter((a) => store.status(a.id)?.kind === "available" || (store.busy[a.id] === "Updating")));
  const updatable = $derived(store.updates.filter((a) => !a.pinned));
</script>

<div class="page">
  <div class="page-head">
    <div class="page-title">
      <h1 class="h1">Updates</h1>
      <span class="dim">
        {#if store.checkedAt}{plural(store.updates.length, "update")} available{:else if store.checking}Checking…{:else}Not checked yet{/if}
      </span>
    </div>
    <button type="button" class="btn" onclick={() => store.check()} disabled={store.checking || !store.current}>
      <Icon name="refresh" size={15} stroke={2.2} />{store.checking ? "Checking…" : "Check now"}
    </button>
    <button type="button" class="btn" class:btn-primary={updatable.length > 0} onclick={() => store.updateAll()} disabled={!updatable.length}>
      <Icon name="download" size={15} stroke={2.4} />Update all ({updatable.length})
    </button>
  </div>

  {#each store.sourceErrors as e (e.source)}
    <div class="notice err narrow"><Icon name="warn" size={16} /><span><strong>{e.sourceLabel}:</strong> {e.message}</span></div>
  {/each}

  <div class="list narrow">
    {#each cards as a (a.id)}
      {@const s = store.status(a.id)}
      {@const r = s?.release}
      {@const busy = store.busy[a.id]}
      {@const pct = store.progress[a.id]}
      <div class="card" class:active={!!busy}>
        <div class="top">
          <Tile name={a.name} iconUrl={a.iconUrl} size={44} />
          <div class="meta">
            <span class="n">{a.name}</span>
            <span class="line">
              <span class="mono">{a.version} {#if r}<span class="dim">→</span> <strong>{r.version}</strong>{/if}</span>
              <SourceTag source={a.source} label={a.sourceLabel} />
              {#if r && (r.publishedAt || r.size)}<span class="dim small">{[r.publishedAt ? `Released ${shortDate(r.publishedAt)}` : "", size(r.size)].filter(Boolean).join(" · ")}</span>{/if}

            </span>
          </div>
          {#if r && !busy}<button type="button" class="btn btn-quiet" onclick={() => store.skip(a, r.version)}>Skip {r.version}</button>{/if}
          {#if r || busy}<ProgressButton label="Update" busy={busy ?? null} {pct} onclick={() => store.update(a.id)} />{/if}
        </div>
        {#if r?.notes.length}
          <ul class="notes">{#each r.notes as n}<li>{n}</li>{/each}</ul>
        {/if}
        {#if a.website && (r?.hasMoreNotes || !r?.notes.length)}
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
          <div class="row rrow">
            <Tile name={r.name} size={36} />
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
  .top { display: flex; flex-wrap: wrap; align-items: center; gap: 14px; }
  .meta { display: flex; flex-direction: column; gap: 4px; margin-right: auto; min-width: 0; }
  .n { font-size: 16px; font-weight: 800; }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .line strong { color: var(--text); font-weight: 600; }
  .small { font-size: 12px; }
  .more { font-size: 12.5px; font-weight: 700; }
  .recent { display: flex; flex-direction: column; gap: 12px; margin-top: 6px; }
  .rrow { padding: 12px 20px; gap: 14px; }
  .rmeta { display: flex; flex-direction: column; gap: 2px; margin-right: auto; }
  .rn { font-size: 13.5px; font-weight: 700; }
</style>
