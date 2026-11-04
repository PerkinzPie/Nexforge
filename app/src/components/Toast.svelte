<script lang="ts">
  import Icon from "./Icon.svelte";
  import { store } from "../lib/store.svelte";
</script>

{#if store.toast}
  <div class="toast" class:err={store.toast.kind === "err"} role={store.toast.kind === "err" ? "alert" : "status"}>
    <span>{store.toast.text}</span>
    <button type="button" class="btn btn-icon" aria-label="Dismiss" onclick={() => (store.toast = null)}><Icon name="x" size={14} /></button>
  </div>
{/if}

<style>
  .toast { position: fixed; right: 24px; bottom: 24px; z-index: 60; max-width: 460px; display: flex; align-items: center; gap: 10px; padding: 10px 8px 10px 16px; border-radius: 12px; border: 1px solid var(--border); background: var(--raised); font-size: 13px; font-weight: 600; }
  .toast.err { border-color: #5a2a36; background: var(--accent-tint); color: var(--accent-text); }
</style>
