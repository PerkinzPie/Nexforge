<script lang="ts">
  import ClientIcon from "../components/ClientIcon.svelte";
  import PageHead from "../components/PageHead.svelte";
  import RowMenu from "../components/RowMenu.svelte";
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
        <div class="sec-head">
          <h2>Game installations</h2>
          <button type="button" class="btn btn-sm" onclick={() => pick(null)}>Add installation</button>
        </div>
        <div class="group">
        {#each store.snap?.installations ?? [] as i (i.id)}
          <div class="frow">
            <ClientIcon id={i.id} flavour={i.flavour} size={28} />
            <div class="lbl"><span class="t">{i.label}</span><span class="mono s">{i.path}</span></div>
            {#if !i.detected}<span class="pill pill-attn">No AddOns folder</span>{/if}
            <button type="button" class="btn btn-sm" onclick={() => pick(i.id)}>Change</button>
          </div>
        {:else}
          <div class="frow"><div class="lbl"><span class="t">No installation found</span></div></div>
        {/each}
        </div>
      </section>

      <section class="sec">
        <h2>Updates</h2>
        <div class="group">
        <label class="frow toggle" for="check-launch">
          <span class="t">Check for updates on launch</span>
          <input id="check-launch" type="checkbox" checked={s.checkOnLaunch} onchange={(e) => save({ checkOnLaunch: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle" for="auto-apply">
          <Tip text="Pinned addons are never updated automatically.">{#snippet children({ props })}<span {...props} class="t">Apply updates automatically</span>{/snippet}</Tip>
          <input id="auto-apply" type="checkbox" checked={s.autoApply} onchange={(e) => save({ autoApply: e.currentTarget.checked })} />
        </label>
        <label class="frow field" for="keep-versions">
          <span class="t">Keep previous versions</span>
          <input id="keep-versions" type="number" min="0" max="10" value={s.keepPreviousVersions}
            onchange={(e) => save({ keepPreviousVersions: Math.max(0, Math.min(10, Number(e.currentTarget.value) || 0)) })} />
        </label>
        </div>
      </section>

      <section class="sec">
        <h2>Sources</h2>
        <div class="group">
        <label class="frow toggle" for="src-github">
          <Tip text="A token is optional. It raises GitHub's limit from 60 to 5,000 requests an hour.">
            {#snippet children({ props })}<span {...props} class="t">GitHub</span>{/snippet}
          </Tip>
          <RowMenu label="GitHub token" items={s.keyGithub
            ? [{ label: "Replace token", run: () => (store.dialog = { kind: "key", name: "github" }) }, { label: "Remove token", danger: true, run: () => clearKey("github") }]
            : [{ label: "Add token", run: () => (store.dialog = { kind: "key", name: "github" }) }]} />
          <input id="src-github" type="checkbox" checked={s.github} onchange={(e) => save({ github: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle" for="src-wowi">
          <span class="t">WoWInterface</span>
          <input id="src-wowi" type="checkbox" checked={s.wowinterface} onchange={(e) => save({ wowinterface: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle" class:off={!s.keyCurseforge} for="src-cf">
          <Tip text={s.keyCurseforge ? "Some authors block downloads outside the CurseForge app." : "Needs an API key, free from console.curseforge.com."}>
            {#snippet children({ props })}<span {...props} class="t">CurseForge</span>{/snippet}
          </Tip>
          <RowMenu label="CurseForge key" items={s.keyCurseforge
            ? [{ label: "Replace key", run: () => (store.dialog = { kind: "key", name: "curseforge" }) }, { label: "Remove key", danger: true, run: () => clearKey("curseforge") }]
            : [{ label: "Add key", run: () => (store.dialog = { kind: "key", name: "curseforge" }) }]} />
          <input id="src-cf" type="checkbox" checked={s.curseforge} disabled={!s.keyCurseforge}
            onchange={(e) => save({ curseforge: e.currentTarget.checked })} />
        </label>
        <label class="frow toggle off" for="src-wago">
          <Tip text="Wago only gives API keys to paying subscribers, so it is not supported.">{#snippet children({ props })}<span {...props} class="t dim-t">Wago</span>{/snippet}</Tip>
          <input id="src-wago" type="checkbox" disabled checked={false} aria-label="Wago (not supported)" />
        </label>
        </div>
      </section>

      <section class="sec">
        <h2>Privacy and diagnostics</h2>
        <div class="group">
        <label class="frow toggle" for="self-check">
          <span class="lead">
            <Tip text="Ticked: one request to GitHub on launch. Nothing is downloaded.">
              {#snippet children({ props })}<span {...props} class="t">Check for new versions of Nexforge</span>{/snippet}
            </Tip>
            <button type="button" class="btn btn-sm" disabled={store.checkingSelf} onclick={() => store.checkSelfNow()}>Check now</button>
          </span>
          <input id="self-check" type="checkbox" checked={s.selfUpdateCheck}
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
  /* A row with a tickbox is its label: a click anywhere on it toggles, except
     on its own buttons and menus. Rows that can't toggle keep the arrow. */
  .toggle { cursor: pointer; }
  .toggle.off, .field { cursor: default; }
  .toggle .t, .field .t { margin-right: auto; font-size: 13px; font-weight: 600; }
  .toggle .t.dim-t { color: var(--text-3); }
  /* A label with its own action right after it (Check now). */
  .lead { display: flex; align-items: center; gap: 12px; margin-right: auto; min-width: 0; }
  .lead .t { margin-right: 0; }
  .toggle input[type="checkbox"] { width: 18px; height: 18px; }
  /* A section's own action sits on its heading line, outside the list. */
  .sec-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 0 0 0 4px; }
  .sec-head h2 { margin: 0; font-size: 13px; font-weight: 700; color: var(--text-2); }
  .about { border: 1px solid var(--border); border-radius: var(--r-md); background: var(--surface); }
  .ab { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; padding: 16px 20px; }
  .abt { display: flex; flex-direction: column; gap: 4px; margin-right: auto; font-size: 12px; color: var(--text-soft); }
  .abt .wm { font-size: 20px; color: #fff; }
</style>
