<script lang="ts" module>
  export interface Item { label: string; run: () => void; danger?: boolean; disabled?: boolean }
</script>

<script lang="ts">
  // A row's ⋯ menu (Bits UI DropdownMenu: keyboard, focus and screen-reader
  // behaviour; drawn in a layer above the page, flipped near the edges).
  import { DropdownMenu } from "bits-ui";
  import Icon from "./Icon.svelte";

  let { label, items }: { label: string; items: Item[] } = $props();
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger class="btn btn-icon" aria-label={label}><Icon name="more" /></DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content class="dd" align="end" sideOffset={4} collisionPadding={8}>
      {#each items as it (it.label)}
        <DropdownMenu.Item class={it.danger ? "dd-item danger" : "dd-item"} disabled={it.disabled} onSelect={it.run}>{it.label}</DropdownMenu.Item>
      {/each}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
