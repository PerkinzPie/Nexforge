<script lang="ts">
  import Icon from "./Icon.svelte";
  import { store } from "../lib/store.svelte";

  function undo() {
    const u = store.toast?.undo;
    store.toast = null;
    u?.();
  }
</script>

{#if store.toast}
  <div class="toast" class:err={store.toast.kind === "err"} role={store.toast.kind === "err" ? "alert" : "status"}>
    <span class="msg">{store.toast.text}</span>
    {#if store.toast.undo}<button type="button" class="undo" onclick={undo}>Undo</button>{/if}
    <button type="button" class="btn btn-icon" aria-label="Dismiss" onclick={() => (store.toast = null)}><Icon name="x" size={14} /></button>
  </div>
{/if}

<style>
  .toast { max-width: 460px; display: flex; align-items: center; gap: 6px; padding: 8px 8px 8px 16px; border-radius: var(--r-md); border: 1px solid var(--border); background: var(--raised); font-size: 13px; font-weight: 600; }
  .toast.err { border-color: #562f39; background: var(--accent-tint); color: var(--accent-text); }
  .msg { margin-right: 6px; }
  .undo { height: 28px; padding: 0 12px; border: 0; border-radius: var(--r-sm); background: transparent; color: var(--info); font: 700 13px var(--sans); cursor: pointer; }
  .undo:hover { background: var(--raised-hover); }
</style>
