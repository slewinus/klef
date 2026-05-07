<script lang="ts">
  import type { KeyDto } from "./types";

  interface Props {
    keys: KeyDto[];
    selected: string | null;
    onSelect: (project: string | null) => void;
  }

  let { keys, selected, onSelect }: Props = $props();

  const PROJECT_PREFIX = "project:";

  // A project is any tag starting with `project:`. The convention lives in
  // the key's tags so there's no separate schema or storage —
  // `klef list --tag project:foo` already works on the CLI side.
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
    gap: 3px;
  }
  .chip {
    padding: 2px 8px;
    font-size: 11px;
    background: var(--surface-2);
    color: var(--text-secondary);
    border: none;
    border-radius: 999px;
    cursor: pointer;
    font-family: inherit;
    transition: background 80ms, color 80ms;
  }
  .chip:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .chip.active {
    background: var(--accent);
    color: #fff;
  }
  .chip.active:hover {
    background: var(--accent-hover);
    color: #fff;
  }
</style>
