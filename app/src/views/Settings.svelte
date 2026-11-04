<script lang="ts">
  import ClientIcon from "../components/ClientIcon.svelte";
  import Icon from "../components/Icon.svelte";
  import wordmark from "../assets/nexabyte-wordmark.svg";
  import { api, type Settings, type SettingsInput } from "../lib/api";
  import { store } from "../lib/store.svelte";

  const s = $derived(store.snap?.settings);

  async function save(patch: Partial<SettingsInput>) {
    if (!s) return;
    const next: SettingsInput = {
      checkOnLaunch: s.checkOnLaunch, autoApply: s.autoApply, keepPreviousVersions: s.keepPreviousVersions,
      selfUpdateCheck: s.selfUpdateCheck, github: s.github, wowinterface: s.wowinterface, curseforge: s.curseforge,
      ...patch,
    };
    try {
      const saved: Settings = await api.saveSettings(next);
      if (store.snap) store.snap.settings = saved;
      await store.load();
    } catch (e) {
      store.say("err", (e as Error).message);
    }
  }

  async function pick(replace: string | null) {
    try {
      const added = await api.addInstallation(replace);
      if (added) {
        store.say("ok", `Added ${added.label} (${added.path}).`);
        await store.load(added.id);
      }
    } catch (e) {
      store.say("err", (e as Error).message);
    }
  }

  async function clearKey(name: string) {
    try {
      const saved = await api.clearKey(name);
      if (store.snap) store.snap.settings = saved;
      await store.load();
      store.say("ok", "Key removed from Windows Credential Manager.");
    } catch (e) {
      store.say("err", (e as Error).message);
    }
  }

  const open = (what: "log" | "data") => api.open(what).catch((e) => store.say("err", e.message));
</script>

<div class="page">
  <div class="page-title">
    <h1 class="h1">Settings</h1>
  </div>

  {#if s}
    <div class="secs narrow">
      <section class="sec">
        <h2><Icon name="home" size={15} stroke={2.2} />Game installations</h2>
        {#each store.snap?.installations ?? [] as i (i.id)}
          <div class="frow">
            <ClientIcon id={i.id} flavour={i.flavour} size={36} />
            <div class="lbl"><span class="t">{i.label}</span><span class="mono s">{i.path}</span></div>
            {#if i.detected}<span class="pill pill-ok">Detected</span>{:else}<span class="pill pill-attn">No AddOns folder</span>{/if}
            <button type="button" class="btn btn-sm" onclick={() => pick(i.id)}>Change</button>
          </div>
        {:else}
          <div class="frow"><div class="lbl"><span class="t">No installation found</span><span class="s">Choose a client folder such as <span class="mono">_forever_</span>.</span></div></div>
        {/each}
        <div class="frow">
          <button type="button" class="btn btn-sm" onclick={() => pick(null)}>Add installation</button>
        </div>
      </section>

      <section class="sec">
        <h2><Icon name="refresh" size={15} stroke={2.2} />Updates</h2>
        <div class="frow">
          <div class="lbl"><span class="t">Check for updates on launch</span></div>
          <label class="sw" for="chk-launch"><input id="chk-launch" type="checkbox" checked={s.checkOnLaunch} onchange={(e) => save({ checkOnLaunch: e.currentTarget.checked })} />{s.checkOnLaunch ? "On" : "Off"}</label>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Apply updates automatically</span><span class="s">Pinned addons are skipped.</span></div>
          <label class="sw" for="chk-auto"><input id="chk-auto" type="checkbox" checked={s.autoApply} onchange={(e) => save({ autoApply: e.currentTarget.checked })} />{s.autoApply ? "On" : "Off"}</label>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Keep previous versions</span><span class="s">For rollback.</span></div>
          <label class="sw" for="keep"><input id="keep" type="number" min="0" max="10" value={s.keepPreviousVersions}
            onchange={(e) => save({ keepPreviousVersions: Math.max(0, Math.min(10, Number(e.currentTarget.value) || 0)) })} />versions</label>
        </div>
      </section>

      <section class="sec">
        <h2><Icon name="globe" size={15} stroke={2.2} />Sources</h2>
        <div class="frow">
          <div class="lbl"><span class="t">GitHub</span><span class="s">A token is optional and raises the rate limit.</span></div>
          <label class="sw"><input type="checkbox" checked={s.github} onchange={(e) => save({ github: e.currentTarget.checked })} />{s.github ? "Enabled" : "Off"}</label>
          {#if s.keyGithub}
            <button type="button" class="btn btn-sm" onclick={() => clearKey("github")}>Remove token</button>
          {:else}
            <button type="button" class="btn btn-sm" onclick={() => (store.dialog = { kind: "key", name: "github" })}>Add token</button>
          {/if}
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">WoWInterface</span></div>
          <label class="sw"><input type="checkbox" checked={s.wowinterface} onchange={(e) => save({ wowinterface: e.currentTarget.checked })} />{s.wowinterface ? "Enabled" : "Off"}</label>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Wago</span><span class="s">Needs a paid key.</span></div>
          <span class="pill pill-off">Not supported</span>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">CurseForge</span><span class="s">Needs an API key.</span></div>
          {#if s.keyCurseforge}
            <span class="pill pill-ok">Key saved</span>
            <button type="button" class="btn btn-sm" onclick={() => (store.dialog = { kind: "key", name: "curseforge" })}>Replace key</button>
            <button type="button" class="btn btn-sm btn-quiet" onclick={() => clearKey("curseforge")}>Remove</button>
          {:else}
            <span class="pill pill-off">No key</span>
            <button type="button" class="btn btn-sm" onclick={() => (store.dialog = { kind: "key", name: "curseforge" })}>Add key</button>
          {/if}
        </div>
      </section>

      <section class="sec">
        <h2><Icon name="shield" size={15} stroke={2.2} />Privacy and diagnostics</h2>
        <div class="frow">
          <div class="lbl"><span class="t">Check for new versions of Nexforge</span><span class="s">Coming later.</span></div>
          <label class="sw"><input type="checkbox" disabled checked={false} />Off</label>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Network log</span><span class="s">Every request Nexforge makes.</span></div>
          <button type="button" class="btn btn-sm" onclick={() => open("log")}>Open log</button>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Data folder</span><span class="mono s">{s.dataDir}</span></div>
          <button type="button" class="btn btn-sm" onclick={() => open("data")}>Open folder</button>
        </div>
      </section>

      <section class="sec about">
        <div class="ab">
          <div class="abt">
            <span class="wm">Nexforge</span>
            <span>Version {s.version} · open source</span>
            <span>Not affiliated with Blizzard Entertainment.</span>
          </div>
          <img src={wordmark} width="180" height="25" alt="Nexabyte" />
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .secs { display: flex; flex-direction: column; gap: 16px; }
  .about { background: var(--nx-indigo); border-color: #3a3860; }
  .ab { display: flex; flex-wrap: wrap; align-items: center; gap: 20px; padding: 20px 24px; }
  .abt { display: flex; flex-direction: column; gap: 6px; margin-right: auto; font-size: 12.5px; color: var(--text-soft); }
  .abt .wm { font-size: 20px; color: #fff; }
</style>
