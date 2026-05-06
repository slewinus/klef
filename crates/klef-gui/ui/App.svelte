<script lang="ts">
  import { onMount } from "svelte";
  import { copyToClipboard, getKeyValue, listKeys } from "./lib/api";
  import type { KeyDto } from "./lib/types";
  import KeyRow from "./lib/KeyRow.svelte";
  import Toast from "./lib/Toast.svelte";

  let keys = $state<KeyDto[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function showToast(msg: string) {
    toast = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1600);
  }

  async function handleCopy(key: KeyDto) {
    try {
      const value = await getKeyValue(key.name);
      await copyToClipboard(value);
      showToast(`${key.name} copied`);
    } catch (e) {
      showToast(`error: ${e}`);
    }
  }

  onMount(async () => {
    try {
      keys = await listKeys();
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
  });
</script>

<header>klef</header>

<main>
  {#if loading}
    <div class="empty">Loading…</div>
  {:else if loadError}
    <div class="err">Failed to load keys: {loadError}</div>
  {:else if keys.length === 0}
    <div class="empty">
      No keys yet. Add some with the CLI: <code>klef add &lt;name&gt;</code>
    </div>
  {:else}
    {#each keys as key (key.name)}
      <KeyRow {key} onCopy={handleCopy} />
    {/each}
  {/if}
</main>

<Toast message={toast} />

<style>
  header {
    padding: 12px 16px;
    background: #fff;
    border-bottom: 1px solid #d2d2d7;
    font-weight: 600;
  }
  main {
    padding: 8px;
  }
  .empty {
    padding: 24px;
    color: #6e6e73;
    text-align: center;
  }
  .err {
    color: #ff3b30;
    padding: 16px;
    font-size: 12px;
  }
  code {
    background: #e5e5ea;
    padding: 1px 4px;
    border-radius: 3px;
  }
  @media (prefers-color-scheme: dark) {
    header {
      background: #2c2c2e;
      border-bottom-color: #3a3a3c;
    }
    code {
      background: #3a3a3c;
    }
  }
</style>
