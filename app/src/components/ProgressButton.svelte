<script lang="ts">
  // An action button that becomes its own progress bar while the job runs:
  // the coral fill tracks the download with its percentage. No words: before
  // the size is known and while unpacking, the bar alone moves.
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
    !busy ? label : pct == null || pct >= 100 ? "" : `${pct}%`,
  );
</script>

{#if busy}
  <span class="pb" role="progressbar" aria-label="{busy} {label}" aria-valuemin="0" aria-valuemax="100" aria-valuenow={pct ?? undefined}>
    <span class="fill" class:indeterminate={pct == null || pct >= 100} style:width="{pct == null || pct >= 100 ? 100 : Math.max(4, pct)}%"></span>
    <span class="txt">{text}</span>
  </span>
{:else}
  <button type="button" class="btn pbtn" class:btn-row={!ghost} class:btn-ghost={ghost} {disabled} {onclick}>{label}</button>
{/if}

<style>
  /* The button and its progress bar are the same fixed width, so nothing
     around them moves as an install runs ("Update" → 34%). */
  .pbtn { width: 96px; }
  .pb { position: relative; display: inline-flex; align-items: center; justify-content: center; width: 96px; height: 28px; padding: 0 6px; border-radius: var(--r-sm); background: var(--accent-tint); overflow: hidden; }
  .fill { position: absolute; inset: 0 auto 0 0; background: var(--accent-fill); transition: width 0.25s linear; }
  /* Size unknown, or unpacking: a solid block slides across (no gradients). */
  .fill.indeterminate { width: 35% !important; animation: slide-block 1.1s ease-in-out infinite; }
  .txt { position: relative; white-space: nowrap; font-size: 12px; font-weight: 700; color: var(--text); font-variant-numeric: tabular-nums; }
  @keyframes slide-block { from { left: -35%; } to { left: 100%; } }
  @media (prefers-reduced-motion: reduce) { .fill.indeterminate { animation: none; width: 100% !important; left: 0; opacity: 0.55; } }
</style>
