<script lang="ts">
  // The source an addon comes from, as a small neutral chip with its name.
  let { label, muted = false }: { source?: string | null; label: string; muted?: boolean } = $props();
</script>

<span class="srcchip" class:muted>{label}</span>

<style>
  .srcchip { display: inline-flex; align-items: center; height: 20px; padding: 0 8px; border-radius: 10px; background: var(--raised); color: var(--text-soft); font-size: 11px; font-weight: 700; white-space: nowrap; }
  .srcchip.muted { background: transparent; border: 1px solid var(--border); color: var(--text-2); font-weight: 600; }
</style>
