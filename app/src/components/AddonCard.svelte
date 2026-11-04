<script lang="ts">
  // A Browse card: the addon's artwork leads, then name, one line of summary
  // and author/downloads. The card opens details; the corner button installs.
  import { mergeProps } from "bits-ui";
  import Icon from "./Icon.svelte";
  import NoBuild from "./NoBuild.svelte";
  import Tip from "./Tip.svelte";
  import RowMenu from "./RowMenu.svelte";
  import Tile from "./Tile.svelte";
  import type { Hit } from "../lib/api";
  import { count } from "../lib/format";
  import { clickRow } from "../lib/menu";
  import { store } from "../lib/store.svelte";

  let { hit, onopen, oninstall }: { hit: Hit; onopen: () => void; oninstall: (anyway: boolean) => void } = $props();

  const noBuild = $derived(!hit.forFlavour && !hit.flavourUnknown);
  const busy = $derived(store.busy[hit.spec]);
  const pct = $derived(store.progress[hit.spec]);
  const flavour = $derived(store.snap?.flavourShort ?? "Forever");
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (the name button opens it from the keyboard) -->
<div class="card-a" onclick={(e) => clickRow(e, onopen)}>
  <button type="button" class="open" data-row-open aria-label="Details for {hit.name}">
    <Tile name={hit.name} iconUrl={hit.iconUrl} size={64} />
    <span class="t">
      <span class="nl"><span class="n">{hit.name}</span>{#if noBuild}<NoBuild text="No {flavour} build" size={14} />{/if}</span>
      {#if hit.summary}<span class="s">{hit.summary}</span>{/if}
      <span class="m">
        {#if hit.authors.length}<span><Icon name="user" size={12} stroke={2.2} />{hit.authors[0]}</span>{/if}
        {#if hit.downloads}<span><Icon name="download" size={12} stroke={2.2} />{count(hit.downloads)}</span>{/if}
        {#if !hit.authors.length && !hit.downloads}<span>{hit.sourceLabel}</span>{/if}
      </span>
    </span>
  </button>
  <div class="a">
    {#if hit.installed}
      <Tip text="Installed">{#snippet children({ props })}<span {...props} class="done" aria-label="Installed"><Icon name="check" size={15} stroke={2.6} /></span>{/snippet}</Tip>
    {:else if busy}
      <span class="pct" role="progressbar" aria-label="{busy} {hit.name}" aria-valuenow={pct ?? undefined}>{pct == null || pct >= 100 ? "…" : `${pct}%`}</span>
    {:else if noBuild}
      <RowMenu label="More actions for {hit.name}" items={[{ label: "Install anyway", run: () => oninstall(true) }]} />
    {:else}
      <Tip text="Install">
        {#snippet children({ props })}
          <button {...mergeProps(props, { onclick: () => oninstall(false) })} type="button" class="btn btn-icon get" aria-label="Install {hit.name}">
            <Icon name="download" size={16} stroke={2.4} />
          </button>
        {/snippet}
      </Tip>
    {/if}
  </div>
</div>

<style>
  .card-a { cursor: pointer; position: relative; display: flex; align-items: center; gap: 8px; min-width: 0; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--surface); }
  .card-a:hover { background: var(--raised); }
  .open { display: flex; align-items: center; gap: 14px; flex: 1 1 auto; min-width: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .t { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .n { font-size: 13px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .s { font-size: 12px; color: var(--text-soft); line-height: 16px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .nl { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .m { display: flex; gap: 12px; font-size: 11px; color: var(--text-2); white-space: nowrap; overflow: hidden; }
  .m > span { display: inline-flex; align-items: center; gap: 4px; }
  .a { flex: 0 0 auto; align-self: flex-start; display: flex; }
  .get { color: var(--accent-text); }
  .get:hover { background: var(--accent-tint); color: var(--accent-text); }
  .done { display: flex; width: 28px; height: 28px; align-items: center; justify-content: center; color: var(--good); }
  .pct { display: flex; align-items: center; height: 28px; padding: 0 8px; border-radius: var(--r-sm); background: var(--accent-tint); color: var(--accent-text); font-size: 11px; font-weight: 700; font-variant-numeric: tabular-nums; }
</style>
