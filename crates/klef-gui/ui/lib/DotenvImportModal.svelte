<script lang="ts">
  import { applyDotenvImport, type DotenvPlan } from "./api";

  interface Props {
    plan: DotenvPlan;
    onClose: () => void;
    onImported: (count: number) => void;
  }

  let { plan, onClose, onImported }: Props = $props();

  let project = $state(plan.suggested_project);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Track per-row "include" toggle. Default: include new + conflict, skip
  // ref + empty. User can flip individual rows.
  let included = $state<Record<string, boolean>>(
    Object.fromEntries(
      plan.items.map((it) => [it.env_var, it.status === "new" || it.status === "conflict"]),
    ),
  );

  let toImportCount = $derived(
    plan.items.filter((it) => included[it.env_var]).length,
  );

  async function submit(e: Event) {
    e.preventDefault();
    if (submitting || toImportCount === 0) return;
    submitting = true;
    error = null;
    try {
      const items = plan.items.filter((it) => included[it.env_var]);
      const count = await applyDotenvImport(items, project.trim());
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
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="backdrop"
  role="button"
  tabindex="-1"
  aria-label="Close dialog"
  onclick={onClose}
  onkeydown={(e) => e.key === "Enter" && onClose()}
></div>
<form class="modal" onsubmit={submit}>
  <h2>Import .env</h2>
  <small class="src">{plan.source_path}</small>

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
    <button type="button" class="cancel" onclick={onClose} disabled={submitting}>
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
    background: #fff;
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
  .src { color: #98989d; font-size: 10px; word-break: break-all; }
  label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 11px;
    color: #6e6e73;
  }
  input[type="text"] {
    padding: 5px 8px;
    font-size: 13px;
    border: 1px solid #d2d2d7;
    border-radius: 5px;
    background: #fff;
    color: inherit;
    font-family: inherit;
    outline: none;
  }
  input[type="text"]:focus { border-color: #007aff; box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2); }
  small { color: #98989d; font-size: 10px; }
  code { background: #e5e5ea; padding: 0 4px; border-radius: 3px; }
  .rows { display: flex; flex-direction: column; gap: 4px; max-height: 200px; overflow-y: auto; }
  label.row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 1px solid #d2d2d7;
    border-radius: 5px;
    cursor: pointer;
    flex-direction: row;
  }
  label.row.disabled { cursor: default; opacity: 0.5; }
  label.row .env { font-weight: 600; color: #1d1d1f; font-size: 12px; }
  label.row .sub { display: flex; gap: 6px; align-items: center; font-size: 11px; }
  label.row .klef { color: #6e6e73; }
  label.row .value { font-family: ui-monospace, monospace; font-size: 11px; color: #6e6e73; }
  .status { padding: 0 6px; border-radius: 3px; font-size: 10px; text-transform: uppercase; font-weight: 600; }
  .status-new { background: #d1f4d1; color: #006400; }
  .status-conflict { background: #ffe5b3; color: #8a4500; }
  .status-ref { background: #d2d2d7; color: #6e6e73; }
  .status-empty { background: #d2d2d7; color: #6e6e73; }
  .actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 4px; }
  .actions button {
    padding: 5px 12px;
    font-size: 12px;
    border-radius: 5px;
    cursor: pointer;
    font-family: inherit;
    border: 1px solid #d2d2d7;
    background: #fff;
    color: inherit;
  }
  .actions button.primary { background: #007aff; color: white; border-color: #007aff; }
  .actions button.primary:hover:not(:disabled) { background: #0051d5; }
  .actions button.cancel:hover:not(:disabled) { background: #f5f5f7; }
  .actions button:disabled { opacity: 0.6; cursor: default; }
  .err { color: #ff3b30; font-size: 12px; padding: 6px 8px; background: rgba(255, 59, 48, 0.08); border-radius: 4px; }
  @media (prefers-color-scheme: dark) {
    .modal { background: #2c2c2e; }
    input[type="text"] { background: #1d1d1f; border-color: #3a3a3c; color: #f5f5f7; }
    code { background: #3a3a3c; }
    label.row { border-color: #3a3a3c; }
    label.row .env { color: #f5f5f7; }
    .status-ref, .status-empty { background: #3a3a3c; color: #98989d; }
    .actions button { background: #3a3a3c; border-color: #3a3a3c; color: #f5f5f7; }
    .actions button:hover:not(:disabled) { background: #48484a; }
    .actions button.primary { background: #0a84ff; border-color: #0a84ff; color: white; }
  }
</style>
