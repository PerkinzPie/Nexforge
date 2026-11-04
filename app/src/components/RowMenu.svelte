<script lang="ts" module>
  export interface Item { label: string; run: () => void; danger?: boolean; disabled?: boolean }
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { label, items }: { label: string; items: Item[] } = $props();
  let open = $state(false);
  let root: HTMLDivElement;
</script>

<div class="wrap" bind:this={root}>
  <button type="button" class="btn btn-icon" aria-label={label} aria-haspopup="menu" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name="more" />
  </button>
  {#if open}
    <div class="menu" role="menu">
      {#each items as it (it.label)}
        <button type="button" role="menuitem" class:danger={it.danger} disabled={it.disabled}
          onclick={() => { open = false; it.run(); }}>{it.label}</button>
      {/each}
    </div>
  {/if}
</div>

<svelte:window
  onclick={(e) => { if (open && !root.contains(e.target as Node)) open = false; }}
  onkeydown={(e) => { if (open && e.key === "Escape") open = false; }}
/>

<style>
  .wrap { position: relative; }
  .menu { position: absolute; right: 0; top: 36px; z-index: 30; min-width: 190px; padding: 6px; border-radius: 10px; border: 1px solid var(--border); background: var(--raised); display: flex; flex-direction: column; gap: 2px; }
  .menu button { height: 34px; padding: 0 10px; border: 0; border-radius: 7px; background: transparent; color: var(--text); font: 600 13px var(--sans); text-align: left; cursor: pointer; white-space: nowrap; }
  .menu button:hover:not(:disabled) { background: var(--raised-hover); }
  .menu button:disabled { color: var(--text-3); cursor: default; }
  .menu .danger { color: var(--accent-text); }
</style>
