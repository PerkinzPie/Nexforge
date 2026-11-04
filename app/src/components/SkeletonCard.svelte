<script lang="ts">
  // The shape of an AddonCard while Browse is loading.
</script>

<div class="sk" aria-hidden="true">
  <span class="skel art"></span>
  <span class="t"><span class="skel" style:width="55%"></span><span class="skel" style:width="85%"></span><span class="skel" style:width="35%"></span></span>
</div>

<style>
  .sk { display: flex; align-items: center; gap: 14px; padding: 12px; border: 1px solid var(--border); border-radius: var(--r-md); background: var(--surface); }
  .art { width: 64px; height: 64px; flex: 0 0 auto; border-radius: 8px; }
  .t { display: flex; flex-direction: column; gap: 7px; flex: 1; }
  .t .skel { height: 11px; }
</style>
