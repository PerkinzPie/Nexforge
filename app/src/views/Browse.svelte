<script lang="ts">
  import AddonCard from "../components/AddonCard.svelte";
  import Icon from "../components/Icon.svelte";
  import NoBuild from "../components/NoBuild.svelte";
  import PageHead from "../components/PageHead.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api, type Hit } from "../lib/api";
  import { ago, count } from "../lib/format";
  import { clickRow } from "../lib/menu";
  import { store } from "../lib/store.svelte";

  // The top bar owns the search box.
  const query = $derived(store.query);
  let source = $state<string>("all");
  let onlyFlavour = $state(true);
  let hits = $state<Hit[]>([]);
  let errors = $state<{ sourceLabel: string; message: string }[]>([]);
  let loading = $state(false);
  /** The query the current `hits` answer; anything else is stale. */
  let hitsFor = $state<string | null>(null);
  let seq = 0;
  let popular = $state<Hit[]>([]);
  /** False until the first answer, so the grids show placeholders, not nothing. */
  let popLoaded = $state(false);
  let topLoaded = $state(false);
  /** With nothing typed and a host chip picked: that host's most downloaded. */
  let top = $state<Hit[]>([]);
  /** Bumped when the catalogue finishes refreshing, to re-run search. */
  let refreshed = $state(0);

  // Refill the local catalogue in the background (at most once a day).
  $effect(() => {
    api.refreshCatalogue(false).then(() => refreshed++).catch(() => {});
  });

  $effect(() => {
    const inst = store.current;
    void refreshed;
    api.popular(inst).then((p) => (popular = p)).catch(() => (popular = [])).finally(() => (popLoaded = true));
  });

  $effect(() => {
    const inst = store.current;
    const src = source;
    void refreshed;
    if (src === "all" || src === "github") {
      top = [];
      return;
    }
    topLoaded = false;
    api.popular(inst, src).then((p) => (top = p)).catch(() => (top = [])).finally(() => (topLoaded = true));
  });

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");
  const noBuild = (h: Hit) => !h.forFlavour && !h.flavourUnknown;
  const openHit = (h: Hit) => (store.detail = { name: h.name, iconUrl: h.iconUrl, source: h.source, sourceId: h.sourceId, hit: h });
  const on = $derived(store.snap?.sourcesOn ?? []);

  // Debounced search; the GitHub index answers instantly, hosts in place.
  $effect(() => {
    const q = query.trim();
    const inst = store.current;
    void refreshed;
    const mine = ++seq;
    loading = true;
    const t = setTimeout(async () => {
      try {
        const r = await api.search(inst, q);
        if (mine !== seq) return;
        hits = r.hits;
        hitsFor = q;
        errors = r.errors;
      } catch (e) {
        if (mine === seq) errors = [{ sourceLabel: "Search", message: (e as Error).message }];
      } finally {
        if (mine === seq) loading = false;
      }
    }, q ? 300 : 0);
    return () => clearTimeout(t);
  });

  const inSource = $derived(
    !query.trim() && source !== "all" && source !== "github"
      ? top
      : hits
          .filter((h) => (query.trim() ? true : h.source === "github"))
          .filter((h) => source === "all" || h.source === source),
  );
  const stale = $derived(hitsFor !== query.trim());
  const shown = $derived(inSource.filter((h) => !onlyFlavour || h.forFlavour || h.flavourUnknown));
  /** Results with no build listed for this client (hidden unless asked for). */
  const other = $derived(inSource.filter((h) => noBuild(h)).length);

  const chips = $derived([
    { id: "github", label: "GitHub", enabled: on.includes("github"), why: "Turned off in Settings" },
    { id: "wago", label: "Wago", enabled: false, why: "Wago needs a paid API key; not supported" },
    { id: "wowinterface", label: "WoWInterface", enabled: on.includes("wowinterface"), why: "Turned off in Settings" },
    { id: "curseforge", label: "CurseForge", enabled: on.includes("curseforge"), why: "Needs an API key (Settings)" },
  ]);

  async function install(h: Hit, anyway: boolean) {
    const ok = await store.install(h.spec, null, anyway);
    if (ok) {
      hits = hits.map((x) => (x.spec === h.spec ? { ...x, installed: true } : x));
      popular = popular.map((x) => (x.spec === h.spec ? { ...x, installed: true } : x));
      top = top.map((x) => (x.spec === h.spec ? { ...x, installed: true } : x));
    }
    else if (!anyway && store.toast?.text.includes("No release of")) {
      store.toast = null;
      store.dialog = { kind: "anyway", spec: h.spec, name: h.name };
    }
  }
</script>

<div class="page">
  <PageHead title="Browse">
    <button type="button" class="btn" onclick={() => (store.dialog = { kind: "install" })} disabled={!store.current}>
      <Icon name="link" size={15} stroke={2.2} />Install from URL
    </button>
  </PageHead>

  <div class="filters narrow">
    <button type="button" class="chip" class:on={source === "all"} onclick={() => (source = "all")}>All sources</button>
    {#each chips as c (c.id)}
      <button type="button" class="chip" class:on={source === c.id} disabled={!c.enabled} title={c.enabled ? "" : c.why}
        onclick={() => (source = c.id)}>
        {c.label}
      </button>
    {/each}
  </div>

  {#each errors as e (e.sourceLabel)}
    <div class="notice err narrow"><Icon name="warn" size={16} /><span>{#if !e.message.startsWith(e.sourceLabel)}<strong>{e.sourceLabel}:</strong> {/if}{e.message}</span></div>
  {/each}

  {#if !query.trim() && source === "all" && (popular.length || !popLoaded)}
    <section class="popular narrow" aria-labelledby="pop-h" aria-busy={!popLoaded}>
      <h2 class="h2" id="pop-h">Popular this month</h2>
      <div class="grid">
        {#if !popLoaded}
          {#each { length: 6 } as _}<SkeletonCard />{/each}
        {:else}
          {#each popular as h (h.spec)}
            <AddonCard hit={h} onopen={() => openHit(h)} oninstall={(anyway) => install(h, anyway)} />
          {/each}
        {/if}
      </div>
    </section>
  {/if}

  <div class="results narrow">
    <!-- The count's line is kept while results load, so they never jump. -->
    {#if query.trim()}
      <span class="dim count">{#if shown.length && !(loading && stale)}{shown.length} result{shown.length === 1 ? "" : "s"}{/if}</span>
    {/if}
    {#if !query.trim() && (source === "all" ? loading && stale : !topLoaded)}
      <section class="popular" aria-busy="true">
        <h2 class="h2">{source === "all" ? "On GitHub" : `Most downloaded on ${chips.find((c) => c.id === source)?.label ?? ""}`}</h2>
        <div class="grid">{#each { length: 4 } as _}<SkeletonCard />{/each}</div>
      </section>
    {:else if query.trim() && loading && stale}
      <div class="panel" aria-busy="true" aria-label="Searching">
        {#each { length: 5 } as _}
          <div class="row hit sk"><span class="skel" style:width="36px" style:height="36px"></span>
            <span class="body"><span class="skel" style:width="32%" style:height="12px"></span><span class="skel" style:width="68%" style:height="11px"></span></span></div>
        {/each}
      </div>
    {:else if shown.length && !query.trim()}
      <section class="popular" aria-labelledby="list-h">
        <h2 class="h2" id="list-h">{source === "all" ? "On GitHub" : `Most downloaded on ${chips.find((c) => c.id === source)?.label ?? ""}`}</h2>
        <div class="grid">
          {#each shown as h (h.spec)}
            <AddonCard hit={h} onopen={() => openHit(h)} oninstall={(anyway) => install(h, anyway)} />
          {/each}
        </div>
      </section>
    {:else if shown.length}
      <div class="panel">
        {#each shown as h (h.spec)}
          {@const busy = store.busy[h.spec]}
          {@const pct = store.progress[h.spec]}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (the name button opens it from the keyboard) -->
          <div class="row hit open-row" onclick={(e) => clickRow(e, () => openHit(h))}>
            <button type="button" class="open" data-row-open aria-label="Details for {h.name}">
            <Tile name={h.name} iconUrl={h.iconUrl} size={36} />
            <span class="body">
              <span class="line">
                <span class="n">{h.name}</span>
                {#if noBuild(h)}<NoBuild text="No {flavour} build" size={14} />{/if}
                {#if h.authors.length}<span class="dim small">{h.authors.slice(0, 2).join(", ")}</span>{/if}
                <SourceTag label={h.sourceLabel} />
                {#each h.alsoOn as o (o)}<SourceTag label={o} muted />{/each}
              </span>
              {#if h.summary}<span class="desc" title={h.summary}>{h.summary}</span>{/if}
            </span>
            </button>
            <span class="side dim">
              {#if h.downloads}<span>{count(h.downloads)} downloads</span>{/if}
              {#if h.updatedAt}<span>Updated {ago(h.updatedAt)}</span>{/if}
            </span>
            <div class="act">
              {#if h.installed}
                <span class="ok done"><Icon name="check" size={14} stroke={2.6} />Installed</span>
              {:else if noBuild(h) && !busy}
                <RowMenu label="More actions for {h.name}" items={[{ label: "Install anyway", run: () => install(h, true) }]} />
              {:else}
                <ProgressButton label="Install" busy={busy ?? null} {pct} onclick={() => install(h, noBuild(h))} />
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
    {#if other > 0 && (query.trim() || source !== "all")}
      <button type="button" class="more" onclick={() => (onlyFlavour = !onlyFlavour)}>
        {onlyFlavour ? `Show ${other} with no ${flavour} build` : `Hide ${other} with no ${flavour} build`}
      </button>
    {/if}
    {#if !shown.length && !other && !loading && query.trim()}
      <div class="panel empty">No results.</div>
    {/if}
  </div>
</div>

<style>
  .more { align-self: flex-start; padding: 0; border: 0; background: none; color: var(--info); font: 700 12px var(--sans); cursor: pointer; }
  .popular { display: flex; flex-direction: column; gap: 12px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 10px; }
  /* Tile and text open the details page; actions stay separate buttons. */
  .open { display: flex; align-items: center; gap: 12px; flex: 1 1 auto; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .open-row { cursor: pointer; }
  .open-row:hover { background: var(--raised); }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .results { display: flex; flex-direction: column; gap: 10px; }
  .count { min-height: 16px; line-height: 16px; font-size: 12px; font-weight: 600; }
  .body { display: flex; flex-direction: column; gap: 2px; flex: 1 1 300px; min-width: 0; }
  /* Two lines per row (name, summary), as HIG lists: details sit at the
     trailing edge, so every row is the same height whatever it holds. */
  .hit { flex-wrap: nowrap; }
  .hit .body { height: 38px; justify-content: center; }
  .hit .line { height: 20px; }
  .hit .desc { line-height: 16px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .side { flex: 0 0 auto; display: flex; flex-direction: column; align-items: flex-end; gap: 2px; font-size: 11px; line-height: 16px; white-space: nowrap; font-variant-numeric: tabular-nums; }
  .line { display: flex; flex-wrap: nowrap; align-items: center; gap: 8px; min-width: 0; overflow: hidden; }
  .line > :global(*) { flex-shrink: 0; }
  .line .n, .line .small { flex-shrink: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* Every state (Install, progress, Installed, ⋯) is centred in the same
     96px, so their centres line up down the column. */
  .act { flex: 0 0 auto; width: 96px; display: flex; justify-content: center; }
  .n { font-size: 13px; font-weight: 700; }
  .small { font-size: 12px; }
  .done { animation: pop 0.3s ease-out; }
  @keyframes pop { from { transform: scale(0.85); opacity: 0; } to { transform: none; opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .done { animation: none; } }
</style>
