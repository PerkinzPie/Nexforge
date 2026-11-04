<script lang="ts">
  import Icon from "../components/Icon.svelte";
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

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");
  const on = $derived(store.snap?.sourcesOn ?? []);

  // Debounced search; the GitHub index answers instantly, hosts in place.
  $effect(() => {
    const q = query.trim();
    const inst = store.current;
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

  const shown = $derived(
    hits
      .filter((h) => (query.trim() ? true : h.source === "github"))
      .filter((h) => source === "all" || h.source === source)
      .filter((h) => !onlyFlavour || h.forFlavour || h.flavourUnknown),
  );

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
    if (ok) hits = hits.map((x) => (x === h ? { ...x, installed: true } : x));
    else if (!anyway && store.toast?.text.includes("No release of")) {
      store.toast = null;
      store.dialog = { kind: "anyway", spec: h.spec, name: h.name };
    }
  }
</script>

<div class="page">
  <div class="page-head">
    <div class="page-title">
      <h1 class="h1">Browse</h1>
      <span class="dim">GitHub index{on.includes("wowinterface") ? " · WoWInterface" : ""}{on.includes("curseforge") ? " · CurseForge" : ""} · installs into {store.installation?.label ?? "WoW Forever"}</span>
    </div>
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
        <span class="dot" style:background={`var(--${{ github: "info", wago: "good", wowinterface: "attention", curseforge: "act" }[c.id]})`}></span>{c.label}
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

  <div class="results narrow">
    <span class="dim count">
      {#if loading && !hits.length}Searching…
      {:else if query.trim()}{shown.length} result{shown.length === 1 ? "" : "s"} for “{query.trim()}”{loading ? " · updating…" : ""}
      {:else}Addons in the GitHub index · type to search every source{/if}
    </span>
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
                <SourceTag source={h.source} label={h.sourceLabel} small />
                {#if h.alsoOn.length}<span class="dim tiny">+ {h.alsoOn.join(", ")}</span>{/if}
              </span>
              {#if h.summary}<span class="desc">{h.summary}</span>{/if}
              {#if !h.forFlavour && !h.flavourUnknown}
                <span class="nobuild">{h.flavours.join(", ") || "Other"} build only · no {flavour} build listed</span>
              {:else}
                <span class="dim tiny">
                  {[h.updatedAt ? `Updated ${ago(h.updatedAt)}` : "", h.downloads ? `${count(h.downloads)} downloads` : "", h.flavourUnknown ? "flavour shown per release" : ""].filter(Boolean).join(" · ")}
                </span>
              {/if}
            </div>
            {#if h.installed}
              <span class="ok"><Icon name="check" size={14} stroke={2.6} />Installed</span>
            {:else if busy}
              <span class="doing">{busy}{pct != null ? `… ${pct}%` : "…"}</span>
            {:else if !h.forFlavour && !h.flavourUnknown}
              <button type="button" class="btn btn-ghost" onclick={() => install(h, true)}>Install anyway</button>
            {:else}
              <button type="button" class="btn btn-row" onclick={() => install(h, false)}>Install</button>
            {/if}
          </div>
        {/each}
      </div>
    {:else if !loading && query.trim()}
      <div class="panel empty">Nothing found{onlyFlavour ? ` with a ${flavour} build` : ""}. {#if onlyFlavour}<a href="#all" onclick={(e) => { e.preventDefault(); onlyFlavour = false; }}>Show every flavour</a>, or paste{:else}Try another name, or paste{/if} a link with Install from URL.</div>
    {/if}
  </div>
</div>

<style>
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
  .doing { font-size: 12.5px; font-weight: 700; color: var(--accent-text); }
</style>
