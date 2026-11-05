<script lang="ts">
  // Screenshots, one at a time, full size. ← and → (or the buttons) step
  // through; the strip below jumps; Escape or a click outside closes.
  import { Dialog } from "bits-ui";
  import Icon from "./Icon.svelte";
  import Image from "./Image.svelte";

  let {
    open = $bindable(false),
    index = $bindable(0),
    images,
    name,
  }: { open?: boolean; index?: number; images: string[]; name: string } = $props();

  const n = $derived(images.length);
  const go = (d: number) => (index = (index + d + n) % n);

  function keys(e: KeyboardEvent) {
    if (e.key === "ArrowRight") { e.preventDefault(); go(1); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); go(-1); }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="gal-scrim" />
    <Dialog.Content class="gal" onkeydown={keys} aria-describedby={undefined}>
      <div class="gbar">
        <Dialog.Title level={2} class="gal-title">{name}</Dialog.Title>
        <span class="count">{index + 1} of {n}</span>
        <Dialog.Close class="btn btn-icon" aria-label="Close"><Icon name="x" /></Dialog.Close>
      </div>

      <div class="stage">
        {#if n > 1}<button type="button" class="nav prev" aria-label="Previous screenshot" onclick={() => go(-1)}><Icon name="back" size={22} /></button>{/if}
        {#key index}<div class="shot"><Image url={images[index]} alt="Screenshot {index + 1} of {name}" fit="contain" /></div>{/key}
        {#if n > 1}<button type="button" class="nav next" aria-label="Next screenshot" onclick={() => go(1)}><Icon name="back" size={22} /></button>{/if}
      </div>

      {#if n > 1}
        <div class="strip" role="group" aria-label="Screenshots">
          {#each images as _, i (i)}
            <button type="button" class="th" class:on={i === index} aria-label="Screenshot {i + 1}" aria-current={i === index ? "true" : undefined}
              onclick={() => (index = i)}><Image url={images[i]} alt="" /></button>
          {/each}
        </div>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.gal-scrim) { position: fixed; inset: 0; z-index: 80; background: rgba(0, 0, 0, 0.85); }
  :global(.gal) { position: fixed; z-index: 81; inset: 24px; display: flex; flex-direction: column; gap: 12px; outline: none; }
  :global(.gal-title) { margin: 0 auto 0 0; font-size: 15px; font-weight: 700; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .gbar { display: flex; align-items: center; gap: 12px; }
  .count { font-size: 12px; color: var(--text-2); font-variant-numeric: tabular-nums; }
  .stage { position: relative; flex: 1 1 auto; min-height: 0; display: flex; align-items: center; justify-content: center; }
  .shot { width: 100%; height: 100%; border-radius: var(--r-md); overflow: hidden; }
  .nav { position: absolute; top: 50%; z-index: 1; display: flex; align-items: center; justify-content: center; width: 40px; height: 56px; margin-top: -28px; border: 1px solid var(--border); border-radius: var(--r-md); background: rgba(24, 25, 27, 0.8); color: var(--text); cursor: pointer; }
  .nav:hover { background: var(--raised); }
  .prev { left: 0; }
  .next { right: 0; }
  .next :global(svg) { transform: scaleX(-1); }
  .strip { flex: 0 0 auto; display: flex; gap: 8px; justify-content: center; overflow-x: auto; padding-bottom: 2px; }
  .th { flex: 0 0 auto; width: 96px; height: 54px; padding: 0; border: 2px solid transparent; border-radius: var(--r-sm); overflow: hidden; cursor: pointer; opacity: 0.6; }
  .th:hover { opacity: 0.9; }
  .th.on { border-color: var(--accent-fill); opacity: 1; }
</style>
