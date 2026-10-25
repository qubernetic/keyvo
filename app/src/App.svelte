<script lang="ts">
  // The app shell: a heading and the daemon's status line. Home, the
  // mirror and the editor come in the next pre-alpha slices.
  import type { Daemon } from "./lib/daemon";

  let { daemon }: { daemon: Daemon } = $props();

  const status = $derived(daemon.status());

  function reason(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }
</script>

<main>
  <h1>keyvo</h1>
  {#await status}
    <p role="status">Connecting to the daemon</p>
  {:then status}
    <p role="status">
      {#if status.device.connected}
        Keypad connected
      {:else}
        No Keypad{status.device.error ? `: ${status.device.error}` : ""}
      {/if}
      <span class="view">
        profile {status.view.profile}, page {status.view.page}, {status.view.mode}
      </span>
    </p>
  {:catch error}
    <p role="alert" class="error">Daemon unreachable: {reason(error)}</p>
  {/await}
</main>

<style>
  main {
    padding: 16px;
  }

  h1 {
    margin: 0 0 12px;
    font-size: 1.4em;
  }

  p {
    margin: 0;
    padding: 8px 12px;
    background: var(--kv-bg-alt);
    border-radius: var(--kv-radius);
  }

  .view {
    color: var(--kv-fg-muted);
    margin-left: 8px;
  }

  .error {
    border-left: 4px solid var(--kv-state-error);
  }
</style>
