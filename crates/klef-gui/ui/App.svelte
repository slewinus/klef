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
  import { Pin, PinOff, Plus, Settings as SettingsIcon } from "lucide-svelte";
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
  let pinned = $state(false);

  // Pipeline: sort by recency, then project filter, then search query.
  // Sort runs first so the recency order is preserved through filtering.
  let visibleKeys = $derived(
    filterKeys(filterByProject(sortByLastUsed(keys), selectedProject), query),
  );

  // Auto-clear a stale project filter: if the user just deleted the last
  // key in a project, the chip would otherwise stay active and the list
  // would look empty for no obvious reason.
  $effect(() => {
    if (selectedProject === null) return;
    const tag = `project:${selectedProject}`;
    if (!keys.some((k) => k.tags?.includes(tag))) {
      selectedProject = null;
    }
  });

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
      () => pinned,
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
        class:active={pinned}
        onclick={() => (pinned = !pinned)}
        title={pinned ? "Unpin (auto-hide on)" : "Pin (keeps popover open)"}
      >
        {#if pinned}<PinOff size={14} />{:else}<Pin size={14} />{/if}
      </button>
      <button class="hdr-btn" onclick={() => (showSettings = true)} title="Settings">
        <SettingsIcon size={14} />
      </button>
      <button class="hdr-btn primary" onclick={() => (showAddModal = true)} title="Add key">
        <Plus size={14} />
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
      <KeyRow {key} selected={i === selectedIndex} onCopy={handleCopy}
        onEdit={(k) => (editTarget = k)} onDelete={(k) => (pendingDelete = k)} />
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
  header {
    padding: 10px 12px 6px; background: var(--surface);
    border-bottom: 1px solid var(--border);
    display: flex; flex-direction: column; gap: 8px;
  }
  .title-row { display: flex; align-items: center; justify-content: space-between; }
  .title { font-weight: 700; font-size: 13px; letter-spacing: -0.01em; }
  .header-actions { display: flex; gap: 2px; }
  .hdr-btn {
    width: 26px; height: 26px; padding: 0;
    background: transparent; color: var(--text-secondary);
    border: none; border-radius: var(--radius-sm);
    cursor: pointer; font-family: inherit;
    display: inline-flex; align-items: center; justify-content: center;
    transition: background 80ms, color 80ms;
  }
  .hdr-btn:hover { background: var(--hover); color: var(--text); }
  .hdr-btn.active { background: var(--accent-bg); color: var(--accent); }
  .hdr-btn.primary { background: var(--accent); color: #fff; }
  .hdr-btn.primary:hover { background: var(--accent-hover); }
  main { padding: 6px; }
  .empty { padding: 32px 16px; color: var(--text-secondary); text-align: center; font-size: 12px; line-height: 1.5; }
  .err { color: var(--danger); padding: 16px; font-size: 12px; }
  code { background: var(--surface-2); padding: 1px 5px; border-radius: var(--radius-sm); font-family: var(--font-mono); font-size: 11px; }
  strong { color: var(--text); font-weight: 600; }
</style>
