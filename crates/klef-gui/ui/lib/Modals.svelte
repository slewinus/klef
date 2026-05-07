<script lang="ts">
  import type { DotenvPlan } from "./api";
  import type { KeyDto } from "./types";
  import AddKeyModal from "./AddKeyModal.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import DotenvImportModal from "./DotenvImportModal.svelte";
  import EditKeyModal from "./EditKeyModal.svelte";
  import SettingsModal from "./SettingsModal.svelte";

  interface Props {
    showAddModal: boolean;
    showSettings: boolean;
    editTarget: KeyDto | null;
    pendingDelete: KeyDto | null;
    dotenvPlan: DotenvPlan | null;
    onAddClose: () => void;
    onAddDone: () => void | Promise<void>;
    onEditClose: () => void;
    onEditDone: () => void | Promise<void>;
    onSettingsClose: () => void;
    onDeleteCancel: () => void;
    onDeleteConfirm: () => void | Promise<void>;
    onDotenvClose: () => void;
    onDotenvDone: (count: number) => void | Promise<void>;
  }

  let p: Props = $props();
</script>

{#if p.showAddModal}
  <AddKeyModal onClose={p.onAddClose} onAdded={p.onAddDone} />
{/if}

{#if p.editTarget}
  <EditKeyModal target={p.editTarget} onClose={p.onEditClose} onSaved={p.onEditDone} />
{/if}

{#if p.pendingDelete}
  <ConfirmDialog
    title="Delete key"
    message="Permanently delete “{p.pendingDelete.name}”? This removes the value from the Keychain and the index entry."
    confirmLabel="Delete"
    danger
    onConfirm={p.onDeleteConfirm}
    onCancel={p.onDeleteCancel}
  />
{/if}

{#if p.showSettings}
  <SettingsModal onClose={p.onSettingsClose} />
{/if}

{#if p.dotenvPlan}
  <DotenvImportModal plan={p.dotenvPlan} onClose={p.onDotenvClose} onImported={p.onDotenvDone} />
{/if}
