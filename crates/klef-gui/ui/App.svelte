<script lang="ts">
  import { onMount } from "svelte";
  import { copyToClipboard, getKeyValue, listKeys } from "./lib/api";
  import { filterKeys } from "./lib/filter";
  import type { KeyDto } from "./lib/types";
  import KeyRow from "./lib/KeyRow.svelte";
  import SearchBar from "./lib/SearchBar.svelte";
  import Toast from "./lib/Toast.svelte";

  let keys = $state<KeyDto[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  let query = $state("");
  let searchBar: SearchBar | undefined = $state();

  let visibleKeys = $derived(filterKeys(keys, query));

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

  function handleKeydown(e: KeyboardEvent) {
    // Esc clears the query (or hides the popover when query is empty —
    // the OS handles popover dismiss via blur, so we just clear).
    if (e.key === "Escape") {
      query = "";
    }
    // Enter on a single visible result triggers copy.
    if (e.key === "Enter" && visibleKeys.length === 1) {
      handleCopy(visibleKeys[0]);
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
    // Auto-focus the search bar so the user can type immediately on popover
    // open. Tiny delay so the input element exists.
    setTimeout(() => searchBar?.focus(), 0);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<header>
  <div class="title">klef</div>
  <SearchBar bind:this={searchBar} bind:value={query} />
</header>

<main>
  {#if loading}
    <div class="empty">Loading…</div>
  {:else if loadError}
    <div class="err">Failed to load keys: {loadError}</div>
  {:else if keys.length === 0}
    <div class="empty">
      No keys yet. Add some with the CLI: <code>klef add &lt;name&gt;</code>
    </div>
  {:else if visibleKeys.length === 0}
    <div class="empty">
      No keys match <strong>“{query}”</strong>
    </div>
  {:else}
    {#each visibleKeys as key (key.name)}
      <KeyRow {key} onCopy={handleCopy} />
    {/each}
  {/if}
</main>

<Toast message={toast} />

<style>
  header {
    padding: 10px 12px 8px;
    background: #fff;
    border-bottom: 1px solid #d2d2d7;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .title {
    font-weight: 600;
    font-size: 13px;
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
  strong {
    color: #1d1d1f;
  }
  @media (prefers-color-scheme: dark) {
    header {
      background: #2c2c2e;
      border-bottom-color: #3a3a3c;
    }
    code {
      background: #3a3a3c;
    }
    strong {
      color: #f5f5f7;
    }
  }
</style>
