<script lang="ts">
  // One source, one colour, always with its name beside the dot.
  import { sourceColour } from "../lib/format";
  let { source, label, small = false }: { source: string | null; label: string; small?: boolean } = $props();
</script>

<span class="src" style:font-size={small ? "12px" : null}><span class="dot" style:background={sourceColour(source)}></span>{label}</span>
