<script lang="ts">
  // A tooltip on any element: hover or keyboard focus shows it, drawn in a
  // layer above the page (never clipped) and flipped to stay on screen.
  // Usage: <Tip text="…">{#snippet children({ props })}<button {...props}>…</button>{/snippet}</Tip>
  // Merge your own handlers with `mergeProps(props, { onclick })`.
  import { Tooltip } from "bits-ui";
  import type { Snippet } from "svelte";

  type Side = "top" | "right" | "bottom" | "left";
  let { text, side = "top", children }: {
    text: string | null | undefined;
    side?: Side;
    children: Snippet<[{ props: Record<string, unknown> }]>;
  } = $props();
</script>

{#if text}
  <Tooltip.Root>
    <Tooltip.Trigger>
      {#snippet child({ props })}{@render children({ props })}{/snippet}
    </Tooltip.Trigger>
    <Tooltip.Portal>
      <Tooltip.Content {side} sideOffset={6} collisionPadding={8} class="tip">{text}</Tooltip.Content>
    </Tooltip.Portal>
  </Tooltip.Root>
{:else}
  {@render children({ props: {} })}
{/if}
