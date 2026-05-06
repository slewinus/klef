<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    onRetry: () => void | Promise<void>;
  }

  let { onRetry }: Props = $props();
  let retrying = $state(false);

  async function openKeychainAccess() {
    try {
      await invoke("open_keychain_access");
    } catch (e) {
      console.warn("failed to open Keychain Access", e);
    }
  }

  async function retry() {
    retrying = true;
    try {
      await onRetry();
    } finally {
      retrying = false;
    }
  }
</script>

<div class="help">
  <h3>klef can't reach your Keychain</h3>
  <p>
    macOS denied access to your login Keychain. This usually happens when
    you clicked <strong>Don't Allow</strong> on the first prompt.
  </p>

  <ol>
    <li>Open <strong>Keychain Access</strong> (button below).</li>
    <li>
      Pick the <em>login</em> keychain on the left, then search for
      <strong>klef</strong>.
    </li>
    <li>
      Right-click the entry → <strong>Get Info</strong> →
      <strong>Access Control</strong> tab.
    </li>
    <li>
      Either tick <strong>Allow all applications to access this item</strong>,
      or add <code>klef-gui</code> to the always-allow list.
    </li>
    <li>Click <strong>Save Changes</strong>, then come back and Retry.</li>
  </ol>

  <div class="actions">
    <button onclick={openKeychainAccess}>Open Keychain Access</button>
    <button class="primary" onclick={retry} disabled={retrying}>
      {retrying ? "Retrying…" : "Retry"}
    </button>
  </div>
</div>

<style>
  .help {
    padding: 16px;
    font-size: 12px;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
  }
  p {
    margin: 0 0 10px;
    color: #6e6e73;
  }
  ol {
    margin: 0 0 12px;
    padding-left: 20px;
    color: #1d1d1f;
  }
  ol li {
    margin-bottom: 4px;
  }
  code {
    background: #e5e5ea;
    padding: 1px 4px;
    border-radius: 3px;
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: 5px;
    cursor: pointer;
    font-family: inherit;
    border: 1px solid #d2d2d7;
    background: #fff;
    color: inherit;
  }
  button:hover { background: #f5f5f7; }
  button.primary { background: #007aff; color: white; border-color: #007aff; }
  button.primary:hover { background: #0051d5; }
  button:disabled { opacity: 0.6; cursor: default; }
  @media (prefers-color-scheme: dark) {
    ol { color: #f5f5f7; }
    code { background: #3a3a3c; }
    button { background: #3a3a3c; border-color: #3a3a3c; color: #f5f5f7; }
    button:hover { background: #48484a; }
    button.primary { background: #0a84ff; border-color: #0a84ff; color: white; }
  }
</style>
