<script lang="ts">
  // First run: find the game clients, adopt each AddOns folder (writes
  // addons.toml only), show what was found, done.
  import ClientIcon from "../components/ClientIcon.svelte";
  import Logo from "../components/Logo.svelte";
  import Icon from "../components/Icon.svelte";
  import { api, type Adopted } from "../lib/api";
  import { plural } from "../lib/format";
  import { store } from "../lib/store.svelte";

  let rows = $state<Adopted[] | null>(null);
  let error = $state<string | null>(null);

  async function adopt() {
    error = null;
    rows = null;
    try {
      rows = await api.adoptAll();
    } catch (e) {
      error = (e as Error).message;
    }
  }

  async function addFolder() {
    error = null;
    try {
      const added = await api.addInstallation(null);
      if (added) await adopt();
    } catch (e) {
      error = (e as Error).message;
    }
  }

  $effect(() => {
    adopt();
  });
</script>

<div class="page welcome">
  <div class="box">
    <div class="lockup"><Logo width={64} /><span class="wm">Nexforge</span></div>
    <h1 class="h1">Your game clients</h1>

    {#if error}
      <div class="notice err"><Icon name="warn" size={16} /><span class="grow">{error}</span>{#if rows === null}<button type="button" class="btn btn-sm" onclick={adopt}>Try again</button>{/if}</div>
    {/if}

    <div class="panel list">
      {#if rows === null && !error}
        {#each store.snap?.installations ?? [] as i (i.id)}
          <div class="row">
            <ClientIcon id={i.id} flavour={i.flavour} size={36} />
            <div class="who"><span class="t">{i.label}</span><span class="mono s">{i.path}</span></div>
            <span class="dim small">Reading AddOns…</span>
          </div>
        {:else}
          <div class="row"><span class="dim">Looking for World of Warcraft…</span></div>
        {/each}
      {:else if rows?.length}
        {#each rows as r (r.installation.id)}
          <div class="row">
            <ClientIcon id={r.installation.id} flavour={r.installation.flavour} size={36} />
            <div class="who"><span class="t">{r.installation.label}</span><span class="mono s">{r.installation.path}</span></div>
            {#if !r.installation.detected}
              <span class="pill pill-attn">No AddOns folder</span>
            {:else}
              <span class="count">{plural(r.addons, "addon")}</span>
            {/if}
          </div>
        {/each}
      {:else if rows}
        <div class="row"><span class="dim">No World of Warcraft client found. Add its folder, or add one later in Settings.</span></div>
      {/if}
    </div>

    <div class="actions">
      <button type="button" class="btn" onclick={addFolder}>Add a folder</button>
      <button type="button" class="btn btn-primary" disabled={rows === null && !error} onclick={() => store.finishFirstRun()}>Done</button>
    </div>
  </div>
</div>

<style>
  .welcome { align-items: center; justify-content: center; }
  .box { width: 640px; max-width: 100%; display: flex; flex-direction: column; align-items: flex-start; gap: 18px; }
  .lockup { display: flex; align-items: center; gap: 12px; margin-bottom: 6px; }
  .lockup .wm { font-size: 24px; line-height: 1; color: var(--text); }
  .list { width: 100%; }
  .row { display: flex; align-items: center; gap: 14px; }
  .who { display: flex; flex-direction: column; gap: 2px; min-width: 0; margin-right: auto; }
  .t { font-size: 13px; font-weight: 700; }
  .s { font-size: 12px; color: var(--text-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .small { font-size: 12px; }
  .count { font: 600 13px var(--display); font-variant-numeric: tabular-nums; }
  .actions { display: flex; gap: 10px; align-self: flex-end; }
</style>
