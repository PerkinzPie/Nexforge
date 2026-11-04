<script lang="ts">
  // "No build for this client", as an attention icon with the words in a tooltip.
  import Icon from "./Icon.svelte";
  import Tip from "./Tip.svelte";

  let { text, size = 15 }: { text: string; size?: number } = $props();
</script>

<Tip {text}>
  {#snippet children({ props })}
    <span {...props} class="nb" role="img" aria-label={text}><Icon name="alert" {size} stroke={2.2} /></span>
  {/snippet}
</Tip>

<style>
  .nb { display: inline-flex; flex: 0 0 auto; color: var(--attention); cursor: help; }
</style>
