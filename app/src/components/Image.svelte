<script lang="ts">
  // A host image (screenshot), fetched by the core through the image allowlist
  // and shown as a data: URL. Blank until it arrives; nothing on failure.
  import { api } from "../lib/api";

  let { url, alt }: { url: string; alt: string } = $props();
  let src = $state<string | null>(null);

  $effect(() => {
    src = null;
    const u = url;
    let live = true;
    api.image(u).then((d) => live && (src = d)).catch(() => {});
    return () => { live = false; };
  });
</script>

{#if src}<img {src} {alt} />{/if}

<style>
  img { display: block; width: 100%; height: 100%; object-fit: cover; }
</style>
