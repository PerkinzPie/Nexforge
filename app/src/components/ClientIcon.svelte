<script lang="ts" module>
  // One lookup per installation for the whole session.
  const cache = new Map<string, Promise<string | null>>();
</script>

<script lang="ts">
  // The game client's own icon (read from its .exe by the backend); a
  // letter tile only if the client has none.
  import { api } from "../lib/api";
  import { flavourTile } from "../lib/format";

  let { id, flavour, size = 34 }: { id: string; flavour: string; size?: number } = $props();

  let src = $state<string | null>(null);
  $effect(() => {
    src = null;
    const key = id;
    if (!cache.has(key)) cache.set(key, api.clientIcon(key).catch(() => null));
    let live = true;
    cache.get(key)!.then((d) => live && (src = d));
    return () => { live = false; };
  });
  const t = $derived(flavourTile(flavour));
</script>

{#if src}
  <img class="ci" {src} alt="" width={size} height={size} style:width="{size}px" style:height="{size}px" />
{:else}
  <span class="ft" style:width="{size}px" style:height="{size}px" style:font-size="{Math.round(size * 0.46)}px" style:background={t.bg}>{t.letter}</span>
{/if}

<style>
  .ci { flex: 0 0 auto; object-fit: contain; }
  .ft { border-radius: var(--r-sm); display: flex; align-items: center; justify-content: center; font-family: var(--display); font-weight: 700; color: #fff; flex: 0 0 auto; }
</style>
