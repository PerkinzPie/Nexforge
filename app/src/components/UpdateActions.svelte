<script lang="ts">
  // "Check now" and "Update all", identical on Installed and Updates.
  import Icon from "./Icon.svelte";
  import { ago } from "../lib/format";
  import { store } from "../lib/store.svelte";

  const updatable = $derived(store.updates.filter((a) => !a.pinned));
</script>

<button type="button" class="btn" onclick={() => store.check()} disabled={store.checking || !store.current}
  data-tip={store.checkedAt ? `Last checked ${ago(store.checkedAt)}` : "Not checked yet"}>
  <Icon name="refresh" size={15} stroke={2.2} />{store.checking ? "Checking…" : "Check now"}
</button>
<button type="button" class="btn" class:btn-primary={updatable.length > 0} onclick={() => store.updateAll()} disabled={!updatable.length}>
  <Icon name="download" size={15} stroke={2.4} />{updatable.length ? `Update all ${updatable.length}` : "Update all"}
</button>
