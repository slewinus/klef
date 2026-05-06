<script lang="ts">
  import type { KeyDto } from "./types";

  interface Props {
    key: KeyDto;
    onCopy: (key: KeyDto) => void | Promise<void>;
    onDelete: (key: KeyDto) => void;
  }

  let { key, onCopy, onDelete }: Props = $props();
  let copying = $state(false);

  async function handleClick() {
    copying = true;
    try {
      await onCopy(key);
    } finally {
      copying = false;
    }
  }
</script>

<div class="row">
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
      class="delete"
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
  .delete {
    background: transparent;
    color: #6e6e73;
    padding: 4px 8px;
    font-size: 16px;
    line-height: 1;
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
