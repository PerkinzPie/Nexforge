<script lang="ts">
  // An action button that becomes its own progress bar while the job runs:
  // the coral fill tracks the download, then "Unpacking…" while it is swapped in.
  let {
    label,
    busy = null,
    pct = null,
    ghost = false,
    disabled = false,
    onclick,
  }: {
    label: string;
    busy?: string | null;
    pct?: number | null;
    ghost?: boolean;
    disabled?: boolean;
    onclick: () => void;
  } = $props();

  const text = $derived(
    !busy ? label : pct == null ? `${busy}…` : pct >= 100 ? "Unpacking…" : `${pct}%`,
  );
</script>

{#if busy}
  <span class="pb" role="progressbar" aria-label="{busy} {label}" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct ?? undefined}>
    <span class="fill" class:indeterminate={pct == null || pct >= 100} style:width="{pct == null || pct >= 100 ? 100 : Math.max(4, pct)}%"></span>
    <span class="txt">{text}</span>
  </span>
{:else}
  <button type="button" class="btn" class:btn-row={!ghost} class:btn-ghost={ghost} {disabled} {onclick}>{label}</button>
{/if}

<style>
  .pb { position: relative; display: inline-flex; align-items: center; justify-content: center; min-width: 104px; height: 30px; padding: 0 13px; border-radius: 8px; background: var(--accent-tint); overflow: hidden; }
  .fill { position: absolute; inset: 0 auto 0 0; background: var(--accent-fill); transition: width 0.25s linear; }
  /* Size unknown, or unpacking: a solid block slides across (no gradients). */
  .fill.indeterminate { width: 35% !important; animation: slide-block 1.1s ease-in-out infinite; }
  .txt { position: relative; font-size: 12.5px; font-weight: 800; color: var(--text); font-variant-numeric: tabular-nums; }
  @keyframes slide-block { from { left: -35%; } to { left: 100%; } }
  @media (prefers-reduced-motion: reduce) { .fill.indeterminate { animation: none; width: 100% !important; left: 0; opacity: 0.55; } }
</style>
