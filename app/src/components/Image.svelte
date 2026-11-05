<script lang="ts">
  // A host image (screenshot), fetched by the core through the image allowlist
  // and shown as a data: URL. Pulses while loading; blank if it can't load.
  import { api } from "../lib/api";

  let { url, alt, fit = "cover" }: { url: string; alt: string; fit?: "cover" | "contain" } = $props();
  let src = $state<string | null>(null);
  let failed = $state(false);

  $effect(() => {
    src = null;
    failed = false;
    const u = url;
    let live = true;
    api.image(u, "screenshot").then((d) => live && (src = d)).catch(() => live && (failed = true));
    return () => { live = false; };
  });
</script>

{#if src}<img {src} {alt} style:object-fit={fit} />{:else if !failed}<span class="skel ph" aria-hidden="true"></span>{/if}

<style>
  img { display: block; width: 100%; height: 100%; }
  .ph { width: 100%; height: 100%; border-radius: 0; }
</style>
