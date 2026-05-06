<script lang="ts">
  interface Props {
    value: string;
    placeholder?: string;
  }

  let { value = $bindable(""), placeholder = "Search…" }: Props = $props();
  let inputEl: HTMLInputElement;

  // Auto-focus when the popover opens. The parent calls focus() via this.
  export function focus() {
    inputEl?.focus();
    inputEl?.select();
  }
</script>

<input
  bind:this={inputEl}
  bind:value
  type="search"
  {placeholder}
  autocomplete="off"
  spellcheck="false"
/>

<style>
  input {
    width: 100%;
    padding: 6px 10px;
    font-size: 13px;
    border: 1px solid #d2d2d7;
    border-radius: 6px;
    background: #fff;
    color: inherit;
    font-family: inherit;
    outline: none;
  }
  input:focus {
    border-color: #007aff;
    box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2);
  }
  /* Hide the WebKit clear button — we'll add a clear shortcut later. */
  input::-webkit-search-cancel-button {
    -webkit-appearance: none;
  }
  @media (prefers-color-scheme: dark) {
    input {
      background: #2c2c2e;
      border-color: #3a3a3c;
      color: #f5f5f7;
    }
    input:focus {
      border-color: #0a84ff;
      box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.3);
    }
  }
</style>
