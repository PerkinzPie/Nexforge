<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api, type Hit } from "../lib/api";
  import { ago, count } from "../lib/format";
  import { store } from "../lib/store.svelte";

  let { searchInput = $bindable() }: { searchInput?: HTMLInputElement } = $props();

  let query = $state("");
  let source = $state<string>("all");
  let onlyFlavour = $state(true);
  let hits = $state<Hit[]>([]);
  let errors = $state<{ sourceLabel: string; message: string }[]>([]);
  let loading = $state(false);
  let seq = 0;
  let popular = $state<Hit[]>([]);
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
    api.popular(inst).then((p) => (popular = p)).catch(() => (popular = []));
  });

  $effect(() => {
    const inst = store.current;
    const src = source;
    void refreshed;
    if (src === "all" || src === "github") {
      top = [];
      return;
    }
    api.popular(inst, src).then((p) => (top = p)).catch(() => (top = []));
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
  <div class="page-head">
    <h1 class="h1 title">Browse</h1>
    <button type="button" class="btn" onclick={() => (store.dialog = { kind: "install" })} disabled={!store.current}>
      <Icon name="link" size={15} stroke={2.2} />Install from URL…
    </button>
  </div>

  <div class="search big narrow">
    <label for="q" class="sr">Search addons</label>
    <Icon name="search" size={18} colour="var(--text-3)" />
    <input id="q" type="search" bind:value={query} bind:this={searchInput} placeholder="Search by name or author" autocomplete="off" spellcheck="false" />
  </div>

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
    <div class="notice err narrow"><Icon name="warn" size={16} /><span><strong>{e.sourceLabel}:</strong> {e.message}</span></div>
  {/each}

  {#if !query.trim() && source === "all" && popular.length}
    <section class="popular narrow" aria-labelledby="pop-h">
      <h2 class="h2" id="pop-h">Popular this month</h2>
      <div class="pops">
        {#each popular as h (h.spec)}
          <div class="pop">
            <button type="button" class="open" onclick={() => openHit(h)} aria-label="Details for {h.name}">
              <Tile name={h.name} iconUrl={h.iconUrl} size={48} />
              <span class="pt">
                <span class="pn">{h.name}</span>
                <span class="pm">
                  {#if h.authors.length}<span><Icon name="user" size={12} stroke={2.2} />{h.authors[0]}</span>{/if}
                  {#if h.downloads}<span><Icon name="download" size={12} stroke={2.2} />{count(h.downloads)}</span>{/if}
                </span>
              </span>
            </button>
            {#if h.installed}
              <span class="ok done" aria-label="Installed"><Icon name="check" size={14} stroke={2.6} /></span>
            {:else if noBuild(h) && !store.busy[h.spec]}
              <RowMenu label="More actions for {h.name}" items={[{ label: "Install anyway", run: () => install(h, true) }]} />
            {:else}
              <ProgressButton label="Install" busy={store.busy[h.spec] ?? null} pct={store.progress[h.spec]} onclick={() => install(h, false)} />
            {/if}
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <div class="results narrow">
    {#if query.trim() || (loading && !hits.length)}
      <span class="dim count">{loading && !hits.length ? "Searching…" : `${shown.length} result${shown.length === 1 ? "" : "s"}`}</span>
    {/if}
    {#if shown.length}
      <div class="panel">
        {#each shown as h (h.spec)}
          {@const busy = store.busy[h.spec]}
          {@const pct = store.progress[h.spec]}
          <div class="row hit">
            <button type="button" class="open" onclick={() => openHit(h)} aria-label="Details for {h.name}">
            <Tile name={h.name} iconUrl={h.iconUrl} size={36} />
            <span class="body">
              <span class="line">
                <span class="n">{h.name}</span>
                {#if h.authors.length}<span class="dim small">{h.authors.slice(0, 2).join(", ")}</span>{/if}
                <SourceTag label={h.sourceLabel} />
                {#each h.alsoOn as o (o)}<SourceTag label={o} muted />{/each}
              </span>
              {#if h.summary}<span class="desc" title={h.summary}>{h.summary}</span>{/if}
            </span>
            </button>
            {#if noBuild(h)}
              <span class="side nobuild">No {flavour} build</span>
            {:else}
              <span class="side dim">
                {#if h.downloads}<span>{count(h.downloads)} downloads</span>{/if}
                {#if h.updatedAt}<span>Updated {ago(h.updatedAt)}</span>{/if}
              </span>
            {/if}
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
  .pops { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px; }
  .pop { display: flex; align-items: center; gap: 10px; padding: 12px 14px 12px 12px; border: 1px solid var(--border); border-radius: 12px; background: var(--surface); }
  .pop:hover { background: var(--raised); }
  .pt { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .pn { font-size: 13px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pm { display: flex; gap: 10px; font-size: 11px; color: var(--text-2); white-space: nowrap; overflow: hidden; }
  .pm > span { display: inline-flex; align-items: center; gap: 4px; }
  /* Tile and text open the details page; actions stay separate buttons. */
  .open { display: flex; align-items: center; gap: 12px; flex: 1 1 auto; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .open:hover .n, .open:hover .pn { text-decoration: underline; text-underline-offset: 3px; }
  .title { margin-right: auto; }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: -8px; }
  .results { display: flex; flex-direction: column; gap: 10px; }
  .count { font-size: 12px; font-weight: 600; }
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
  .act { flex: 0 0 auto; min-width: 88px; display: flex; justify-content: flex-end; }
  .n { font-size: 13px; font-weight: 700; }
  .small { font-size: 12px; }
  .nobuild { font-size: 11px; font-weight: 600; color: var(--attention); }
  .done { animation: pop 0.3s ease-out; }
  @keyframes pop { from { transform: scale(0.85); opacity: 0; } to { transform: none; opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .done { animation: none; } }
</style>
