<script lang="ts">
  // One addon: what it is, its screenshots, versions, and the same actions as
  // its row. Paints from what the row already knew, then fills in from the host.
  import Gallery from "../components/Gallery.svelte";
  import Icon from "../components/Icon.svelte";
  import Image from "../components/Image.svelte";
  import NoBuild from "../components/NoBuild.svelte";
  import ProgressButton from "../components/ProgressButton.svelte";
  import RowMenu from "../components/RowMenu.svelte";
  import SourceTag from "../components/SourceTag.svelte";
  import Tile from "../components/Tile.svelte";
  import { api, type Details } from "../lib/api";
  import { ago, count, plural, shortDate, size } from "../lib/format";
  import { addonMenu } from "../lib/menu";
  import { store, type DetailTarget } from "../lib/store.svelte";

  let { target }: { target: DetailTarget } = $props();

  let info = $state<Details | null>(null);
  let error = $state<string | null>(null);
  let galleryOpen = $state(false);
  let galleryAt = $state(0);
  const shots = $derived(info?.screenshots ?? []);
  // Only promise screenshots we know are coming: on WoWInterface the icon is
  // the first screenshot's thumbnail; GitHub has none; CurseForge can't say.
  const expectShots = $derived(target.source === "wowinterface" && !!target.iconUrl);
  const thumbs = $derived(info && info.screenshotThumbs.length === shots.length ? info.screenshotThumbs : shots);
  function view(i: number) {
    galleryAt = i;
    galleryOpen = true;
  }

  $effect(() => {
    const t = target;
    info = null;
    error = null;
    if (!t.source) return;
    api.details(t.source, t.sourceId).then((d) => (info = d)).catch((e) => (error = (e as Error).message));
  });

  const flavour = $derived(store.snap?.flavourShort ?? "Forever");
  // The installed copy, if any (opened from Browse it is matched by source).
  const addon = $derived(
    store.addons.find((a) => a.id === target.addonId) ??
      store.addons.find((a) => a.managed && a.source === target.source && a.sourceId === target.sourceId),
  );
  const s = $derived(addon ? store.status(addon.id) : undefined);
  const hit = $derived(target.hit);
  const latest = $derived(info?.releases[0]);
  const builds = $derived(latest?.flavours.length ? latest.flavours : (hit?.flavours ?? []));
  const noBuild = $derived(!!hit && !hit.forFlavour && !hit.flavourUnknown);
  const authors = $derived(info?.authors.length ? info.authors : (hit?.authors ?? (addon?.author ? [addon.author] : [])));
  const label = $derived(addon?.sourceLabel ?? hit?.sourceLabel ?? null);
  const website = $derived(info?.url ?? addon?.website ?? null);
  const busyKey = $derived(addon?.id ?? hit?.spec ?? "");
  const busy = $derived(store.busy[busyKey]);
  const back = $derived(store.view === "installed" ? "Installed" : store.view === "updates" ? "Updates" : store.view === "browse" ? "Browse" : "Back");

  async function install(anyway: boolean) {
    if (!hit) return;
    const ok = await store.install(hit.spec, null, anyway, hit.name);
    if (!ok && !anyway && store.toast?.text.includes("No release of")) {
      store.toast = null;
      store.dialog = { kind: "anyway", spec: hit.spec, name: hit.name };
    }
  }

  const facts = $derived(
    [
      addon ? ["Installed", addon.version || "–"] : null,
      latest ? ["Latest", latest.version] : null,
      latest?.publishedAt ? ["Released", shortDate(latest.publishedAt)] : hit?.updatedAt ? ["Updated", ago(hit.updatedAt)] : null,
      hit?.downloads ? ["Downloads", count(hit.downloads)] : null,
      builds.length ? ["Builds for", builds.join(", ")] : null,
      addon ? ["Folders", addon.folders.join(", ")] : null,
    ].filter((x): x is [string, string] => !!x),
  );
</script>

<div class="page">
  <button type="button" class="btn btn-quiet back" onclick={() => (store.detail = null)}>
    <Icon name="back" size={15} stroke={2.4} />{back}
  </button>

  <header class="head">
    {#if shots.length}
      <button type="button" class="art" aria-label="View screenshots of {target.name}" onclick={() => view(0)}>
        <Tile name={target.name} iconUrl={info?.iconUrl ?? target.iconUrl} size={64} />
      </button>
    {:else}
      <Tile name={target.name} iconUrl={info?.iconUrl ?? target.iconUrl} size={64} unknown={!target.source} />
    {/if}
    <div class="who">
      <span class="tl"><h1 class="h1">{info?.name ?? target.name}</h1>{#if noBuild}<NoBuild text="No {flavour} build listed. It may not load in {flavour}." size={17} />{/if}</span>
      <span class="by">
        {#if authors.length}<span class="dim"><Icon name="user" size={13} stroke={2.2} />{authors.join(", ")}</span>{/if}
        {#if label}<SourceTag label={label} />{/if}
        {#each hit?.alsoOn ?? [] as o (o)}<SourceTag label={o} muted />{/each}
      </span>
    </div>
    <div class="acts">
      {#if addon}
        {#if busy === "Updating" || busy === "Installing"}
          <ProgressButton label="Update" {busy} pct={store.progress[addon.id]} onclick={() => {}} />
        {:else if s?.kind === "available" && !addon.pinned}
          <button type="button" class="btn btn-primary" onclick={() => store.update(addon.id)}>Update to {s.release?.version}</button>
        {:else if addon.pinned}
          <span class="pin"><Icon name="pin" size={14} stroke={2.2} />Pinned</span>
        {:else}
          <span class="ok"><Icon name="check" size={14} stroke={2.6} />Installed</span>
        {/if}
        <RowMenu label="More actions for {addon.name}" items={addonMenu(addon)} />
      {:else if hit}
        {#if hit.installed}
          <span class="ok"><Icon name="check" size={14} stroke={2.6} />Installed</span>
        {:else if busy}
          <ProgressButton label="Install" busy={busy} pct={store.progress[hit.spec]} onclick={() => {}} />
        {:else if noBuild}
          <RowMenu label="More actions for {hit.name}" items={[{ label: "Install anyway", run: () => install(true) }]} />
        {:else}
          <button type="button" class="btn btn-primary" onclick={() => install(false)}><Icon name="download" size={15} stroke={2.4} />Install</button>
        {/if}
      {/if}
    </div>
  </header>

  {#if error}
    <div class="notice err"><Icon name="warn" size={16} /><span>{error}</span></div>
  {/if}

  {#if shots.length}
    <section class="block" aria-labelledby="shots-h">
      <h2 class="h2" id="shots-h">Screenshots <span class="dim n">{shots.length}</span></h2>
      <ul class="shots">
        {#each thumbs as url, i (i)}
          <li><button type="button" class="shot" aria-label="View screenshot {i + 1} of {shots.length}" onclick={() => view(i)}>
            <Image {url} alt="" />
          </button></li>
        {/each}
      </ul>
    </section>
  {:else if expectShots && !info && !error}
    <section class="block" aria-busy="true" aria-label="Loading screenshots">
      <h2 class="h2">Screenshots</h2>
      <ul class="shots">{#each { length: 3 } as _}<li><span class="shot skel"></span></li>{/each}</ul>
    </section>
  {/if}

  <div class="cols">
    <div class="main">
      {#if target.source && !info && !error && !hit?.summary}
        <section class="block" aria-busy="true" aria-label="Loading description">
          <h2 class="h2">About</h2>
          {#each ["92%", "86%", "60%"] as w}<span class="skel" style:width={w} style:height="12px"></span>{/each}
        </section>
      {:else if info?.about.length || hit?.summary || !target.source}
        <section class="block">
          <h2 class="h2">About</h2>
          {#if info?.about.length}
            {#each info.about as p}<p>{p}</p>{/each}
          {:else if hit?.summary}
            <p>{hit.summary}</p>
          {:else if !target.source}
            <p class="dim">Nexforge doesn't know where this addon comes from. Identify its source to get updates and details.</p>
          {/if}
        </section>
      {/if}

      {#if info?.releases.length}
        <section class="block">
          <h2 class="h2">Versions</h2>
          <div class="panel">
            {#each info.releases as r (r.version + r.publishedAt)}
              <div class="rel">
                <div class="rl">
                  <span class="rv">{r.version}</span>
                  {#if addon && addon.version === r.version}<span class="pill pill-ok">Installed</span>
                  {:else if addon?.kept.includes(r.version)}<span class="pill pill-off">Kept</span>{/if}
                  {#if r.channel !== "stable"}<span class="tag">{r.channel}</span>{/if}
                  <span class="dim small">{[r.publishedAt ? shortDate(r.publishedAt) : "", size(r.size)].filter(Boolean).join(" · ")}</span>
                  <span class="fl">{#each r.flavours as f (f)}<span class="tag" class:on={f === flavour}>{f}</span>{/each}</span>
                  {#if addon && addon.version !== r.version && addon.kept.includes(r.version)}
                    <RowMenu label="Actions for {r.version}" items={[{ label: `Roll back to ${r.version}`, run: () => store.rollback(addon.id, addon.name, r.version) }]} />
                  {/if}
                </div>
                {#if r.notes.length}<ul class="notes">{#each r.notes as n}<li>{n}</li>{/each}</ul>{/if}
              </div>
            {/each}
          </div>
        </section>
      {:else if target.source && !info && !error}
        <section class="block" aria-busy="true" aria-label="Loading versions">
          <h2 class="h2">Versions</h2>
          <div class="panel">
            {#each { length: 3 } as _}
              <div class="rel"><div class="rl"><span class="skel" style:width="64px" style:height="14px"></span><span class="skel" style:width="90px" style:height="11px"></span></div>
                <span class="skel" style:width="55%" style:height="11px"></span></div>
            {/each}
          </div>
        </section>
      {/if}
    </div>

    <aside class="side">
      {#if facts.length}
        <dl class="panel facts">
          {#each facts as [k, v] (k)}<div class="fact"><dt>{k}</dt><dd>{v}</dd></div>{/each}
        </dl>
      {/if}
      {#if website}
        <button type="button" class="btn" onclick={() => api.openUrl(website!).catch((e) => store.say("err", e.message))}>
          <Icon name="external" size={14} stroke={2.2} />Open website
        </button>
      {/if}
      {#if addon}
        <button type="button" class="btn" onclick={() => api.open("addon", store.current!, addon.id).catch((e) => store.say("err", e.message))}>
          <Icon name="folder" size={14} stroke={2.2} />Open folder
        </button>
      {/if}
      {#if addon && !addon.managed}<span class="dim small">{plural(addon.folders.length, "folder")}</span>{/if}
    </aside>
  </div>
</div>

<Gallery bind:open={galleryOpen} bind:index={galleryAt} images={shots} {thumbs} name={info?.name ?? target.name} />

<style>
  .back { align-self: flex-start; margin: -8px 0 -8px -10px; gap: 4px; }
  .head { display: flex; align-items: center; gap: 16px; }
  .who { display: flex; flex-direction: column; gap: 4px; min-width: 0; margin-right: auto; }
  .tl { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .who .h1 { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .by { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; font-size: 12px; }
  .by .dim { display: inline-flex; align-items: center; gap: 5px; }
  .acts { display: flex; align-items: center; gap: 8px; flex: 0 0 auto; }
  .shots { display: flex; gap: 10px; margin: 0; padding: 0 0 6px; list-style: none; overflow-x: auto; }
  .shot { display: block; flex: 0 0 auto; width: 224px; height: 126px; padding: 0; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--surface); overflow: hidden; cursor: zoom-in; }
  .shot:hover { border-color: var(--text-3); }
  .h2 .n { font-weight: 600; margin-left: 4px; }
  .art { display: flex; padding: 0; border: 0; background: none; border-radius: 8px; cursor: zoom-in; }
  .cols { display: grid; grid-template-columns: minmax(0, 1fr) 240px; gap: 20px; align-items: start; }
  .main { display: flex; flex-direction: column; gap: 20px; min-width: 0; }
  .block { display: flex; flex-direction: column; gap: 8px; }
  .block p { margin: 0; max-width: 72ch; line-height: 1.45; color: var(--text-soft); }
  .rel { display: flex; flex-direction: column; gap: 6px; padding: 10px 16px; border-bottom: 1px solid var(--divider); }
  .rel:last-child { border-bottom: 0; }
  .rl { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .rv { font-weight: 700; font-variant-numeric: tabular-nums; }
  .fl { display: flex; gap: 4px; margin-left: auto; }
  .tag.on { color: var(--text); background: var(--raised-hover); }
  .small { font-size: 11px; }
  .side { display: flex; flex-direction: column; gap: 8px; }
  .side .btn { justify-content: flex-start; }
  .facts { margin: 0; padding: 4px 0; }
  .fact { display: flex; flex-direction: column; gap: 0; padding: 6px 14px; }
  dt { font-size: 11px; color: var(--text-2); }
  dd { margin: 0; font-size: 13px; font-weight: 600; overflow-wrap: anywhere; }
</style>
