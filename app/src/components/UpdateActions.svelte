<script lang="ts">
  // "Check now" and "Update all", identical on Installed and Updates.
  import { mergeProps } from "bits-ui";
  import Icon from "./Icon.svelte";
  import Tip from "./Tip.svelte";
  import { ago } from "../lib/format";
  import { store } from "../lib/store.svelte";

  const updatable = $derived(store.updates.filter((a) => !a.pinned));
</script>

<Tip text={store.checkedAt ? `Last checked ${ago(store.checkedAt)}` : "Not checked yet"} side="bottom">
  {#snippet children({ props })}
    <button {...mergeProps(props, { onclick: () => store.check() })} type="button" class="btn" disabled={store.checking || !store.current}>
      <Icon name="refresh" size={15} stroke={2.2} />{store.checking ? "Checking…" : "Check now"}
    </button>
  {/snippet}
</Tip>
<button type="button" class="btn" class:btn-primary={updatable.length > 0} onclick={() => store.updateAll()} disabled={!updatable.length}>
  <Icon name="download" size={15} stroke={2.4} />{updatable.length ? `Update all ${updatable.length}` : "Update all"}
</button>
