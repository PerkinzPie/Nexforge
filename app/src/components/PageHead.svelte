<script lang="ts">
  // The same title row on every page: title on the left, actions on the right,
  // one fixed height, so switching pages never moves anything.
  import type { Snippet } from "svelte";

  let { title, children }: { title: string; children?: Snippet } = $props();
</script>

<div class="ph">
  <h1 class="h1">{title}</h1>
  {#if children}<div class="acts">{@render children()}</div>{/if}
</div>

<style>
  .ph { flex: 0 0 auto; display: flex; align-items: center; gap: 12px; height: 36px; }
  .ph h1 { margin-right: auto; white-space: nowrap; }
  .acts { display: flex; align-items: center; gap: 8px; }
</style>
