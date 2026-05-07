<script lang="ts">
  interface Props {
    title: string;
    message: string;
    confirmLabel?: string;
    danger?: boolean;
    onConfirm: () => void | Promise<void>;
    onCancel: () => void;
  }

  let {
    title,
    message,
    confirmLabel = "Confirm",
    danger = false,
    onConfirm,
    onCancel,
  }: Props = $props();

  let working = $state(false);

  async function confirm() {
    if (working) return;
    working = true;
    try {
      await onConfirm();
    } finally {
      working = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onCancel();
    } else if (e.key === "Enter") {
      e.stopPropagation();
      confirm();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="backdrop"
  role="button"
  tabindex="-1"
  aria-label="Cancel"
  onclick={onCancel}
  onkeydown={(e) => e.key === "Enter" && onCancel()}
></div>
<div class="modal" role="dialog" aria-modal="true">
  <h2>{title}</h2>
  <p>{message}</p>
  <div class="actions">
    <button class="cancel" onclick={onCancel} disabled={working}>
      Cancel
    </button>
    <button
      class={danger ? "danger" : "primary"}
      onclick={confirm}
      disabled={working}
    >
      {working ? "…" : confirmLabel}
    </button>
  </div>
</div>

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
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: var(--surface);
    border-radius: 8px;
    padding: 14px 16px;
    z-index: 11;
    width: 320px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
  }
  h2 {
    margin: 0 0 6px;
    font-size: 14px;
    font-weight: 600;
  }
  p {
    margin: 0 0 12px;
    font-size: 13px;
    color: var(--text-secondary);
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: var(--radius);
    cursor: pointer;
    font-family: inherit;
    border: 1px solid var(--border);
    background: var(--surface);
    color: inherit;
  }
  button.primary {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
  }
  button.danger {
    background: #ff3b30;
    color: white;
    border-color: var(--danger);
  }
  button.danger:hover:not(:disabled) {
    background: #d70015;
  }
  button.cancel:hover:not(:disabled) {
    background: var(--surface-2);
  }
  button:disabled {
    opacity: 0.6;
    cursor: default;
  }
  </style>
