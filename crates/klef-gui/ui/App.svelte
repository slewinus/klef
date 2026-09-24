<script lang="ts">
  import { onMount } from "svelte";
  import {
    copyKey,
    deleteKey,
    listKeys,
    previewDotenvImport,
    setShortcut,
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
  import { pickDotenvFile } from "./lib/dotenvPicker";
  import HeaderBar from "./lib/HeaderBar.svelte";
  import KeyRow from "./lib/KeyRow.svelte";
  import ProjectChips from "./lib/ProjectChips.svelte";
  import SearchBar from "./lib/SearchBar.svelte";
  import { loadSettings } from "./lib/settings";
  import Toast from "./lib/Toast.svelte";
  import UnlockVault from "./lib/UnlockVault.svelte";
  import { Vault } from "./lib/vault.svelte";

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
  let shortcutError = $state<string | null>(null);

  const vault = new Vault();

  function applyShortcut() {
    setShortcut(loadSettings().shortcut).then(
      () => (shortcutError = null),
      (e) => (shortcutError = String(e)),
    );
  }

  // Settings may have changed the vault or the shortcut.
  async function handleSettingsClosed() {
    showSettings = false;
    applyShortcut();
    if (await vault.sync()) await refresh();
    // Don't leave the previous vault's rows (and Enter-to-copy) live behind the lock.
    if (vault.locked) keys = [];
  }

  // Pipeline: sort by recency first so the order survives filtering.
  let visibleKeys = $derived(
    filterKeys(filterByProject(sortByLastUsed(keys), selectedProject), query),
  );

  // Auto-clear a stale project filter when its last key is deleted.
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
      await copyKey(key.name);
      const s = loadSettings().autoClearSeconds;
      const suffix = s > 0 ? ` — clipboard clears in ${s}s` : "";
      showToast(`${key.name} copied${suffix}`);
      const now = new Date().toISOString();
      keys = keys.map((k) =>
        k.name === key.name ? { ...k, last_used_at: now } : k,
      );
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
    // Refresh from disk for canonical KeyMeta (default env_var, sorted tags).
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
    if (vault.locked) return; // would import into the Keychain, not the vault
    try {
      dotenvPlan = await previewDotenvImport(path);
    } catch (e) {
      showToast(`import error: ${e}`);
    }
  }

  async function pickAndImportDotenv() {
    const path = await pickDotenvFile();
    if (path !== null) await handleDotenvDropped(path);
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

  // Refresh + refocus. Called on mount and on each popover-shown event.
  async function refresh() {
    if (vault.locked) {
      loading = false;
      return;
    }
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

  onMount(() => {
    refresh();
    applyShortcut();
    // onMount must return a sync teardown (Svelte 5); async setup runs in
    // a fire-and-forget IIFE with teardowns captured by closure.
    let tLife: (() => void) | undefined;
    let tDrop: (() => void) | undefined;
    void (async () => {
      tLife = await setupPopoverLifecycle(
        () => refresh(),
        () => anyModalOpen,
        () => pinned,
      );
      tDrop = await onDotenvDropped(handleDotenvDropped);
    })();
    return () => {
      tLife?.();
      tDrop?.();
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<header>
  <HeaderBar bind:pinned vaultName={vault.name} locked={vault.locked} onImport={pickAndImportDotenv}
    onSettings={() => (showSettings = true)} onAdd={() => (showAddModal = true)} />
  <SearchBar bind:this={searchBar} bind:value={query} />
  <ProjectChips
    {keys}
    selected={selectedProject}
    onSelect={(p) => (selectedProject = p)}
  />
</header>

<main>
  {#if shortcutError}<div class="warn">Shortcut unavailable: {shortcutError}. Pick another in Settings.</div>{/if}
  {#if vault.locked && vault.wanted}
    <UnlockVault path={vault.wanted} onUnlock={(p) => vault.unlock(p).then(refresh)}
      onUseKeychain={() => vault.forget().then(refresh)} />
  {:else if loading}
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
  onSettingsClose={handleSettingsClosed}
  onDeleteCancel={() => (pendingDelete = null)}
  onDeleteConfirm={handleDeleteConfirm}
  onDotenvClose={() => (dotenvPlan = null)}
  onDotenvDone={handleDotenvImported}
/>

<style>
  header { padding: 10px 12px 6px; background: var(--surface); border-bottom: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  main { padding: 6px; }
  .empty { padding: 32px 16px; color: var(--text-secondary); text-align: center; font-size: 12px; line-height: 1.5; }
  .err { color: var(--danger); padding: 16px; font-size: 12px; }
  .warn { color: var(--danger); background: var(--surface-2); padding: 6px 10px; margin: 0 0 6px; border-radius: var(--radius-sm); font-size: 11px; }
  code { background: var(--surface-2); padding: 1px 5px; border-radius: var(--radius-sm); font-family: var(--font-mono); font-size: 11px; }
  strong { color: var(--text); font-weight: 600; }
</style>
