<script lang="ts">
  // Above every page: search (typing goes to Browse) and the game client
  // switcher, which is also the install target.
  import { DropdownMenu } from "bits-ui";
  import ClientIcon from "./ClientIcon.svelte";
  import Icon from "./Icon.svelte";
  import { store } from "../lib/store.svelte";

  let { searchInput = $bindable() }: { searchInput?: HTMLInputElement } = $props();
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
    <DropdownMenu.Root>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button {...props} type="button" class="switch" aria-label="Game client, currently {inst.label}">
            <ClientIcon id={inst.id} flavour={inst.flavour} size={22} />
            <span class="n">{inst.label}</span>
            <Icon name="chevrons" size={14} colour="var(--text-2)" />
          </button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content class="dd" align="end" sideOffset={4} collisionPadding={8}>
          <DropdownMenu.RadioGroup value={inst.id} onValueChange={(v) => store.switchTo(v)}>
            {#each store.snap?.installations ?? [] as i (i.id)}
              <DropdownMenu.RadioItem value={i.id} class="dd-item">
                {#snippet children({ checked })}
                  <ClientIcon id={i.id} flavour={i.flavour} size={20} />{i.label}
                  {#if checked}<span class="cur"><Icon name="check" size={14} stroke={2.6} /></span>{/if}
                {/snippet}
              </DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.RadioGroup>
          <DropdownMenu.Separator class="dd-sep" />
          <DropdownMenu.Item class="dd-item" onSelect={() => (store.view = "settings")}>Add or change installations…</DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  {/if}
</header>


<style>
  .topbar { flex: 0 0 auto; display: flex; align-items: center; gap: 10px; padding: 12px 28px 4px; }
  .search { position: relative; flex: 1 1 auto; display: flex; align-items: center; }
  .search :global(svg) { position: absolute; left: 12px; pointer-events: none; }
  .search input { width: 100%; height: 38px; padding: 0 14px 0 38px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--surface); color: var(--text); font: 500 13px var(--sans); }
  .search input:hover { border-color: var(--raised-hover); }
  .switch { flex: 0 0 auto; display: flex; align-items: center; gap: 10px; height: 38px; min-width: 180px; padding: 0 10px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--surface); color: inherit; text-align: left; cursor: pointer; }
  .switch:hover { background: var(--raised); }
  .n { flex: 1; min-width: 0; font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
