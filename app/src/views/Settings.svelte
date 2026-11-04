<script lang="ts">
  import ClientIcon from "../components/ClientIcon.svelte";
  import { mergeProps } from "bits-ui";
  import PageHead from "../components/PageHead.svelte";
  import Tip from "../components/Tip.svelte";
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
  <PageHead title="Settings" />

  {#if s}
    <div class="secs narrow">
      <section class="sec">
        <h2>Game installations</h2>
        <div class="group">
        {#each store.snap?.installations ?? [] as i (i.id)}
          <div class="frow">
            <ClientIcon id={i.id} flavour={i.flavour} size={28} />
            <div class="lbl"><span class="t">{i.label}</span><span class="mono s">{i.path}</span></div>
            {#if !i.detected}<span class="pill pill-attn">No AddOns folder</span>{/if}
            <button type="button" class="btn btn-sm" onclick={() => pick(i.id)}>Change…</button>
          </div>
        {:else}
          <div class="frow"><div class="lbl"><span class="t">No installation found</span></div></div>
        {/each}
        <div class="frow end">
          <button type="button" class="btn btn-sm" onclick={() => pick(null)}>Add installation…</button>
        </div>
        </div>
      </section>

      <section class="sec">
        <h2>Updates</h2>
        <div class="group">
        <label class="frow toggle">
          <span class="t">Check for updates on launch</span>
          <input type="checkbox" checked={s.checkOnLaunch} onchange={(e) => save({ checkOnLaunch: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle">
          <Tip text="Pinned addons are never updated automatically.">{#snippet children({ props })}<span {...props} class="t">Apply updates automatically</span>{/snippet}</Tip>
          <input type="checkbox" checked={s.autoApply} onchange={(e) => save({ autoApply: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle">
          <span class="t">Keep previous versions</span>
          <input type="number" min="0" max="10" value={s.keepPreviousVersions}
            onchange={(e) => save({ keepPreviousVersions: Math.max(0, Math.min(10, Number(e.currentTarget.value) || 0)) })} />
        </label>
        </div>
      </section>

      <section class="sec">
        <h2>Sources</h2>
        <div class="group">
        <div class="frow toggle">
          <label class="t" for="src-github">GitHub</label>
          {#if s.keyGithub}
            <button type="button" class="btn btn-sm btn-quiet" onclick={() => clearKey("github")}>Remove token</button>
          {:else}
            <Tip text="Optional. Raises GitHub's limit from 60 to 5,000 requests an hour.">
              {#snippet children({ props })}<button {...mergeProps(props, { onclick: () => (store.dialog = { kind: "key", name: "github" }) })} type="button" class="btn btn-sm">Add token…</button>{/snippet}
            </Tip>
          {/if}
          <input id="src-github" type="checkbox" checked={s.github} onchange={(e) => save({ github: e.currentTarget.checked })} />
        </div>
        <label class="frow toggle">
          <span class="t">WoWInterface</span>
          <input type="checkbox" checked={s.wowinterface} onchange={(e) => save({ wowinterface: e.currentTarget.checked })} />
        </label>
        <div class="frow toggle">
          <label class="t" for="src-cf">CurseForge</label>
          {#if s.keyCurseforge}
            <button type="button" class="btn btn-sm btn-quiet" onclick={() => clearKey("curseforge")}>Remove key</button>
            <button type="button" class="btn btn-sm" onclick={() => (store.dialog = { kind: "key", name: "curseforge" })}>Replace key…</button>
          {:else}
            <Tip text="Free from console.curseforge.com. Some authors block downloads outside the CurseForge app.">
              {#snippet children({ props })}<button {...mergeProps(props, { onclick: () => (store.dialog = { kind: "key", name: "curseforge" }) })} type="button" class="btn btn-sm">Add key…</button>{/snippet}
            </Tip>
          {/if}
          <input id="src-cf" type="checkbox" checked={s.curseforge} disabled={!s.keyCurseforge}
            onchange={(e) => save({ curseforge: e.currentTarget.checked })} />
        </div>
        <div class="frow toggle">
          <Tip text="Wago only gives API keys to paying subscribers, so it is not supported.">{#snippet children({ props })}<span {...props} class="t off">Wago</span>{/snippet}</Tip>
          <input type="checkbox" disabled checked={false} aria-label="Wago (not supported)" />
        </div>
        </div>
      </section>

      <section class="sec">
        <h2>Privacy and diagnostics</h2>
        <div class="group">
        <label class="frow toggle">
          <Tip text="One request to GitHub on launch. Nothing is downloaded.">{#snippet children({ props })}<span {...props} class="t">Check for new versions of Nexforge</span>{/snippet}</Tip>
          <input type="checkbox" checked={s.selfUpdateCheck}
            onchange={(e) => {
              const on = e.currentTarget.checked;
              save({ selfUpdateCheck: on }).then(() => (on ? store.checkSelf() : (store.newVersion = null)));
            }} />
        </label>
        <div class="frow">
          <div class="lbl"><span class="t">Network log</span></div>
          <button type="button" class="btn btn-sm" onclick={() => open("log")}>Open log</button>
        </div>
        <div class="frow">
          <div class="lbl"><span class="t">Data folder</span><span class="mono s">{s.dataDir}</span></div>
          <button type="button" class="btn btn-sm" onclick={() => open("data")}>Open folder</button>
        </div>
        </div>
      </section>

      <section class="about">
        <div class="ab">
          <div class="abt">
            <span class="wm">Nexforge</span>
            <span>Version {s.version}</span>
            <span>Not affiliated with Blizzard Entertainment.</span>
          </div>
          <img src={wordmark} width="180" height="25" alt="Nexabyte" />
        </div>
      </section>
    </div>
  {/if}
</div>

<style>
  .secs { display: flex; flex-direction: column; gap: 20px; }
  .toggle { cursor: pointer; }
  .toggle .t { margin-right: auto; font-size: 13px; font-weight: 600; }
  .toggle .t.off { color: var(--text-3); }
  .toggle input[type="checkbox"] { width: 18px; height: 18px; }
  .end { justify-content: flex-end; }
  .about { border: 1px solid var(--border); border-radius: var(--r-md); background: var(--surface); }
  .ab { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; padding: 16px 20px; }
  .abt { display: flex; flex-direction: column; gap: 4px; margin-right: auto; font-size: 12px; color: var(--text-soft); }
  .abt .wm { font-size: 20px; color: #fff; }
</style>
