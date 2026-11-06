<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import UpdateActions from "../components/UpdateActions.svelte";
  import { clickRow, openDetails } from "../lib/menu";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api } from "../lib/api";
  import { shortDate, size, when } from "../lib/format";
  import { store } from "../lib/store.svelte";

  // Cards that finished updating stay (marked Updated) until the next check,
  // so the list doesn't close up under the pointer.
  const cards = $derived(store.addons.filter((a) => store.status(a.id)?.kind === "available" || store.busy[a.id] === "Updating" || a.id in store.updatedNow));

  // Opening Updates before anything was checked (launch checks off, or a
  // different client) checks once for this client; it only lists updates.
  let triedFor: string | null = null;
  $effect(() => {
    const inst = store.current;
    if (!inst || store.checkedAt || store.checking || triedFor === inst) return;
    triedFor = inst;
    store.check();
  });

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
              <span class="num">{a.version} {#if r && !store.updatedNow[a.id]}<span class="dim">→</span> <strong>{r.version}</strong>{/if}</span>
              <SourceTag source={a.source} label={a.sourceLabel} />
              {#if r && (r.publishedAt || r.size)}<span class="dim small">{[r.publishedAt ? `Released ${shortDate(r.publishedAt)}` : "", size(r.size)].filter(Boolean).join(" · ")}</span>{/if}

            </span>
          </button>
          {#if store.updatedNow[a.id] && !busy}<span class="ok done-st"><Icon name="check" size={14} stroke={2.6} />Updated</span>
          {:else}
            {#if r}<button type="button" class="btn btn-quiet" disabled={!!busy} onclick={() => store.skip(a, r.version)}>Skip {r.version}</button>{/if}
            {#if r || busy}<ProgressButton label="Update" busy={busy ?? null} {pct} onclick={() => store.update(a.id)} />{/if}
          {/if}
        </div>
        <!-- One size whether notes are loading, short or long: the cards
             below never move when they arrive. -->
        <div class="nbody">
          {#if notes.length}
            <ul class="notes">{#each notes as n}<li>{n}</li>{/each}</ul>
          {:else if r && !fetched[`${a.id}@${r.version}`]}
            <div class="skels" aria-label="Loading release notes"><span class="skel" style:width="62%"></span><span class="skel" style:width="48%"></span><span class="skel" style:width="36%"></span></div>
          {/if}
        </div>
        <div class="nmore">
          {#if a.website && (r?.hasMoreNotes || extra?.hasMore || !notes.length)}
            <a href={a.website} class="more" onclick={(e) => { e.preventDefault(); api.openUrl(a.website!).catch((err) => store.say("err", err.message)); }}>Full release notes</a>
          {/if}
        </div>
      </div>
    {:else}
      {#if store.checking && !store.checkedAt}
        <div class="skcards" aria-busy="true" aria-label="Checking for updates">
          {#each { length: 2 } as _}
            <div class="card skcard"><span class="skel" style:width="36px" style:height="36px"></span>
              <span class="skl"><span class="skel" style:width="40%" style:height="13px"></span><span class="skel" style:width="25%" style:height="11px"></span></span></div>
          {/each}
        </div>
      {:else if !store.recent.length}
        <!-- Nothing else on the page: say so. Otherwise each row says it. -->
        <div class="panel empty">
          {#if !store.checkedAt}Not checked yet.{:else}<span class="ok"><Icon name="check" size={14} stroke={2.6} />All up to date</span>{/if}
        </div>
      {/if}
    {/each}
  </div>

  {#if store.recent.length}
    <div class="recent narrow">
      <h2 class="h2">Recently updated</h2>
      <div class="panel">
        {#each store.recent as r (r.id)}
          {@const a = store.addons.find((x) => x.id === r.id)}
          {@const st = store.status(r.id)?.kind}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (the name button opens it from the keyboard) -->
          <div class="row" class:open-card={!!a} onclick={(e) => a && clickRow(e, () => openDetails(a))}>
            <Tile name={r.name} iconUrl={a?.iconUrl ?? null} size={32} />
            <button type="button" class="rmeta" data-row-open aria-label="Details for {r.name}" disabled={!a}>
              <span class="rn">{r.name}</span>
              <span class="dim small">{r.from} → {r.to} · {when(r.at)}</span>
            </button>
            {#if store.busy[r.id]}
              <span class="dim small">{store.busy[r.id]}…</span>
            {:else}
              {#if store.checking}<span class="skel stsk" aria-label="Checking"></span>
              {:else if st === "up_to_date" || st === "skipped"}<span class="quiet">Up to date</span>
              {:else if st === "available"}<span class="avail">Update available</span>{/if}
              <RowMenu label="More actions for {r.name}" items={[{ label: `Roll back to ${r.from}`, run: () => store.rollback(r.id, r.name, r.from) }]} />
            {/if}
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
  .skcards { display: flex; flex-direction: column; gap: 12px; }
  .quiet { font-size: 12px; color: var(--text-3); }
  .done-st { width: 96px; justify-content: center; }
  .avail { font-size: 12px; font-weight: 600; color: var(--accent-text); }
  .stsk { width: 64px; height: 12px; }
  .skcard { flex-direction: row; align-items: center; }
  .skl { display: flex; flex-direction: column; gap: 7px; flex: 1; }
  .open-card:hover { background: var(--raised); }
  .skels { display: flex; flex-direction: column; gap: 8px; padding: 2px 0 2px 18px; }
  .skels .skel { height: 12px; }
  /* Room for three note lines and the link line, whatever arrives. */
  .nbody { min-height: 60px; }
  .nmore { min-height: 18px; margin-top: -6px; }
  .n { font-size: 15px; font-weight: 700; }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .line strong { color: var(--text); font-weight: 600; }
  .small { font-size: 11px; }
  .more { font-size: 12px; font-weight: 700; }
  .recent { display: flex; flex-direction: column; gap: 8px; margin-top: 4px; }
  .rmeta { display: flex; flex-direction: column; gap: 0; margin-right: auto; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .rmeta:disabled { cursor: default; }
  .rn { font-size: 13px; font-weight: 700; }
</style>
