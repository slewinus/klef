<script lang="ts">
  import { onMount } from "svelte";
  import {
    copyToClipboard,
    deleteKey,
    getKeyValue,
    listKeys,
    previewDotenvImport,
    recordAccess,
    type DotenvPlan,
  } from "./lib/api";
  import { filterByProject, filterKeys, sortByLastUsed } from "./lib/filter";
  import { isNavKey, nextIndexFor } from "./lib/keyboardNav";
  import {
    hideCurrentPopover,
    isKeychainDenied,
    onDotenvDropped,
    setupPopoverLifecycle,
  } from "./lib/popoverLifecycle";
  import type { KeyDto } from "./lib/types";
  import KeychainAccessHelp from "./lib/KeychainAccessHelp.svelte";
  import Modals from "./lib/Modals.svelte";
  import KeyRow from "./lib/KeyRow.svelte";
  import ProjectChips from "./lib/ProjectChips.svelte";
  import SearchBar from "./lib/SearchBar.svelte";
  import { loadSettings } from "./lib/settings";
  import Toast from "./lib/Toast.svelte";

  let keys = $state<KeyDto[]>([]);
  let loading = $state(true);
  let loadError = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  let query = $state("");
  let selectedProject = $state<string | null>(null);
  let selectedIndex = $state(0);
  let searchBar: SearchBar | undefined = $state();

  let showAddModal = $state(false);
  let showSettings = $state(false);
  let editTarget = $state<KeyDto | null>(null);
  let pendingDelete = $state<KeyDto | null>(null);
  let dotenvPlan = $state<DotenvPlan | null>(null);

  // Pipeline: sort by recency, then project filter, then search query.
  // Sort runs first so the recency order is preserved through filtering.
  let visibleKeys = $derived(
    filterKeys(filterByProject(sortByLastUsed(keys), selectedProject), query),
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
      const s = loadSettings().autoClearSeconds;
      const suffix = s > 0 ? ` — clipboard clears in ${s}s` : "";
      showToast(`${key.name} copied${suffix}`);
      // Optimistic update so the row jumps to top immediately.
      const now = new Date().toISOString();
      keys = keys.map((k) =>
        k.name === key.name ? { ...k, last_used_at: now } : k,
      );
      recordAccess(key.name).catch((e) => console.warn("record_access", e));
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

  // Skip our key handlers when a modal is open — modals own Escape etc.
  let anyModalOpen = $derived(
    showAddModal ||
      showSettings ||
      pendingDelete !== null ||
      editTarget !== null ||
      dotenvPlan !== null,
  );

  async function handleDotenvDropped(path: string) {
    try {
      dotenvPlan = await previewDotenvImport(path);
    } catch (e) {
      showToast(`import error: ${e}`);
    }
  }

  async function handleDotenvImported(count: number) {
    dotenvPlan = null;
    showToast(`${count} keys imported`);
    keys = await listKeys();
  }

  // Reset selection when the visible list changes (search/project filter).
  $effect(() => {
    visibleKeys;
    selectedIndex = 0;
  });

  function handleKeydown(e: KeyboardEvent) {
    if (anyModalOpen) return;
    if (e.key === "Escape") {
      if (query) {
        query = "";
      } else {
        hideCurrentPopover();
      }
      return;
    }
    if (isNavKey(e.key)) {
      e.preventDefault();
      selectedIndex = nextIndexFor(e.key, selectedIndex, visibleKeys.length);
      return;
    }
    // Enter copies the selected row.
    if (e.key === "Enter" && visibleKeys.length > 0) {
      const target = visibleKeys[selectedIndex] ?? visibleKeys[0];
      handleCopy(target);
    }
  }

  // Refresh keys + refocus the search bar. Called on mount and on each
  // popover-shown event from Rust (see lib/popoverLifecycle).
  async function refresh() {
    try {
      keys = await listKeys();
      loadError = null;
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
    setTimeout(() => searchBar?.focus(), 0);
  }

  onMount(async () => {
    refresh();
    const teardownLife = await setupPopoverLifecycle(
      () => refresh(),
      () => anyModalOpen,
    );
    const teardownDrop = await onDotenvDropped(handleDotenvDropped);
    return () => {
      teardownLife();
      teardownDrop();
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<header>
  <div class="title-row">
    <div class="title">klef</div>
    <div class="header-actions">
      <button
        class="hdr-btn"
        onclick={() => (showSettings = true)}
        aria-label="Settings"
        title="Settings"
      >
        ⚙
      </button>
      <button
        class="hdr-btn primary"
        onclick={() => (showAddModal = true)}
        aria-label="Add key"
        title="Add key"
      >
        +
      </button>
    </div>
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
  {:else if loadError && isKeychainDenied(loadError)}
    <KeychainAccessHelp onRetry={refresh} />
  {:else if loadError}
    <div class="err">Failed to load keys: {loadError}</div>
  {:else if keys.length === 0}
    <div class="empty">
      No keys yet. Add some with the CLI: <code>klef add &lt;name&gt;</code>
    </div>
  {:else if visibleKeys.length === 0}
    <div class="empty">
      No keys match
      {#if query}<strong>“{query}”</strong>{/if}
      {#if query && selectedProject}in{/if}
      {#if selectedProject}project <strong>{selectedProject}</strong>{/if}
    </div>
  {:else}
    {#each visibleKeys as key, i (key.name)}
      <KeyRow
        {key}
        selected={i === selectedIndex}
        onCopy={handleCopy}
        onEdit={(k) => (editTarget = k)}
        onDelete={(k) => (pendingDelete = k)}
      />
    {/each}
  {/if}
</main>

<Toast message={toast} />

<Modals
  {showAddModal}
  {showSettings}
  {editTarget}
  {pendingDelete}
  {dotenvPlan}
  onAddClose={() => (showAddModal = false)}
  onAddDone={handleAdded}
  onEditClose={() => (editTarget = null)}
  onEditDone={handleSaved}
  onSettingsClose={() => (showSettings = false)}
  onDeleteCancel={() => (pendingDelete = null)}
  onDeleteConfirm={handleDeleteConfirm}
  onDotenvClose={() => (dotenvPlan = null)}
  onDotenvDone={handleDotenvImported}
/>

<style>
  header { padding: 10px 12px 8px; background: #fff; border-bottom: 1px solid #d2d2d7; display: flex; flex-direction: column; gap: 6px; }
  .title-row { display: flex; align-items: center; justify-content: space-between; }
  .title { font-weight: 600; font-size: 13px; }
  .header-actions { display: flex; gap: 4px; }
  .hdr-btn {
    width: 22px; height: 22px; padding: 0;
    background: transparent; color: #6e6e73;
    border: 1px solid transparent; border-radius: 4px;
    cursor: pointer; font-size: 14px; line-height: 1; font-family: inherit;
  }
  .hdr-btn:hover { background: #f5f5f7; color: #1d1d1f; }
  .hdr-btn.primary { background: #007aff; color: white; font-size: 16px; border-color: #007aff; }
  .hdr-btn.primary:hover { background: #0051d5; color: white; }
  @media (prefers-color-scheme: dark) {
    .hdr-btn { color: #98989d; }
    .hdr-btn:hover { background: #3a3a3c; color: #f5f5f7; }
    .hdr-btn.primary { background: #0a84ff; border-color: #0a84ff; }
    .hdr-btn.primary:hover { background: #0066cc; }
  }
  main { padding: 8px; }
  .empty { padding: 24px; color: #6e6e73; text-align: center; }
  .err { color: #ff3b30; padding: 16px; font-size: 12px; }
  code { background: #e5e5ea; padding: 1px 4px; border-radius: 3px; }
  strong { color: #1d1d1f; }
  @media (prefers-color-scheme: dark) {
    header { background: #2c2c2e; border-bottom-color: #3a3a3c; }
    code { background: #3a3a3c; }
    strong { color: #f5f5f7; }
  }
</style>
