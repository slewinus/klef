<script lang="ts">
  import { addKey } from "./api";

  interface Props {
    onClose: () => void;
    onAdded: () => void;
  }

  let { onClose, onAdded }: Props = $props();

  let name = $state("");
  let value = $state("");
  let envVar = $state("");
  let note = $state("");
  let tagsRaw = $state("");
  let showValue = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  let nameInput: HTMLInputElement;

  function focusName() {
    nameInput?.focus();
  }

  // Auto-focus the name field when the modal opens.
  $effect(() => {
    setTimeout(focusName, 0);
  });

  async function submit(e: Event) {
    e.preventDefault();
    if (submitting) return;
    error = null;
    if (name.trim() === "") {
      error = "Name is required";
      return;
    }
    if (value.trim() === "") {
      error = "Value is required";
      return;
    }
    submitting = true;
    try {
      const tags = tagsRaw
        .split(",")
        .map((t) => t.trim())
        .filter((t) => t.length > 0);
      await addKey({
        name: name.trim(),
        value,
        envVar: envVar.trim() || undefined,
        note: note.trim() || undefined,
        tags,
      });
      onAdded();
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
  <h2>Add Key</h2>

  <label>
    <span>Name</span>
    <input
      bind:this={nameInput}
      bind:value={name}
      type="text"
      placeholder="stripe-prod"
      autocomplete="off"
      spellcheck="false"
    />
  </label>

  <label class="value-row">
    <span>Value</span>
    <div class="value-input">
      <input
        bind:value
        type={showValue ? "text" : "password"}
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
      bind:value={envVar}
      type="text"
      placeholder="STRIPE_PROD_API_KEY (auto if blank)"
      autocomplete="off"
      spellcheck="false"
    />
  </label>

  <label>
    <span>Note</span>
    <input
      bind:value={note}
      type="text"
      placeholder="Optional"
    />
  </label>

  <label>
    <span>Tags</span>
    <input
      bind:value={tagsRaw}
      type="text"
      placeholder="comma-separated, e.g. billing, prod, project:dahouse"
      autocomplete="off"
      spellcheck="false"
    />
  </label>

  {#if error}
    <div class="err">{error}</div>
  {/if}

  <div class="actions">
    <button type="button" class="cancel" onclick={onClose} disabled={submitting}>
      Cancel
    </button>
    <button type="submit" class="primary" disabled={submitting}>
      {submitting ? "Adding…" : "Add"}
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
