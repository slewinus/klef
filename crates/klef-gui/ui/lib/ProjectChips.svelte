<script lang="ts">
  import type { KeyDto } from "./types";

  interface Props {
    keys: KeyDto[];
    selected: string | null;
    onSelect: (project: string | null) => void;
  }

  let { keys, selected, onSelect }: Props = $props();

  const PROJECT_PREFIX = "project:";

  // Extract unique project names sorted alphabetically. A project is any tag
  // starting with `project:`. The convention lives in the key's tags, so
  // there's no separate schema or storage — `klef list --tag project:foo`
  // already works on the CLI side.
  let projects = $derived.by(() => {
    const set = new Set<string>();
    for (const k of keys) {
      for (const t of k.tags ?? []) {
        if (t.startsWith(PROJECT_PREFIX)) {
          set.add(t.slice(PROJECT_PREFIX.length));
        }
      }
    }
    return [...set].sort();
  });
</script>

{#if projects.length > 0}
  <div class="chips">
    <button
      class="chip"
      class:active={selected === null}
      onclick={() => onSelect(null)}
    >
      All
    </button>
    {#each projects as p (p)}
      <button
        class="chip"
        class:active={selected === p}
        onclick={() => onSelect(selected === p ? null : p)}
      >
        {p}
      </button>
    {/each}
  </div>
{/if}

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px 0 0;
  }
  .chip {
    padding: 2px 8px;
    font-size: 11px;
    background: transparent;
    color: #6e6e73;
    border: 1px solid #d2d2d7;
    border-radius: 999px;
    cursor: pointer;
    font-family: inherit;
  }
  .chip:hover {
    background: #f5f5f7;
  }
  .chip.active {
    background: #007aff;
    border-color: #007aff;
    color: white;
  }
  .chip.active:hover {
    background: #0051d5;
  }
  @media (prefers-color-scheme: dark) {
    .chip {
      color: #98989d;
      border-color: #3a3a3c;
    }
    .chip:hover {
      background: #3a3a3c;
    }
    .chip.active {
      background: #0a84ff;
      border-color: #0a84ff;
      color: white;
    }
    .chip.active:hover {
      background: #0066cc;
    }
  }
</style>
