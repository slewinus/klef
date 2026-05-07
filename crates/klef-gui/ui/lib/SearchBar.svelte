<script lang="ts">
  import { Search } from "lucide-svelte";

  interface Props {
    value: string;
    placeholder?: string;
  }

  let { value = $bindable(""), placeholder = "Search keys…" }: Props = $props();
  let inputEl: HTMLInputElement;

  export function focus() {
    inputEl?.focus();
    inputEl?.select();
  }
</script>

<div class="wrap">
  <Search class="icon" size={14} />
  <input
    bind:this={inputEl}
    bind:value
    type="search"
    {placeholder}
    autocomplete="off"
    spellcheck="false"
  />
</div>

<style>
  .wrap {
    position: relative;
    display: flex;
    align-items: center;
  }
  .wrap :global(.icon) {
    position: absolute;
    left: 8px;
    color: var(--text-tertiary);
    pointer-events: none;
  }
  input {
    width: 100%;
    padding: 6px 10px 6px 30px;
    font-size: 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    color: inherit;
    font-family: inherit;
    outline: none;
    transition: background 80ms, border-color 80ms, box-shadow 80ms;
  }
  input::placeholder { color: var(--text-tertiary); }
  input:focus {
    background: var(--surface);
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-bg);
  }
  input::-webkit-search-cancel-button {
    -webkit-appearance: none;
  }
</style>
