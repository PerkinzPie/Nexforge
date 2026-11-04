<script lang="ts">
  import Activity from "./components/Activity.svelte";
  import Dialogs from "./components/Dialogs.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Toast from "./components/Toast.svelte";
  import TopBar from "./components/TopBar.svelte";
  import { store } from "./lib/store.svelte";
  import Browse from "./views/Browse.svelte";
  import Details from "./views/Details.svelte";
  import Installed from "./views/Installed.svelte";
  import Settings from "./views/Settings.svelte";
  import Updates from "./views/Updates.svelte";
  import Welcome from "./views/Welcome.svelte";

  let filterInput = $state<HTMLInputElement>();
  let searchInput = $state<HTMLInputElement>();

  $effect(() => {
    store.start();
  });

  // Ctrl+F filter/search · Ctrl+R check for updates · Ctrl+, settings
  function keys(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    if (e.key === "f" || e.key === "F") {
      e.preventDefault();
      // Installed filters its list; everywhere else searches addons.
      (store.view === "installed" && !store.detail ? filterInput : searchInput)?.focus();
    } else if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      store.check();
    } else if (e.key === ",") {
      e.preventDefault();
      store.view = "settings";
    }
  }
</script>

<svelte:window onkeydown={keys} />

<div class="shell">
  <Sidebar />
  <main>
    {#if store.preview}
      <div class="preview">Browser preview · sample data. Run <span class="mono">pnpm tauri dev</span> for the real app.</div>
    {/if}
    {#if store.snap && !store.firstRun}<TopBar bind:searchInput />{/if}
    {#if store.loadError}
      <div class="page"><div class="notice err"><span>{store.loadError}</span></div></div>
    {:else if !store.snap}
      <!-- Local state loads in milliseconds; no spinner. -->
    {:else if store.firstRun}
      <Welcome />
    {:else if store.detail}
      {#key store.detail}<Details target={store.detail} />{/key}
    {:else if store.view === "installed"}
      <Installed bind:filterInput />
    {:else if store.view === "updates"}
      <Updates />
    {:else if store.view === "browse"}
      <Browse />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<Dialogs />
<div class="dock" aria-live="polite">
  <Activity />
  <Toast />
</div>

<style>
  .shell { display: flex; height: 100%; min-width: 900px; }
  main { flex: 1 1 auto; min-width: 0; height: 100%; display: flex; flex-direction: column; }
  .dock { position: fixed; right: 24px; bottom: 24px; z-index: 60; display: flex; flex-direction: column; align-items: flex-end; gap: 10px; pointer-events: none; }
  .dock > :global(*) { pointer-events: auto; }
  .preview { flex: 0 0 auto; padding: 6px 32px; font-size: 12px; font-weight: 600; color: var(--attention); background: var(--attn-tint); }
</style>
