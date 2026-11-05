<script lang="ts">
  // The Nexforge mark: the coral anvil with Nexabyte's three hexagons
  // (assets/nexforge-icon.svg, cropped to the anvil so it reads at small sizes).
  let { width = 36 }: { width?: number } = $props();
</script>

<svg {width} height={Math.round((width * 215) / 437)} viewBox="32 148 437 215" aria-hidden="true">
  <path fill="#D74653" d="M36 152H465V176L423 219H387C351 219 326 242 326 275C326 292 337 306 351 321L385 359H111L145 320C160 302 172 288 172 273C172 242 146 225 109 218C75 212 47 191 36 152Z" />
  <g fill="#222938">
    <path d="M221 218H249L263 242L249 266H221L207 242Z" />
    <path d="M221 272H249L263 296L249 320H221L207 296Z" />
    <path d="M267 245H295L309 269L295 293H267L253 269Z" />
  </g>
</svg>
