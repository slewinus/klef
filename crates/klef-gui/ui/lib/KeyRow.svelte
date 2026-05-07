<script lang="ts">
  import type { KeyDto } from "./types";
  import { Copy, Pencil, Trash2 } from "lucide-svelte";

  interface Props {
    key: KeyDto;
    selected?: boolean;
    onCopy: (key: KeyDto) => void | Promise<void>;
    onEdit: (key: KeyDto) => void;
    onDelete: (key: KeyDto) => void;
  }

  let { key, selected = false, onCopy, onEdit, onDelete }: Props = $props();
  let copying = $state(false);
  let rowEl: HTMLDivElement;

  $effect(() => {
    if (selected) {
      rowEl?.scrollIntoView({ block: "nearest" });
    }
  });

  async function handleCopy() {
    copying = true;
    try {
      await onCopy(key);
    } finally {
      copying = false;
    }
  }
</script>

<div class="row" class:selected bind:this={rowEl}>
  <div class="info">
    <div class="name">{key.name}</div>
    <div class="meta">
      <span class="env">{key.env_var}</span>
      {#if key.tags && key.tags.length}
        {#each key.tags as tag (tag)}
          <span class="tag">{tag}</span>
        {/each}
      {/if}
    </div>
  </div>
  <div class="actions">
    <button class="action copy" onclick={handleCopy} disabled={copying} aria-label="Copy {key.name}" title="Copy">
      {#if copying}
        <span class="dots">…</span>
      {:else}
        <Copy size={13} />
      {/if}
    </button>
    <button class="action" onclick={() => onEdit(key)} aria-label="Edit {key.name}" title="Edit">
      <Pencil size={13} />
    </button>
    <button class="action danger" onclick={() => onDelete(key)} aria-label="Delete {key.name}" title="Delete">
      <Trash2 size={13} />
    </button>
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: var(--radius);
    cursor: default;
    transition: background 80ms;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--accent-bg);
  }
  .info { min-width: 0; }
  .name {
    font-weight: 600;
    font-size: 13px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    overflow: hidden;
  }
  .env {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .tag {
    font-size: 10px;
    padding: 1px 6px;
    background: var(--surface-2);
    color: var(--text-secondary);
    border-radius: 999px;
    line-height: 1.4;
    flex-shrink: 0;
  }
  .actions {
    display: flex;
    gap: 1px;
    opacity: 0;
    transition: opacity 80ms;
  }
  .row:hover .actions,
  .row.selected .actions {
    opacity: 1;
  }
  .action {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    color: var(--text-secondary);
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-family: inherit;
    transition: background 80ms, color 80ms;
  }
  .action:hover {
    background: var(--hover-strong);
    color: var(--text);
  }
  .action.copy:hover {
    background: var(--accent-bg);
    color: var(--accent);
  }
  .action.danger:hover {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .action:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .dots {
    font-size: 14px;
    line-height: 1;
  }
</style>
