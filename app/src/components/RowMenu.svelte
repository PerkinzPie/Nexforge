<script lang="ts" module>
  export interface Item { label: string; run: () => void; danger?: boolean; disabled?: boolean }
</script>

<script lang="ts">
  // A row's ⋯ menu. Drawn as a fixed layer at the button's position so a
  // scrolling or clipped parent (the table panel) can never hide it.
  import Icon from "./Icon.svelte";

  let { label, items }: { label: string; items: Item[] } = $props();
  let open = $state(false);
  let pos = $state({ top: 0, left: 0, up: false });
  let button: HTMLButtonElement;
  let menu = $state<HTMLDivElement | null>(null);

  const WIDTH = 200;

  function toggle() {
    if (open) return close();
    const r = button.getBoundingClientRect();
    const height = items.length * 36 + 12;
    const up = r.bottom + height + 8 > window.innerHeight;
    pos = {
      top: up ? r.top - height - 4 : r.bottom + 4,
      left: Math.max(8, Math.min(r.right - WIDTH, window.innerWidth - WIDTH - 8)),
      up,
    };
    open = true;
    queueMicrotask(() => menu?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus());
  }

  function close(refocus = false) {
    open = false;
    if (refocus) button?.focus();
  }

  function keys(e: KeyboardEvent) {
    const all = [...(menu?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") { e.preventDefault(); close(true); }
    else if (e.key === "ArrowDown") { e.preventDefault(); all[(i + 1) % all.length]?.focus(); }
    else if (e.key === "ArrowUp") { e.preventDefault(); all[(i - 1 + all.length) % all.length]?.focus(); }
    else if (e.key === "Tab") close();
  }
</script>

<button type="button" class="btn btn-icon" bind:this={button} aria-label={label} aria-haspopup="menu" aria-expanded={open} onclick={toggle}>
  <Icon name="more" />
</button>

{#if open}
  <div class="menu" role="menu" tabindex="-1" bind:this={menu} style:top="{pos.top}px" style:left="{pos.left}px" style:width="{WIDTH}px" onkeydown={keys}>
    {#each items as it (it.label)}
      <button type="button" role="menuitem" class:danger={it.danger} disabled={it.disabled}
        onclick={() => { close(); it.run(); }}>{it.label}</button>
    {/each}
  </div>
{/if}

<svelte:window
  onclick={(e) => { if (open && !menu?.contains(e.target as Node) && !button.contains(e.target as Node)) close(); }}
  onresize={() => open && close()}
  onscrollcapture={() => open && close()}
/>

<style>
  .menu { position: fixed; z-index: 70; padding: 6px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--raised); display: flex; flex-direction: column; gap: 2px; }
  .menu button { height: 34px; padding: 0 10px; border: 0; border-radius: var(--r-sm); background: transparent; color: var(--text); font: 600 13px var(--sans); text-align: left; cursor: pointer; white-space: nowrap; }
  .menu button:hover:not(:disabled), .menu button:focus-visible { background: var(--raised-hover); outline: none; }
  .menu button:disabled { color: var(--text-3); cursor: default; }
  .menu .danger { color: var(--accent-text); }
</style>
