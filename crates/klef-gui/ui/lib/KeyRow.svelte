<script lang="ts">
  import type { KeyDto } from "./types";

  interface Props {
    key: KeyDto;
    selected?: boolean;
    onCopy: (key: KeyDto) => void | Promise<void>;
    onEdit: (key: KeyDto) => void;
    onDelete: (key: KeyDto) => void;
    onSelect?: (key: KeyDto) => void;
  }

  let { key, selected = false, onCopy, onEdit, onDelete, onSelect }: Props = $props();
  let copying = $state(false);
  let rowEl: HTMLDivElement;

  // When the parent flips `selected` to true, scroll the row into view so
  // arrow-key navigation past the visible window still tracks visually.
  $effect(() => {
    if (selected) {
      rowEl?.scrollIntoView({ block: "nearest" });
    }
  });

  async function handleClick() {
    copying = true;
    try {
      await onCopy(key);
    } finally {
      copying = false;
    }
  }
</script>

<div class="row" class:selected bind:this={rowEl}>
  <div>
    <div class="name">{key.name}</div>
    <div class="meta">
      <span>{key.env_var}</span>
      {#if key.tags && key.tags.length}
        <span class="sep">·</span>
        {#each key.tags as tag (tag)}
          <span class="tag">{tag}</span>
        {/each}
      {/if}
    </div>
  </div>
  <div class="row-actions">
    <button class="copy" onclick={handleClick} disabled={copying}>
      {copying ? "…" : "Copy"}
    </button>
    <button
      class="icon-btn edit"
      onclick={() => onEdit(key)}
      aria-label="Edit {key.name}"
      title="Edit"
    >
      ✎
    </button>
    <button
      class="icon-btn delete"
      onclick={() => onDelete(key)}
      aria-label="Delete {key.name}"
      title="Delete"
    >
      ×
    </button>
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: #fff;
    border: 1px solid #d2d2d7;
    border-radius: 6px;
    margin-bottom: 6px;
  }
  .row.selected {
    border-color: #007aff;
    box-shadow: 0 0 0 2px rgba(0, 122, 255, 0.2);
  }
  .name {
    font-weight: 600;
  }
  .meta {
    color: #6e6e73;
    font-size: 12px;
    margin-top: 2px;
  }
  .sep {
    margin: 0 4px;
    color: #c7c7cc;
  }
  .tag {
    display: inline-block;
    padding: 1px 6px;
    background: #e5e5ea;
    border-radius: 3px;
    margin-right: 4px;
    font-size: 11px;
  }
  .row-actions {
    display: flex;
    gap: 4px;
  }
  button {
    padding: 4px 10px;
    font-size: 12px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
  }
  .copy {
    background: #007aff;
    color: white;
  }
  .copy:hover {
    background: #0051d5;
  }
  .copy:disabled {
    background: #c7c7cc;
    cursor: default;
  }
  .icon-btn {
    background: transparent;
    color: #6e6e73;
    padding: 4px 8px;
    font-size: 14px;
    line-height: 1;
  }
  .edit:hover {
    background: rgba(0, 122, 255, 0.12);
    color: #007aff;
  }
  .delete {
    font-size: 16px;
  }
  .delete:hover {
    background: rgba(255, 59, 48, 0.12);
    color: #ff3b30;
  }
  @media (prefers-color-scheme: dark) {
    .row {
      background: #2c2c2e;
      border-color: #3a3a3c;
    }
    .row.selected {
      border-color: #0a84ff;
      box-shadow: 0 0 0 2px rgba(10, 132, 255, 0.3);
    }
    .meta {
      color: #98989d;
    }
    .tag {
      background: #3a3a3c;
      color: #f5f5f7;
    }
    .sep {
      color: #48484a;
    }
  }
</style>
