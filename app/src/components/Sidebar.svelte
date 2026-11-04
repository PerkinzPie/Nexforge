<script lang="ts">
  // The icon rail: crest, the three main views, Settings at the foot, and the
  // version. Labels show as tooltips; counts as badges.
  import Crest from "./Crest.svelte";
  import { mergeProps } from "bits-ui";
  import Icon from "./Icon.svelte";
  import Tip from "./Tip.svelte";
  import { api } from "../lib/api";
  import { store, type View } from "../lib/store.svelte";

  const nav: { view: View; label: string; icon: "installed" | "updates" | "browse" }[] = [
    { view: "installed", label: "Installed", icon: "installed" },
    { view: "updates", label: "Updates", icon: "updates" },
    { view: "browse", label: "Browse", icon: "browse" },
  ];

  const updates = $derived(store.updates.length);
  const on = (v: View) => store.view === v && !store.detail;
</script>

<nav aria-label="Main">
  <button type="button" class="crest" aria-label="Nexforge: Installed" onclick={() => (store.view = "installed")}><Crest size={30} /></button>

  {#each nav as n (n.view)}
    <Tip text={n.label} side="right">
      {#snippet children({ props })}
        <button {...mergeProps(props, { onclick: () => (store.view = n.view) })} type="button" class="item" class:on={on(n.view)}
          aria-current={on(n.view) ? "page" : undefined} aria-label={n.view === "updates" && updates ? `Updates, ${updates} available` : n.label}>
          <Icon name={n.icon} size={20} />
          {#if n.view === "updates" && updates}<span class="badge">{updates}</span>{/if}
        </button>
      {/snippet}
    </Tip>
  {/each}

  <div class="foot">
    {#if store.newVersion}
      <Tip text="Nexforge {store.newVersion.version} is available" side="right">
        {#snippet children({ props })}
          <button {...mergeProps(props, { onclick: () => api.openUrl(store.newVersion!.url).catch((err) => store.say("err", err.message)) })}
            type="button" class="item new" aria-label="Nexforge {store.newVersion!.version} is available">
            <Icon name="download" size={20} />
          </button>
        {/snippet}
      </Tip>
    {/if}
    <Tip text="Settings" side="right">
      {#snippet children({ props })}
        <button {...mergeProps(props, { onclick: () => (store.view = "settings") })} type="button" class="item" class:on={on("settings")}
          aria-current={on("settings") ? "page" : undefined} aria-label="Settings">
          <Icon name="settings" size={20} />
        </button>
      {/snippet}
    </Tip>
    <span class="ver" title="Nexforge {store.snap?.settings.version ?? ''}">{store.snap?.settings.version ?? ""}</span>
  </div>
</nav>

<style>
  nav { flex: 0 0 64px; height: 100%; display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 14px 0 12px; background: var(--rail); border-right: 1px solid var(--divider); }
  .crest { display: flex; padding: 4px; margin-bottom: 12px; border: 0; border-radius: var(--r-sm); background: none; cursor: pointer; }
  .item { position: relative; display: flex; align-items: center; justify-content: center; width: 44px; height: 40px; border: 0; border-radius: var(--r-sm); background: transparent; color: var(--text-3); cursor: pointer; }
  .item:hover { background: var(--surface); color: var(--text); }
  .item.on { background: var(--accent-tint); color: var(--accent-text); box-shadow: inset 0 0 0 1px #5a2c33; }
  .item.new { color: var(--good); }
  .badge { position: absolute; top: 3px; right: 3px; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 8px; background: var(--accent-fill); color: var(--bg); font-size: 10px; font-weight: 700; line-height: 16px; text-align: center; font-variant-numeric: tabular-nums; }
  .foot { margin-top: auto; display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .ver { font-size: 10px; color: var(--text-3); font-variant-numeric: tabular-nums; }
</style>
