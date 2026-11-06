<script lang="ts">
  // The Nexforge mark: the two-tone puzzle anvil, outlined for dark
  // backgrounds, cropped to the anvil so it reads at small sizes. The artwork
  // is raster (assets/nexforge-icon-dark-1024.png), so each size used in the
  // app has a 1x/2x/3x export and the browser picks the sharpest.
  import a28 from "../assets/brand/anvil-28.png";
  import a36 from "../assets/brand/anvil-36.png";
  import a56 from "../assets/brand/anvil-56.png";
  import a64 from "../assets/brand/anvil-64.png";
  import a72 from "../assets/brand/anvil-72.png";
  import a84 from "../assets/brand/anvil-84.png";
  import a108 from "../assets/brand/anvil-108.png";
  import a128 from "../assets/brand/anvil-128.png";
  import a192 from "../assets/brand/anvil-192.png";

  let { width = 36 }: { width?: number } = $props();

  const srcset = [[a28, 28], [a36, 36], [a56, 56], [a64, 64], [a72, 72], [a84, 84], [a108, 108], [a128, 128], [a192, 192]]
    .map(([src, w]) => `${src} ${w}w`)
    .join(", ");
</script>

<img src={a64} {srcset} sizes="{width}px" {width} height={Math.round((width * 578) / 1000)} alt="" aria-hidden="true" draggable="false" />

<style>
  img { display: block; flex: 0 0 auto; user-select: none; }
</style>
