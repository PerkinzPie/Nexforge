<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
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

  // "Install anyway" says what it installs: the nearest build that is listed.
  const ORDER = ["Classic Era", "Anniversary", "Forever", "Retail"];
  function otherBuild(h: Hit): string {
    const pick = ORDER.find((f) => h.flavours.includes(f)) ?? h.flavours[0];
    return pick ? `Install ${pick} build` : "Install anyway";
  }
  const anywayTip = $derived(`No ${flavour} build is listed. It may not load in ${flavour}.`);
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
  /** Results the "Builds for" filter hid (no build listed for this client). */
  const hidden = $derived(inSource.length - shown.length);

  const chips = $derived([
    { id: "github", label: "GitHub", enabled: on.includes("github"), why: "Turned off in Settings" },
    { id: "wago", label: "Wago", enabled: false, why: "Wago needs a paid API key; not supported" },
    { id: "wowinterface", label: "WoWInterface", enabled: on.includes("wowinterface"), why: "Turned off in Settings" },
    { id: "curseforge", label: "CurseForge", enabled: on.includes("curseforge"), why: "Needs an API key (Settings)" },
  ]);

  // Picking a client filters to its builds and makes it the install target
  // (the same switch as the sidebar). "Any flavour" keeps the current target.
  async function pickBuilds(value: string) {
    if (value === "any") {
      onlyFlavour = false;
      return;
    }
    onlyFlavour = true;
    await store.switchTo(value);
  }

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
      <Icon name="link" size={15} stroke={2.2} />Install from URL
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
    <label class="builds" for="builds-for">
      <span>Builds for</span>
      <select id="builds-for" value={onlyFlavour ? store.current ?? "any" : "any"} onchange={(e) => pickBuilds(e.currentTarget.value)}>
        {#each store.snap?.installations ?? [] as i (i.id)}<option value={i.id}>{i.label}</option>{/each}
        <option value="any">Any flavour</option>
      </select>
    </label>
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
            <Tile name={h.name} iconUrl={h.iconUrl} size={48} />
            <div class="pt"><span class="pn">{h.name}</span>{#if h.authors.length}<span class="dim tiny">{h.authors[0]}</span>{/if}</div>
            {#if h.installed}
              <span class="ok done"><Icon name="check" size={14} stroke={2.6} />Installed</span>
            {:else if !h.forFlavour && !h.flavourUnknown}
              <ProgressButton label={otherBuild(h)} tip={anywayTip} ghost busy={store.busy[h.spec] ?? null} pct={store.progress[h.spec]} onclick={() => install(h, true)} />
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
          <div class="row">
            <Tile name={h.name} iconUrl={h.iconUrl} size={40} />
            <div class="body">
              <span class="line">
                <span class="n">{h.name}</span>
                {#if h.authors.length}<span class="dim small">by {h.authors.slice(0, 2).join(", ")}</span>{/if}
                <SourceTag label={h.sourceLabel} />
                {#each h.alsoOn as o (o)}<SourceTag label={o} muted />{/each}
              </span>
              {#if h.summary}<span class="desc">{h.summary}</span>{/if}
              {#if !h.forFlavour && !h.flavourUnknown}
                <span class="nobuild">No {flavour} build</span>
              {:else}
                <span class="dim tiny">
                  {[h.updatedAt ? `Updated ${ago(h.updatedAt)}` : "", h.downloads ? `${count(h.downloads)} downloads` : ""].filter(Boolean).join(" · ")}
                </span>
              {/if}
            </div>
            {#if h.installed}
              <span class="ok done"><Icon name="check" size={14} stroke={2.6} />Installed</span>
            {:else if !h.forFlavour && !h.flavourUnknown}
              <ProgressButton label={otherBuild(h)} tip={anywayTip} ghost busy={busy ?? null} {pct} onclick={() => install(h, true)} />
            {:else}
              <ProgressButton label="Install" busy={busy ?? null} {pct} onclick={() => install(h, false)} />
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    {#if onlyFlavour && hidden > 0 && query.trim()}
      <button type="button" class="more" onclick={() => (onlyFlavour = false)}>{hidden} more with no {flavour} build</button>
    {/if}
    {#if !shown.length && !hidden && !loading && query.trim()}
      <div class="panel empty">No results.</div>
    {/if}
  </div>
</div>

<style>
  .more { align-self: flex-start; padding: 0; border: 0; background: none; color: var(--info); font: 700 12.5px var(--sans); cursor: pointer; }
  .popular { display: flex; flex-direction: column; gap: 12px; }
  .pops { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
  .pop { display: flex; flex-direction: column; align-items: flex-start; gap: 12px; padding: 16px; border: 1px solid var(--border); border-radius: 14px; background: var(--surface); }
  .pt { display: flex; flex-direction: column; gap: 2px; min-width: 0; max-width: 100%; }
  .pn { font-size: 14px; font-weight: 800; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title { margin-right: auto; }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: -6px; }
  .builds { display: inline-flex; align-items: center; gap: 8px; margin-left: auto; font-size: 12.5px; font-weight: 600; color: var(--text-2); }
  .builds select { height: 32px; padding: 0 30px 0 12px; border: 1px solid var(--border); border-radius: 9px; background: var(--raised) url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23A8A8B0' stroke-width='2.4' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E") no-repeat right 10px center; color: var(--text); font: 600 12.5px var(--sans); appearance: none; cursor: pointer; }
  .builds select:hover { background-color: var(--raised-hover); }
  .results { display: flex; flex-direction: column; gap: 10px; }
  .count { font-size: 12.5px; font-weight: 600; }
  .body { display: flex; flex-direction: column; gap: 4px; flex: 1 1 300px; min-width: 0; }
  .line { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .n { font-size: 14.5px; font-weight: 800; }
  .small { font-size: 12px; }
  .tiny { font-size: 11.5px; }
  .nobuild { font-size: 11.5px; font-weight: 600; color: var(--attention); }
  .done { animation: pop 0.3s ease-out; }
  @keyframes pop { from { transform: scale(0.85); opacity: 0; } to { transform: none; opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .done { animation: none; } }
</style>
