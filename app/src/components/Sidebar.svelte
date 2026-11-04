<script lang="ts">
  import ClientIcon from "./ClientIcon.svelte";
  import Crest from "./Crest.svelte";
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api";
  import { plural } from "../lib/format";
  import { store, type View } from "../lib/store.svelte";

  let open = $state(false);

  const nav: { view: View; label: string; icon: "installed" | "updates" | "browse" | "settings" }[] = [
    { view: "installed", label: "Installed", icon: "installed" },
    { view: "updates", label: "Updates", icon: "updates" },
    { view: "browse", label: "Browse", icon: "browse" },
    { view: "settings", label: "Settings", icon: "settings" },
  ];

  const inst = $derived(store.installation);
  const updates = $derived(store.updates.length);
</script>

<nav aria-label="Main">
  <div class="brand"><Crest /><span class="wm">Nexforge</span></div>

  {#if inst}
    <div class="switch-wrap">
      <button type="button" class="switch" aria-haspopup="listbox" aria-expanded={open}
        aria-label="Switch game flavour, currently {inst.label}" onclick={() => (open = !open)}>
        <ClientIcon id={inst.id} flavour={inst.flavour} size={30} />
        <div class="who">
          <span class="n">{inst.label}</span>
          <span class="m">{plural(store.addons.length, "addon")}{store.checkedAt ? ` · ${plural(updates, "update")}` : ""}</span>
        </div>
        <Icon name="chevrons" colour="var(--text-2)" />
      </button>
      {#if open}
        <div class="menu" role="listbox">
          {#each store.snap?.installations ?? [] as i (i.id)}
            <button type="button" role="option" aria-selected={i.id === inst.id}
              onclick={() => { open = false; store.switchTo(i.id); }}>
              <ClientIcon id={i.id} flavour={i.flavour} size={24} />{i.label}
              {#if i.id === inst.id}<span class="cur"><Icon name="check" size={14} stroke={2.6} /></span>{/if}
            </button>
          {/each}
          <button type="button" class="add" onclick={() => { open = false; store.view = "settings"; }}>Add or change installations…</button>
        </div>
      {/if}
    </div>
  {/if}

  {#each nav as n (n.view)}
    <button type="button" class="nav" class:on={store.view === n.view} aria-current={store.view === n.view ? "page" : undefined}
      onclick={() => (store.view = n.view)}>
      <Icon name={n.icon} size={17} />
      {n.label}
      {#if n.view === "installed" && store.addons.length}<span class="count">{store.addons.length}</span>{/if}
      {#if n.view === "updates" && updates}<span class="badge">{updates}</span>{/if}
    </button>
  {/each}

  <div class="foot">
    {#if store.newVersion}
      <a class="newv" href={store.newVersion.url} onclick={(e) => { e.preventDefault(); api.openUrl(store.newVersion!.url).catch((err) => store.say("err", err.message)); }}>Nexforge {store.newVersion.version} is available</a>
    {/if}
    <a class="log" href="#log" onclick={(e) => { e.preventDefault(); api.open("log").catch((err) => store.say("err", err.message)); }}>Network log</a>
    <div class="nb">
      <svg width="18" height="20" viewBox="108 325 100 116" aria-hidden="true"><path fill="#D74653" d="m141.45 326.81-15.58-0.002-7.686 13.312c-4.2278 7.3222-7.7295 13.39-7.782 13.484l-0.096 0.171 7.7745 13.465 7.7738 13.465 15.589 0.01 15.588 0.01 7.7565-13.437c4.2659-7.3905 7.7558-13.475 7.755-13.521-0.00075-0.0459-3.4911-6.1299-7.7565-13.52l-7.755-13.436zm53.733 30.408-15.592-0.0105-7.755 13.434c-4.2659 7.3884-7.755 13.473-7.7542 13.521 0.00082 0.0479 3.4911 6.1332 7.7565 13.523l7.7542 13.436 15.579 0.001 15.579 0.002 2.2927-3.9719c1.261-2.1847 4.7642-8.253 7.785-13.485l5.49-9.51-7.77-13.462-7.7775-13.464zm-53.731 29.931-15.581-0.00075-0.72892 1.2614c-0.40115 0.69389-3.9022 6.7575-7.7805 13.475l-7.0519 12.214 0.0959 0.17139c0.0525 0.0938 3.5545 6.1616 7.782 13.484l7.6868 13.312 15.58-0.002 15.58-0.002 7.755-13.436c4.2651-7.3901 7.755-13.474 7.7565-13.52 0.00041-0.0459-3.4894-6.1304-7.755-13.52l-7.761-13.44z"></path></svg>
      <span>Nexabyte Entertainment</span>
    </div>
  </div>
</nav>

<svelte:window onclick={(e) => { if (open && !(e.target as Element).closest?.(".switch-wrap")) open = false; }} />

<style>
  nav { flex: 0 0 220px; height: 100%; display: flex; flex-direction: column; gap: 2px; padding: 18px 12px; background: var(--nx-indigo); overflow-y: auto; }
  .brand { display: flex; align-items: center; gap: 10px; padding: 2px 10px 16px 10px; }
  .brand .wm { font-size: 17px; color: #fff; }
  .switch-wrap { position: relative; margin-bottom: 12px; }
  .switch { width: 100%; display: flex; align-items: center; gap: 10px; height: 52px; padding: 0 8px 0 10px; border: 1px solid rgba(255, 255, 255, 0.12); border-radius: 10px; background: rgba(0, 0, 0, 0.22); text-align: left; cursor: pointer; color: inherit; }
  .switch:hover { background: rgba(0, 0, 0, 0.32); }
  .who { display: flex; flex-direction: column; gap: 0; min-width: 0; flex: 1; line-height: 1.3; }
  .who .n { font-size: 13px; font-weight: 700; color: #fff; }
  .who .n, .who .m { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .who .m { font-size: 11px; color: var(--text-2); }
  .menu { position: absolute; z-index: 20; top: 56px; left: 0; right: 0; padding: 6px; border-radius: 10px; border: 1px solid var(--border); background: var(--raised); display: flex; flex-direction: column; gap: 2px; }
  .menu button { display: flex; align-items: center; gap: 10px; height: 32px; padding: 0 8px; border: 0; border-radius: 7px; background: transparent; color: var(--text); font: 600 13px var(--sans); cursor: pointer; text-align: left; }
  .menu button:hover { background: var(--raised-hover); }
  .menu .cur { margin-left: auto; color: var(--good); display: flex; }
  .menu .add { color: var(--text-2); font-weight: 600; border-top: 1px solid var(--border); border-radius: 0 0 7px 7px; margin-top: 4px; padding-top: 4px; }
  .nav { display: flex; align-items: center; gap: 10px; height: 32px; padding: 0 10px; border-radius: 7px; border: 0; background: transparent; font-size: 13px; font-weight: 500; color: var(--text-soft); cursor: pointer; text-align: left; }
  .nav:hover { background: rgba(255, 255, 255, 0.06); color: var(--text); }
  .nav.on { background: rgba(255, 255, 255, 0.1); color: #fff; font-weight: 700; }
  .badge { margin-left: auto; min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: var(--accent-fill); color: var(--bg); font-size: 11px; font-weight: 700; display: flex; align-items: center; justify-content: center; }
  .count { margin-left: auto; font-size: 12px; color: var(--text-2); }
  .foot { margin-top: auto; display: flex; flex-direction: column; gap: 10px; padding: 14px 10px 0 10px; border-top: 1px solid rgba(255, 255, 255, 0.1); }
  .newv { font-size: 12px; font-weight: 700; color: var(--good); align-self: flex-start; }
  .log { font-size: 11px; line-height: 1.5; align-self: flex-start; }
  .nb { display: flex; align-items: center; gap: 8px; padding-top: 6px; }
  .nb span { font-size: 10px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; color: var(--text-2); }
</style>
