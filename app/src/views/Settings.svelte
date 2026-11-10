<script lang="ts">
  import ClientIcon from "../components/ClientIcon.svelte";
  import Logo from "../components/Logo.svelte";
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

  async function removeClient(id: string, label: string) {
    try {
      await api.removeInstallation(id);
      await store.load(id === store.current ? null : store.current);
      store.say("ok", `${label} is off the list. Its addons are untouched.`);
    } catch (e) {
      store.say("err", (e as Error).message);
    }
  }

  async function clearKey(name: string) {
    try {
      const saved = await api.clearKey(name);
      if (store.snap) store.snap.settings = saved;
      await store.load();
      store.say("ok", "Key removed.");
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
            <button type="button" class="btn btn-sm" onclick={() => removeClient(i.id, i.label)}>Remove</button>
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
          <Tip text="After every check, on every game client. Pinned addons are never updated automatically.">{#snippet children({ props })}<span {...props} class="t">Apply updates automatically</span>{/snippet}</Tip>
          <input id="auto-apply" type="checkbox" checked={s.autoApply} onchange={(e) => save({ autoApply: e.currentTarget.checked })} />
        </label>
        <label class="frow field" for="keep-versions">
          <span class="t">Keep previous versions</span>
          <input id="keep-versions" type="number" min="1" max="10" value={s.keepPreviousVersions}
            onchange={(e) => save({ keepPreviousVersions: Math.max(1, Math.min(10, Number(e.currentTarget.value) || 1)) })} />
        </label>
        </div>
      </section>

      <section class="sec">
        <h2>Sources</h2>
        <div class="group">
        <label class="frow toggle" for="src-github">
          <Tip text="Optional. Raises GitHub's hourly limit.">
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
          <Tip text="Wago keys need a paid subscription.">{#snippet children({ props })}<span {...props} class="t dim-t">Wago</span>{/snippet}</Tip>
          <input id="src-wago" type="checkbox" disabled checked={false} aria-label="Wago (not supported)" />
        </label>
        </div>
      </section>

      <section class="sec">
        <h2>Privacy and diagnostics</h2>
        <div class="group">
        <label class="frow toggle" for="self-check">
          <Tip text="Never downloads or installs anything.">
            {#snippet children({ props })}<span {...props} class="t">Check for new versions of Nexforge</span>{/snippet}
          </Tip>
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

      <!-- About, as a quiet page footer rather than another card. -->
      <footer class="foot">
        <div class="foot-row">
          <Logo width={28} />
          <span class="wm">Nexforge</span>
          <span class="dim fv">Version {s.version}</span>
          <button type="button" class="btn btn-sm btn-quiet" disabled={store.checkingSelf} onclick={() => store.checkSelfNow()}>Check for updates</button>
          <img class="nb" src={wordmark} width="130" height="18" alt="Nexabyte" />
        </div>
        <p class="legal">Not affiliated with Blizzard Entertainment or any game studio. World of Warcraft is a trademark of Blizzard Entertainment, Inc. © 2026 Nexabyte Ltd. Released under the MIT licence.</p>
      </footer>
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
  .toggle input[type="checkbox"] { width: 18px; height: 18px; }
  /* A section's own action sits on its heading line, outside the list. */
  .sec-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 0 0 0 4px; }
  .sec-head h2 { margin: 0; font-size: 13px; font-weight: 700; color: var(--text-2); }
  .foot { display: flex; flex-direction: column; gap: 10px; margin-top: 4px; padding: 16px 4px 4px; border-top: 1px solid var(--divider); }
  .foot-row { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .foot-row .wm { font-size: 14px; color: var(--text); }
  .fv { font-size: 12px; font-variant-numeric: tabular-nums; }
  .nb { margin-left: auto; opacity: 0.8; }
  .legal { margin: 0; max-width: 80ch; font-size: 11px; line-height: 1.5; color: var(--text-3); }
</style>
