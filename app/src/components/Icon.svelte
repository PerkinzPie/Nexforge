<script lang="ts">
  // Line icons on a 24-unit grid.
  type Name =
    | "installed" | "updates" | "browse" | "settings" | "chevrons" | "refresh" | "download" | "more"
    | "check" | "pin" | "warn" | "search" | "link" | "home" | "globe" | "shield" | "x" | "folder";
  let { name, size = 16, stroke = 2, colour = "currentColor" }: { name: Name; size?: number; stroke?: number; colour?: string } = $props();
</script>

{#if name === "more"}
  <svg width={size} height={size} viewBox="0 0 24 24" fill={colour} aria-hidden="true"><circle cx="5" cy="12" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="19" cy="12" r="1.8" /></svg>
{:else}
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={colour} stroke-width={stroke} stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#if name === "installed"}<rect x="3" y="3" width="18" height="18" rx="4" /><path d="M3 9h18M9 21V9" />
    {:else if name === "updates" || name === "refresh"}<path d="M21 12a9 9 0 1 1-2.6-6.4" /><path d="M21 3v6h-6" />
    {:else if name === "browse" || name === "search"}<circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" />
    {:else if name === "settings"}<path d="M4 6h16M4 12h16M4 18h16" /><circle cx="9" cy="6" r="2" fill="var(--nx-indigo)" /><circle cx="15" cy="12" r="2" fill="var(--nx-indigo)" /><circle cx="8" cy="18" r="2" fill="var(--nx-indigo)" />
    {:else if name === "chevrons"}<path d="m7 10 5-5 5 5M7 14l5 5 5-5" />
    {:else if name === "download"}<path d="M12 4v12M6 10l6 6 6-6M4 20h16" />
    {:else if name === "check"}<path d="m5 12 5 5L20 7" />
    {:else if name === "pin"}<path d="M12 17v5M9 3h6l-1 7 3 2v2H7v-2l3-2-1-7z" />
    {:else if name === "warn"}<path d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" />
    {:else if name === "link"}<path d="M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1" /><path d="M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1" />
    {:else if name === "home"}<path d="M4 20h16M6 20V9l6-5 6 5v11M10 20v-6h4v6" />
    {:else if name === "globe"}<circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
    {:else if name === "shield"}<path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3z" /><path d="m9 12 2 2 4-4" />
    {:else if name === "x"}<path d="M6 6l12 12M18 6 6 18" />
    {:else if name === "folder"}<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    {/if}
  </svg>
{/if}
