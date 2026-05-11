<script lang="ts">
  import { untrack } from "svelte";
  import {
    applyDotenvImport,
    cancelDotenvImport,
    type DotenvPlan,
  } from "./api";

  interface Props {
    plan: DotenvPlan;
    onClose: () => void;
    onImported: (count: number) => void;
  }

  let { plan, onClose, onImported }: Props = $props();

  // Tell the Rust side to drop the server-side plan when the user cancels.
  // (apply_dotenv_import consumes the session itself on submit.)
  function cancelAndClose() {
    void cancelDotenvImport(plan.session_id).catch(() => {});
    onClose();
  }

  // The modal is mounted/unmounted per-import (see {#if dotenvPlan} in
  // App.svelte) so we want a one-shot snapshot, not a reactive binding.
  let project = $state(untrack(() => plan.suggested_project));
  let rewriteSource = $state(true);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Track per-row "include" toggle. Default: include new + conflict, skip
  // ref + empty. User can flip individual rows.
  let included = $state<Record<string, boolean>>(
    untrack(() =>
      Object.fromEntries(
        plan.items.map((it) => [
          it.env_var,
          it.status === "new" || it.status === "conflict",
        ]),
      ),
    ),
  );

  let toImportCount = $derived(
    plan.items.filter((it) => included[it.env_var]).length,
  );

  // True when every line is already a klef: ref (or empty) — nothing
  // importable. Common after a re-drop following a previous rewrite.
  let allInert = $derived(
    plan.items.length > 0 &&
      plan.items.every((it) => it.status === "ref" || it.status === "empty"),
  );

  async function submit(e: Event) {
    e.preventDefault();
    if (submitting || toImportCount === 0) return;
    submitting = true;
    error = null;
    try {
      const accepted = plan.items
        .filter((it) => included[it.env_var])
        .map((it) => it.env_var);
      const count = await applyDotenvImport(
        plan.session_id,
        project.trim(),
        rewriteSource,
        accepted,
      );
      onImported(count);
    } catch (err) {
      error = String(err);
    } finally {
      submitting = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      cancelAndClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="backdrop"
  role="button"
  tabindex="-1"
  aria-label="Close dialog"
  onclick={cancelAndClose}
  onkeydown={(e) => e.key === "Enter" && cancelAndClose()}
></div>
<form class="modal" onsubmit={submit}>
  <h2>Import .env</h2>
  <small class="src">{plan.source_path}</small>

  {#if allInert}
    <div class="banner">
      Every line in this <code>.env</code> is already a <code>klef:</code> ref
      or empty — nothing to import. If the secret values are gone (you
      deleted the keys) the original values can't be recovered from this
      file. Restore them from a backup or recreate manually.
    </div>
  {/if}

  <label>
    <span>Project tag (auto-suggested from folder)</span>
    <input
      bind:value={project}
      type="text"
      autocomplete="off"
      spellcheck="false"
    />
    <small>All imported keys will be tagged <code>project:{project}</code></small>
  </label>

  <label class="checkbox">
    <input type="checkbox" bind:checked={rewriteSource} />
    <span>Rewrite source <code>.env</code> with <code>klef:</code> refs after import</span>
  </label>

  <div class="rows">
    {#each plan.items as it (it.env_var)}
      <label class="row" class:disabled={it.status === "ref" || it.status === "empty"}>
        <input
          type="checkbox"
          bind:checked={included[it.env_var]}
          disabled={it.status === "ref" || it.status === "empty"}
        />
        <div class="meta">
          <div class="env">{it.env_var}</div>
          <div class="sub">
            <span class="klef">→ {it.klef_name}</span>
            <span class="status status-{it.status}">{it.status}</span>
          </div>
        </div>
        <div class="value">{it.redacted_value}</div>
      </label>
    {/each}
  </div>

  {#if error}
    <div class="err">{error}</div>
  {/if}

  <div class="actions">
    <button type="button" class="cancel" onclick={cancelAndClose} disabled={submitting}>
      Cancel
    </button>
    <button
      type="submit"
      class="primary"
      disabled={submitting || toImportCount === 0 || !project.trim()}
    >
      {submitting ? "Importing…" : `Import ${toImportCount} key${toImportCount === 1 ? "" : "s"}`}
    </button>
  </div>
</form>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 10;
    border: none;
    cursor: pointer;
  }
  .modal {
    position: absolute;
    inset: 12px;
    background: var(--surface);
    border-radius: 8px;
    padding: 14px;
    z-index: 11;
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.25);
    overflow-y: auto;
  }
  h2 { margin: 0; font-size: 14px; font-weight: 600; }
  .src { color: var(--text-tertiary); font-size: 10px; word-break: break-all; }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11px;
    color: var(--text-secondary);
  }
  input[type="text"] {
    padding: 5px 8px;
    font-size: 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: inherit;
    font-family: inherit;
    outline: none;
  }
  input[type="text"]:focus { border-color: var(--accent); box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2); }
  small { color: var(--text-tertiary); font-size: 10px; }
  .banner { background: var(--surface)5cc; color: #8a4500; padding: 8px 10px; border-radius: var(--radius); font-size: 12px; line-height: 1.4; }
  label.checkbox { flex-direction: row; align-items: center; gap: 6px; font-size: 12px; color: inherit; }
  label.checkbox input { width: auto; }
  code { background: var(--surface-2); padding: 0 4px; border-radius: 3px; }
  .rows { display: flex; flex-direction: column; gap: 4px; max-height: 200px; overflow-y: auto; }
  label.row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: pointer;
    flex-direction: row;
  }
  label.row.disabled { cursor: default; opacity: 0.5; }
  label.row .env { font-weight: 600; color: var(--text); font-size: 12px; }
  label.row .sub { display: flex; gap: 6px; align-items: center; font-size: 11px; }
  label.row .klef { color: var(--text-secondary); }
  label.row .value { font-family: ui-monospace, monospace; font-size: 11px; color: var(--text-secondary); }
  .status { padding: 0 6px; border-radius: 3px; font-size: 10px; text-transform: uppercase; font-weight: 600; }
  .status-new { background: #d1f4d1; color: #006400; }
  .status-conflict { background: #ffe5b3; color: #8a4500; }
  .status-ref { background: #d2d2d7; color: var(--text-secondary); }
  .status-empty { background: #d2d2d7; color: var(--text-secondary); }
  .actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
  .actions button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: var(--radius);
    cursor: pointer;
    font-family: inherit;
    border: 1px solid var(--border);
    background: var(--surface);
    color: inherit;
  }
  .actions button.primary { background: var(--accent); color: white; border-color: var(--accent); }
  .actions button.primary:hover:not(:disabled) { background: var(--accent-hover); }
  .actions button.cancel:hover:not(:disabled) { background: var(--surface-2); }
  .actions button:disabled { opacity: 0.6; cursor: default; }
  .err { color: var(--danger); font-size: 12px; padding: 6px 8px; background: rgba(255, 59, 48, 0.08); border-radius: var(--radius-sm); }
  </style>
