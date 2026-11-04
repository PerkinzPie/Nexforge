<script lang="ts">
  // Icon tile: the host's real icon when one is known (fetched by the core,
  // shown as a data: URL), else a monogram on a tinted tile.
  import { api } from "../lib/api";
  import { monogram, tint } from "../lib/format";

  let { name, iconUrl = null, size = 38, unknown = false }: { name: string; iconUrl?: string | null; size?: number; unknown?: boolean } = $props();

  let src = $state<string | null>(null);
  $effect(() => {
    src = null;
    const url = iconUrl;
    if (!url) return;
    let live = true;
    api.image(url).then((d) => live && (src = d)).catch(() => {});
    return () => { live = false; };
  });

  const [bg, fg] = $derived(tint(name));
  const radius = $derived(size >= 44 ? 12 : size >= 40 ? 11 : 10);
  const font = $derived(Math.round(size * 0.4));
</script>

<div
  class="tile"
  class:unknown
  style:width="{size}px"
  style:height="{size}px"
  style:border-radius="{radius}px"
  style:font-size="{font}px"
  style:background={unknown ? "var(--raised)" : bg}
  style:color={unknown ? "var(--text-3)" : fg}
>
  {#if src}<img {src} alt="" />{:else}{unknown ? "?" : monogram(name)}{/if}
</div>

<style>
  .tile { display: flex; align-items: center; justify-content: center; font-weight: 800; flex: 0 0 auto; overflow: hidden; }
  .tile.unknown { border: 1px dashed #3c3b58; }
  img { width: 100%; height: 100%; object-fit: cover; }
</style>
