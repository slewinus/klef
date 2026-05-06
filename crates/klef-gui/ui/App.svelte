<script lang="ts">
  import { onMount } from "svelte";
  import {
    copyToClipboard,
    deleteKey,
    getKeyValue,
    listKeys,
  } from "./lib/api";
  import { filterByProject, filterKeys } from "./lib/filter";
  import type { KeyDto } from "./lib/types";
  import AddKeyModal from "./lib/AddKeyModal.svelte";
  import ConfirmDialog from "./lib/ConfirmDialog.svelte";
  import EditKeyModal from "./lib/EditKeyModal.svelte";
  import KeyRow from "./lib/KeyRow.svelte";
  import ProjectChips from "./lib/ProjectChips.svelte";
  import SearchBar from "./lib/SearchBar.svelte";
  import Toast from "./lib/Toast.svelte";

  let keys = $state<KeyDto[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  let query = $state("");
  let selectedProject = $state<string | null>(null);
  let searchBar: SearchBar | undefined = $state();

  let showAddModal = $state(false);
  let editTarget = $state<KeyDto | null>(null);
  let pendingDelete = $state<KeyDto | null>(null);

  // Filter pipeline: project first (narrows the candidate set), then search
  // query. Order doesn't change correctness but project-first is cheaper
  // when a project is selected.
  let visibleKeys = $derived(
    filterKeys(filterByProject(keys, selectedProject), query),
  );

  function showToast(msg: string) {
    toast = msg;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 1600);
  }

  async function handleCopy(key: KeyDto) {
    try {
      const value = await getKeyValue(key.name);
      await copyToClipboard(value);
      showToast(`${key.name} copied — clipboard clears in 30s`);
    } catch (e) {
      showToast(`error: ${e}`);
    }
  }

  async function handleDeleteConfirm() {
    if (!pendingDelete) return;
    const name = pendingDelete.name;
    try {
      await deleteKey(name);
      keys = keys.filter((k) => k.name !== name);
      showToast(`${name} deleted`);
    } catch (e) {
      showToast(`error: ${e}`);
    } finally {
      pendingDelete = null;
    }
  }

  async function handleAdded() {
    showAddModal = false;
    showToast("key added");
    // Refresh from disk so we pick up the canonical KeyMeta (default
    // env_var, sorted tags) rather than reconstructing it client-side.
    keys = await listKeys();
  }

  async function handleSaved() {
    const name = editTarget?.name ?? "key";
    editTarget = null;
    showToast(`${name} updated`);
    keys = await listKeys();
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

  // Refresh keys + refocus the search bar. Called once on mount and again
  // every time the popover is opened via the tray icon or ⌘⇧K. The Rust
  // side emits `popover-shown` from `toggle_window`; using that explicit
  // event sidesteps the unreliable DOM `focus` event which doesn't fire
  // on Tauri webview show/hide.
  async function refresh() {
    try {
      keys = await listKeys();
      loadError = null;
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
    // Tiny delay so the SearchBar mounts before we try to focus it on the
    // first call. Subsequent calls fire while the input already exists.
    setTimeout(() => searchBar?.focus(), 0);
  }

  onMount(async () => {
    refresh();
    const { listen } = await import("@tauri-apps/api/event");
    const unlisten = await listen("popover-shown", () => refresh());
    return () => unlisten();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<header>
  <div class="title-row">
    <div class="title">klef</div>
    <button
      class="add-btn"
      onclick={() => (showAddModal = true)}
      aria-label="Add key"
      title="Add key"
    >
      +
    </button>
  </div>
  <SearchBar bind:this={searchBar} bind:value={query} />
  <ProjectChips
    {keys}
    selected={selectedProject}
    onSelect={(p) => (selectedProject = p)}
  />
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
      No keys match
      {#if query && selectedProject}
        <strong>“{query}”</strong> in project
        <strong>{selectedProject}</strong>
      {:else if query}
        <strong>“{query}”</strong>
      {:else if selectedProject}
        project <strong>{selectedProject}</strong>
      {/if}
    </div>
  {:else}
    {#each visibleKeys as key (key.name)}
      <KeyRow
        {key}
        onCopy={handleCopy}
        onEdit={(k) => (editTarget = k)}
        onDelete={(k) => (pendingDelete = k)}
      />
    {/each}
  {/if}
</main>

<Toast message={toast} />

{#if showAddModal}
  <AddKeyModal
    onClose={() => (showAddModal = false)}
    onAdded={handleAdded}
  />
{/if}

{#if editTarget}
  <EditKeyModal
    target={editTarget}
    onClose={() => (editTarget = null)}
    onSaved={handleSaved}
  />
{/if}

{#if pendingDelete}
  <ConfirmDialog
    title="Delete key"
    message="Permanently delete “{pendingDelete.name}”? This removes the value from the Keychain and the index entry."
    confirmLabel="Delete"
    danger
    onConfirm={handleDeleteConfirm}
    onCancel={() => (pendingDelete = null)}
  />
{/if}

<style>
  header {
    padding: 10px 12px 8px;
    background: #fff;
    border-bottom: 1px solid #d2d2d7;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .title {
    font-weight: 600;
    font-size: 13px;
  }
  .add-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    background: #007aff;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    font-family: inherit;
  }
  .add-btn:hover {
    background: #0051d5;
  }
  @media (prefers-color-scheme: dark) {
    .add-btn {
      background: #0a84ff;
    }
    .add-btn:hover {
      background: #0066cc;
    }
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
