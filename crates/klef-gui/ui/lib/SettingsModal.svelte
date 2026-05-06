<script lang="ts">
  import { untrack } from "svelte";
  import { DEFAULT_SETTINGS, loadSettings, saveSettings } from "./settings";

  interface Props {
    onClose: () => void;
  }

  let { onClose }: Props = $props();

  // Snapshot at mount; the form binds to local state until save.
  let initial = untrack(() => loadSettings());
  let autoClearSeconds = $state(initial.autoClearSeconds);

  function submit(e: Event) {
    e.preventDefault();
    saveSettings({ autoClearSeconds });
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

  <div class="actions">
    <button type="button" class="cancel" onclick={reset}>Reset to default</button>
    <div class="spacer"></div>
    <button type="button" class="cancel" onclick={onClose}>Cancel</button>
    <button type="submit" class="primary">Save</button>
  </div>
</form>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 10;
    border: none;
    cursor: pointer;
  }
  .modal {
    position: fixed;
    inset: 12px;
    background: #fff;
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
  label { display: flex; flex-direction: column; gap: 3px; font-size: 11px; color: #6e6e73; }
  small { color: #98989d; font-size: 10px; margin-top: 2px; }
  input {
    padding: 5px 8px;
    font-size: 13px;
    border: 1px solid #d2d2d7;
    border-radius: 5px;
    background: #fff;
    color: inherit;
    font-family: inherit;
    outline: none;
  }
  input:focus { border-color: #007aff; box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2); }
  .actions { display: flex; gap: 8px; align-items: center; margin-top: 6px; }
  .spacer { flex: 1; }
  .actions button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: 5px;
    cursor: pointer;
    font-family: inherit;
    border: 1px solid #d2d2d7;
    background: #fff;
    color: inherit;
  }
  .actions button.primary { background: #007aff; color: white; border-color: #007aff; }
  .actions button.primary:hover { background: #0051d5; }
  .actions button.cancel:hover { background: #f5f5f7; }
  @media (prefers-color-scheme: dark) {
    .modal { background: #2c2c2e; }
    input { background: #1d1d1f; border-color: #3a3a3c; color: #f5f5f7; }
    .actions button { background: #3a3a3c; border-color: #3a3a3c; color: #f5f5f7; }
    .actions button:hover { background: #48484a; }
    .actions button.primary { background: #0a84ff; border-color: #0a84ff; color: white; }
  }
</style>
