<script lang="ts">
  // Jobs with no row on screen to show progress on (Install from URL, Restore).
  import { store } from "../lib/store.svelte";

  const jobs = $derived(Object.entries(store.jobs));
</script>

{#each jobs as [key, j] (key)}
  {@const pct = store.progress[key]}
  {@const label = store.busy[key] ?? "Working"}
  <div class="job" role="status" aria-live="polite">
    <div class="top">
      <span class="t">{j.title}</span>
      <span class="p">{pct == null ? `${label}…` : pct >= 100 ? "Unpacking…" : `${pct}%`}</span>
    </div>
    <span class="bar wide" class:indeterminate={pct == null || pct >= 100}><span style:width="{pct ?? 30}%"></span></span>
  </div>
{/each}

<style>
  .job { width: 320px; display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border-radius: 12px; border: 1px solid var(--border); background: var(--raised); }
  .top { display: flex; align-items: baseline; gap: 10px; }
  .t { font-size: 13px; font-weight: 700; margin-right: auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .p { font-size: 12px; font-weight: 700; color: var(--accent-text); font-variant-numeric: tabular-nums; }
</style>
