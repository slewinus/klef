<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { FileUp, Lock, Pin, PinOff, Plus, Power, Settings as SettingsIcon } from "lucide-svelte";

  interface Props {
    pinned: boolean;
    /** Basename of the open age vault, or null on the Keychain. */
    vaultName: string | null;
    /** A configured vault isn't unlocked yet: writes would land in the Keychain. */
    locked: boolean;
    onImport: () => void;
    onSettings: () => void;
    onAdd: () => void;
  }

  let { pinned = $bindable(), vaultName, locked, onImport, onSettings, onAdd }: Props = $props();
</script>

<div class="title-row">
  <div class="title">
    klef
    {#if vaultName}<span class="vault" title="age vault"><Lock size={10} /> {vaultName}</span>{/if}
  </div>
  <div class="header-actions">
    <button
      class="hdr-btn"
      class:active={pinned}
      onclick={() => (pinned = !pinned)}
      title={pinned ? "Unpin (auto-hide on)" : "Pin (keeps popover open)"}
    >
      {#if pinned}<PinOff size={14} />{:else}<Pin size={14} />{/if}
    </button>
    <button class="hdr-btn" onclick={onImport} disabled={locked} title="Import .env file">
      <FileUp size={14} />
    </button>
    <button class="hdr-btn" onclick={onSettings} title="Settings">
      <SettingsIcon size={14} />
    </button>
    <button class="hdr-btn primary" onclick={onAdd} disabled={locked} title="Add key">
      <Plus size={14} />
    </button>
    <button class="hdr-btn" onclick={() => invoke("quit_app")} title="Quit klef">
      <Power size={14} />
    </button>
  </div>
</div>

<style>
  .title-row { display: flex; align-items: center; justify-content: space-between; }
  .title { font-weight: 700; font-size: 13px; letter-spacing: -0.01em; display: flex; align-items: center; gap: 6px; }
  .vault { display: inline-flex; align-items: center; gap: 3px; font-weight: 500; font-size: 10px; color: var(--text-secondary); background: var(--surface-2); padding: 1px 6px; border-radius: var(--radius-sm); font-family: var(--font-mono); }
  .header-actions { display: flex; gap: 2px; }
  .hdr-btn { width: 26px; height: 26px; padding: 0; background: transparent; color: var(--text-secondary); border: none; border-radius: var(--radius-sm); cursor: pointer; font-family: inherit; display: inline-flex; align-items: center; justify-content: center; transition: background 80ms, color 80ms; }
  .hdr-btn:hover { background: var(--hover); color: var(--text); }
  .hdr-btn:disabled { opacity: 0.4; cursor: default; }
  .hdr-btn.active { background: var(--accent-bg); color: var(--accent); }
  .hdr-btn.primary { background: var(--accent); color: #fff; } .hdr-btn.primary:hover { background: var(--accent-hover); }
</style>
