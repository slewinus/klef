<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    DEFAULT_SETTINGS,
    isAutostartEnabled,
    loadSettings,
    saveSettings,
    setAutostart,
  } from "./settings";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  // Snapshot at mount; the form binds to local state until save.
  let initial = untrack(() => loadSettings());
  let autoClearSeconds = $state(initial.autoClearSeconds);
  let autostart = $state(false);
  let autostartLoading = $state(true);

  onMount(async () => {
    try {
      autostart = await isAutostartEnabled();
    } catch {
      autostart = false;
    } finally {
      autostartLoading = false;
    }
  });

  async function submit(e: Event) {
    e.preventDefault();
    saveSettings({ autoClearSeconds });
    try {
      await setAutostart(autostart);
    } catch (err) {
      console.warn("autostart toggle failed", err);
    }
    onClose();
  }

  function reset() {
    autoClearSeconds = DEFAULT_SETTINGS.autoClearSeconds;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="backdrop"
  role="button"
  tabindex="-1"
  aria-label="Close dialog"
  onclick={onClose}
  onkeydown={(e) => e.key === "Enter" && onClose()}
></div>
<form class="modal" onsubmit={submit}>
  <h2>Settings</h2>

  <label>
    <span>Auto-clear clipboard after (seconds)</span>
    <input
      bind:value={autoClearSeconds}
      type="number"
      min="0"
      max="600"
      step="1"
    />
    <small>
      0 disables auto-clear. Range 0–600. Default {DEFAULT_SETTINGS.autoClearSeconds}.
    </small>
  </label>

  <label class="checkbox">
    <input
      type="checkbox"
      bind:checked={autostart}
      disabled={autostartLoading}
    />
    <span>Open at login</span>
  </label>

  <div class="actions">
    <button type="button" class="cancel" onclick={reset}>Reset to default</button>
    <div class="spacer"></div>
    <button type="button" class="cancel" onclick={onClose}>Cancel</button>
    <button type="submit" class="primary">Save</button>
  </div>
</form>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 10;
    border: none;
    cursor: pointer;
  }
  .modal {
    position: absolute;
    inset: 12px;
    background: var(--surface);
    border-radius: 8px;
    padding: 14px;
    z-index: 11;
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    overflow-y: auto;
  }
  h2 { margin: 0 0 4px; font-size: 14px; font-weight: 600; }
  label { display: flex; flex-direction: column; gap: 3px; font-size: 11px; color: var(--text-secondary); }
  label.checkbox { flex-direction: row; align-items: center; gap: 6px; font-size: 12px; color: inherit; }
  label.checkbox input { width: auto; padding: 0; }
  small { color: var(--text-tertiary); font-size: 10px; margin-top: 2px; }
  input {
    padding: 5px 8px;
    font-size: 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: inherit;
    font-family: inherit;
    outline: none;
  }
  input:focus { border-color: var(--accent); box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2); }
  .actions { display: flex; gap: 8px; align-items: center; margin-top: 6px; }
  .spacer { flex: 1; }
  .actions button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: var(--radius);
    cursor: pointer;
    font-family: inherit;
    border: 1px solid var(--border);
    background: var(--surface);
    color: inherit;
  }
  .actions button.primary { background: var(--accent); color: white; border-color: var(--accent); }
  .actions button.primary:hover { background: var(--accent-hover); }
  .actions button.cancel:hover { background: var(--surface-2); }
  </style>
