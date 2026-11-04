<script lang="ts">
  // Above every page: search (typing goes to Browse) and the game client
  // switcher, which is also the install target.
  import ClientIcon from "./ClientIcon.svelte";
  import Icon from "./Icon.svelte";
  import { plural } from "../lib/format";
  import { store } from "../lib/store.svelte";

  let { searchInput = $bindable() }: { searchInput?: HTMLInputElement } = $props();
  let open = $state(false);
  const inst = $derived(store.installation);

  function typed() {
    if (store.view !== "browse" || store.detail) store.view = "browse";
  }
</script>

<header class="topbar">
  <div class="search">
    <label for="q" class="sr">Search addons</label>
    <Icon name="search" size={16} colour="var(--text-3)" />
    <input id="q" type="search" bind:value={store.query} bind:this={searchInput} oninput={typed}
      placeholder="Search addons" autocomplete="off" spellcheck="false" />
  </div>

  {#if inst}
    <div class="switch-wrap">
      <button type="button" class="switch" aria-haspopup="listbox" aria-expanded={open}
        aria-label="Game client, currently {inst.label}" onclick={() => (open = !open)}>
        <ClientIcon id={inst.id} flavour={inst.flavour} size={22} />
        <span class="who">
          <span class="n">{inst.label}</span>
          <span class="m">{plural(store.addons.length, "addon")}</span>
        </span>
        <Icon name="chevrons" size={14} colour="var(--text-2)" />
      </button>
      {#if open}
        <div class="menu" role="listbox">
          {#each store.snap?.installations ?? [] as i (i.id)}
            <button type="button" role="option" aria-selected={i.id === inst.id}
              onclick={() => { open = false; store.switchTo(i.id); }}>
              <ClientIcon id={i.id} flavour={i.flavour} size={22} />{i.label}
              {#if i.id === inst.id}<span class="cur"><Icon name="check" size={14} stroke={2.6} /></span>{/if}
            </button>
          {/each}
          <button type="button" class="add" onclick={() => { open = false; store.view = "settings"; }}>Add or change installations…</button>
        </div>
      {/if}
    </div>
  {/if}
</header>

<svelte:window onclick={(e) => { if (open && !(e.target as Element).closest?.(".switch-wrap")) open = false; }}
  onkeydown={(e) => { if (open && e.key === "Escape") open = false; }} />

<style>
  .topbar { flex: 0 0 auto; display: flex; align-items: center; gap: 10px; padding: 12px 28px 4px; }
  .search { position: relative; flex: 1 1 auto; display: flex; align-items: center; }
  .search :global(svg) { position: absolute; left: 12px; pointer-events: none; }
  .search input { width: 100%; height: 38px; padding: 0 14px 0 38px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: var(--text); font: 500 13px var(--sans); }
  .search input:hover { border-color: var(--raised-hover); }
  .switch-wrap { position: relative; flex: 0 0 auto; }
  .switch { display: flex; align-items: center; gap: 10px; height: 38px; min-width: 190px; padding: 0 10px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface); color: inherit; text-align: left; cursor: pointer; }
  .switch:hover { background: var(--raised); }
  .who { display: flex; flex-direction: column; flex: 1; min-width: 0; line-height: 1.2; }
  .who .n { font-size: 12px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .who .m { font-size: 10px; color: var(--text-2); }
  .menu { position: absolute; z-index: 30; top: 42px; right: 0; width: 240px; padding: 6px; border-radius: 10px; border: 1px solid var(--border); background: var(--raised); display: flex; flex-direction: column; gap: 2px; }
  .menu button { display: flex; align-items: center; gap: 10px; height: 32px; padding: 0 8px; border: 0; border-radius: 7px; background: transparent; color: var(--text); font: 600 13px var(--sans); cursor: pointer; text-align: left; }
  .menu button:hover { background: var(--raised-hover); }
  .menu .cur { margin-left: auto; color: var(--good); display: flex; }
  .menu .add { color: var(--text-2); border-top: 1px solid var(--border); border-radius: 0 0 7px 7px; margin-top: 4px; padding-top: 4px; }
</style>
