<script lang="ts">
  // Install from URL / Identify source / Remove / API key / Install anyway.
  import Icon from "./Icon.svelte";
  import { api } from "../lib/api";
  import { store } from "../lib/store.svelte";

  let value = $state("");
  let working = $state(false);
  let error = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  let cancel = $state<HTMLButtonElement | null>(null);

  const d = $derived(store.dialog);
  const flavour = $derived(store.snap?.flavourShort ?? "Forever");

  $effect(() => {
    if (!d) return;
    error = null;
    working = false;
    value = d.kind === "install" ? (d.initial ?? suggestion(d.adopt)) : "";
    // Nothing to type (Remove, Install anyway): focus Cancel, so Enter never
    // runs the action unread (HIG: a destructive button is never the default).
    queueMicrotask(() => (input ?? cancel)?.focus());
  });

  function suggestion(a: typeof store.addons[number] | undefined): string {
    if (!a) return "";
    // A host ID from the addon's own TOC is the best first guess.
    const known = a.knownIds.find(([s]) => s !== "wago" && (s !== "curseforge" || store.snap?.sourcesOn.includes("curseforge")));
    return known ? `${known[0]}:${known[1]}` : "";
  }

  function close() {
    if (!working) store.dialog = null;
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    // Keep our own copy: closing the dialog clears `store.dialog` (and `d`).
    const cur = store.dialog;
    if (!cur || working) return;
    working = true;
    error = null;
    try {
      if (cur.kind === "install") {
        const spec = value.trim();
        if (!spec) throw new Error("Paste a link or type an addon name.");
        store.dialog = null;
        const ok = await store.install(spec, cur.adopt ?? null, false, cur.adopt?.name ?? spec);
        if (!ok && store.toast?.text.includes("No release of")) {
          store.dialog = { kind: "anyway", spec, name: spec };
          store.toast = null;
        }
        return;
      }
      if (cur.kind === "anyway") {
        store.dialog = null;
        await store.install(cur.spec, null, true, cur.name);
        return;
      }
      if (cur.kind === "remove") {
        store.dialog = null;
        await store.remove(cur.addon);
        return;
      }
      if (cur.kind === "key") {
        const s = await api.setKey(cur.name, value);
        if (store.snap) store.snap.settings = s;
        store.dialog = null;
        store.say("ok", cur.name === "curseforge" ? "CurseForge key saved." : "GitHub token saved.");
        await store.load();
      }
    } catch (err) {
      // Errors after the dialog closed still reach the player.
      if (store.dialog) error = (err as Error).message;
      else store.say("err", (err as Error).message);
    } finally {
      working = false;
    }
  }
</script>

{#if d}
  <div class="scrim" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
    <div role="dialog" aria-modal="true" aria-labelledby="dlg-title" class="dlg-wrap">
    <form class="dlg" onsubmit={submit}>
      <div class="head">
        <h2 id="dlg-title">
          {#if d.kind === "install"}{d.adopt ? `Identify source for ${d.adopt.name}` : "Install from URL"}
          {:else if d.kind === "remove"}Remove {d.addon.name}?
          {:else if d.kind === "key"}{d.name === "curseforge" ? "CurseForge API key" : "GitHub token"}
          {:else}No {flavour} build listed{/if}
        </h2>
        <button type="button" class="btn btn-icon" aria-label="Close" onclick={close}><Icon name="x" /></button>
      </div>

      {#if d.kind === "install"}
        <p class="dim">A link, an addon name, or <span class="mono">github:owner/repo</span>.{#if d.adopt} Replaces {d.adopt.folders.join(", ")} (backed up first).{/if}</p>
        <label class="sr" for="dlg-in">Address or name</label>
        <input id="dlg-in" class="text-input" bind:this={input} bind:value placeholder="https://github.com/owner/repo" autocomplete="off" spellcheck="false" />
      {:else if d.kind === "remove"}
        <p class="dim">Deletes {d.addon.folders.join(", ")}. A backup is kept; WTF settings stay.</p>
      {:else if d.kind === "key"}
        <p class="dim">Saved in Windows Credential Manager.</p>
        <label class="sr" for="dlg-key">Key</label>
        <input id="dlg-key" class="text-input" type="password" bind:this={input} bind:value autocomplete="off" spellcheck="false" />
      {:else}
        <p class="dim">It may not load on {flavour}.</p>
      {/if}

      {#if error}<p class="err" role="alert">{error}</p>{/if}

      <div class="actions">
        <button type="button" class="btn" bind:this={cancel} onclick={close} disabled={working}>Cancel</button>
        <button type="submit" class="btn" class:btn-primary={d.kind !== "remove"} class:btn-danger={d.kind === "remove"} disabled={working}>
          {#if d.kind === "install"}Install{:else if d.kind === "remove"}Remove{:else if d.kind === "key"}Save key{:else}Install anyway{/if}
        </button>
      </div>
    </form>
    </div>
  </div>
{/if}

<svelte:window onkeydown={(e) => { if (d && e.key === "Escape") close(); }} />

<style>
  .scrim { position: fixed; inset: 0; z-index: 50; background: rgba(8, 8, 14, 0.6); display: flex; align-items: center; justify-content: center; padding: 24px; }
  .dlg-wrap { width: 520px; max-width: 100%; }
  .dlg { width: 100%; max-width: 100%; display: flex; flex-direction: column; gap: 12px; padding: 18px 20px; border-radius: 12px; border: 1px solid var(--border); background: var(--surface); }
  .head { display: flex; align-items: center; gap: 12px; }
  h2 { margin: 0 auto 0 0; font-size: 15px; font-weight: 700; }
  p { margin: 0; line-height: 1.4; }
  .err { color: var(--accent-text); font-weight: 600; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 4px; }
</style>
