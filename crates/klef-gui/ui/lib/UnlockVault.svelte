<script lang="ts">
  import { Lock } from "lucide-svelte";

  interface Props {
    path: string;
    /** Rejects with a message (e.g. wrong passphrase) to show inline. */
    onUnlock: (passphrase: string) => Promise<void>;
    onUseKeychain: () => void;
  }

  let { path, onUnlock, onUseKeychain }: Props = $props();

  let passphrase = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let input: HTMLInputElement;

  $effect(() => {
    setTimeout(() => input?.focus(), 0);
  });

  async function submit(e: Event) {
    e.preventDefault();
    if (busy || passphrase.length === 0) return;
    busy = true;
    error = null;
    try {
      await onUnlock(passphrase);
    } catch (err) {
      error = String(err);
      input?.select();
    } finally {
      // Don't keep the passphrase in the DOM once it has been used.
      passphrase = "";
      busy = false;
    }
  }
</script>

<form class="unlock" onsubmit={submit}>
  <Lock size={20} />
  <div class="path" title={path}>{path}</div>
  <input
    bind:this={input}
    bind:value={passphrase}
    type="password"
    placeholder="Vault passphrase"
    autocomplete="off"
    disabled={busy}
  />
  {#if error}<div class="err">{error}</div>{/if}
  <button type="submit" class="primary" disabled={busy || passphrase.length === 0}>
    {busy ? "Unlocking…" : "Unlock"}
  </button>
  <button type="button" class="link" onclick={onUseKeychain}>Use the Keychain instead</button>
</form>

<style>
  .unlock { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 28px 20px; color: var(--text-secondary); }
  .path { font-family: var(--font-mono); font-size: 11px; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  input { width: 100%; padding: 6px 8px; font-size: 13px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--surface); color: var(--text); outline: none; font-family: inherit; }
  input:focus { border-color: var(--accent); box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2); }
  .err { color: var(--danger); font-size: 11px; }
  button { font-family: inherit; font-size: 12px; cursor: pointer; }
  .primary { width: 100%; padding: 6px 12px; border-radius: var(--radius); border: 1px solid var(--accent); background: var(--accent); color: #fff; }
  .primary:disabled { opacity: 0.5; cursor: default; }
  .link { background: none; border: none; color: var(--text-secondary); text-decoration: underline; }
</style>
