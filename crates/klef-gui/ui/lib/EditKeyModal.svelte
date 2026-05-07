<script lang="ts">
  import { untrack } from "svelte";
  import { editKey } from "./api";
  import type { KeyDto } from "./types";

  interface Props {
    target: KeyDto;
    onClose: () => void;
    onSaved: () => void;
  }

  let { target, onClose, onSaved }: Props = $props();

  // Pre-fill from the existing key. Value stays empty: the user types it
  // only if they want to change it. Empty value = preserve current secret.
  // `untrack` is intentional: the modal is mounted/unmounted per edit
  // (see {#if editTarget} in App.svelte), so we want a one-shot snapshot
  // of the prop, not a reactive binding to it.
  let value = $state("");
  let envVar = $state(untrack(() => target.env_var));
  let note = $state(untrack(() => target.note ?? ""));
  let tagsRaw = $state(untrack(() => (target.tags ?? []).join(", ")));
  let showValue = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  let envInput: HTMLInputElement;

  $effect(() => {
    setTimeout(() => envInput?.focus(), 0);
  });

  async function submit(e: Event) {
    e.preventDefault();
    if (submitting) return;
    error = null;
    submitting = true;
    try {
      const tags = tagsRaw
        .split(",")
        .map((t) => t.trim())
        .filter((t) => t.length > 0);
      await editKey({
        name: target.name,
        value: value.length > 0 ? value : undefined,
        envVar: envVar.trim() || undefined,
        note: note.trim() || undefined,
        tags,
      });
      onSaved();
    } catch (e) {
      error = String(e);
    } finally {
      submitting = false;
    }
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
  <h2>Edit “{target.name}”</h2>

  <label class="value-row">
    <span>New value</span>
    <div class="value-input">
      <input
        bind:value
        type={showValue ? "text" : "password"}
        placeholder="Leave empty to keep current"
        autocomplete="off"
        spellcheck="false"
      />
      <button
        type="button"
        class="reveal"
        onclick={() => (showValue = !showValue)}
        aria-label={showValue ? "Hide value" : "Show value"}
      >
        {showValue ? "Hide" : "Show"}
      </button>
    </div>
  </label>

  <label>
    <span>Env var</span>
    <input
      bind:this={envInput}
      bind:value={envVar}
      type="text"
      autocomplete="off"
      spellcheck="false"
    />
  </label>

  <label>
    <span>Note</span>
    <input bind:value={note} type="text" />
  </label>

  <label>
    <span>Tags</span>
    <input
      bind:value={tagsRaw}
      type="text"
      placeholder="billing, prod, project:my-app"
      autocomplete="off"
      spellcheck="false"
    />
    <small>
      Comma-separated. Use <code>project:&lt;name&gt;</code> to make it appear as a chip filter.
    </small>
  </label>

  {#if error}
    <div class="err">{error}</div>
  {/if}

  <div class="actions">
    <button type="button" class="cancel" onclick={onClose} disabled={submitting}>
      Cancel
    </button>
    <button type="submit" class="primary" disabled={submitting}>
      {submitting ? "Saving…" : "Save"}
    </button>
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
    gap: 8px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    overflow-y: auto;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 14px;
    font-weight: 600;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11px;
    color: var(--text-secondary);
  }
  input {
    padding: 5px 8px;
    font-size: 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: inherit;
    background: var(--surface);
    font-family: inherit;
    outline: none;
  }
  input:focus {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2);
  }
  .value-input {
    display: flex;
    gap: 4px;
  }
  .value-input input {
    flex: 1;
  }
  .reveal {
    padding: 4px 8px;
    font-size: 11px;
    background: transparent;
    color: #007aff;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    font-family: inherit;
  }
  .reveal:hover {
    background: var(--surface-2);
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 6px;
  }
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
  .actions button.primary {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  .actions button.primary:hover:not(:disabled) {
    background: var(--accent-hover);
  }
  .actions button.cancel:hover:not(:disabled) {
    background: var(--surface-2);
  }
  .actions button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .err {
    color: var(--danger);
    font-size: 12px;
    padding: 6px 8px;
    background: rgba(255, 59, 48, 0.08);
    border-radius: var(--radius-sm);
  }
  </style>
