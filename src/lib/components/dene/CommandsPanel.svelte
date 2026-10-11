<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { userCommands, loadUserCommands, notify } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();

  let editing = null;
  let newCmd = emptyCmd();

  function emptyCmd() {
    return {
      id: "cmd-" + Date.now(),
      name: "", description: "", exec: "",
      icon: "⚡", show_in_context_menu: true,
      show_in_toolbar: false, shortcut: null,
      working_dir: null, terminal: false, category: "tools"
    };
  }

  async function save(cmd) {
    try {
      await invoke("save_user_command", { cmd });
      await loadUserCommands();
      editing = null;
      newCmd = emptyCmd();
      notify("success", `Saved: ${cmd.name}`);
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function del(id) {
    if (!confirm("Delete this command?")) return;
    await invoke("delete_user_command", { id });
    await loadUserCommands();
    notify("success", "Command deleted");
  }
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">
    <div class="panel-header">
      <span>Custom Commands</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <div class="panel-body">
      <!-- Existing commands -->
      <div class="cmd-list">
        {#each $userCommands as cmd}
          <div class="cmd-row" class:active-edit={editing?.id === cmd.id}>
            <div class="cmd-info">
              <span class="cmd-icon">{cmd.icon}</span>
              <div>
                <div class="cmd-name">{cmd.name}</div>
                <code class="cmd-exec">{cmd.exec}</code>
              </div>
            </div>
            <div class="cmd-actions">
              <button class="btn" on:click={() => editing = {...cmd}}>Edit</button>
              <button class="btn btn-danger" on:click={() => del(cmd.id)}>Del</button>
            </div>
          </div>
        {/each}
      </div>

      <!-- Edit form -->
      {#if editing}
        <div class="cmd-form glass-panel">
          <h3>Edit: {editing.name}</h3>
          <div class="form-grid">
            <label>Name<input bind:value={editing.name} /></label>
            <label>Icon<input bind:value={editing.icon} placeholder="⚡" style="width:60px"/></label>
            <label class="full">Description<input bind:value={editing.description} /></label>
            <label class="full">
              Command
              <input bind:value={editing.exec} placeholder="git status — use {'{path}'} {'{dir}'} {'{file}'}"/>
            </label>
            <label class="full">Working Dir (blank = current)<input bind:value={editing.working_dir} placeholder="Leave blank for current directory"/></label>
            <label class="full">Category<input bind:value={editing.category} /></label>
            <label>Shortcut<input bind:value={editing.shortcut} placeholder="ctrl+shift+x"/></label>
            <label class="checkbox"><input type="checkbox" bind:checked={editing.terminal} /> Run in terminal</label>
            <label class="checkbox"><input type="checkbox" bind:checked={editing.show_in_context_menu} /> Context menu</label>
            <label class="checkbox"><input type="checkbox" bind:checked={editing.show_in_toolbar} /> Toolbar</label>
          </div>
          <div class="form-actions">
            <button class="btn btn-accent" on:click={() => save(editing)}>Save</button>
            <button class="btn" on:click={() => editing = null}>Cancel</button>
          </div>
        </div>
      {:else}
        <!-- Add new -->
        <div class="cmd-form glass-panel">
          <h3>New Command</h3>
          <div class="form-grid">
            <label>Name<input bind:value={newCmd.name} /></label>
            <label>Icon<input bind:value={newCmd.icon} style="width:60px"/></label>
            <label class="full">Command<input bind:value={newCmd.exec} placeholder="e.g. git status — supports {'{path}'} {'{dir}'} {'{file}'}"/></label>
            <label class="checkbox"><input type="checkbox" bind:checked={newCmd.terminal} /> Run in terminal</label>
            <label class="checkbox"><input type="checkbox" bind:checked={newCmd.show_in_context_menu} /> Context menu</label>
          </div>
          <button class="btn btn-accent" on:click={() => save(newCmd)} disabled={!newCmd.name || !newCmd.exec}>
            Add Command
          </button>
        </div>
      {/if}

      <div class="tip">
        <strong>Placeholders:</strong>
        <code>{'{path}'}</code> full path,
        <code>{'{file}'}</code> filename,
        <code>{'{dir}'}</code> parent dir,
        <code>{'{name}'}</code> name without ext
      </div>
    </div>
  </div>
</div>

<style>
  .overlay { position: fixed; inset:0; background:rgba(0,0,0,0.45); backdrop-filter:var(--blur-ui) !important; -webkit-backdrop-filter:var(--blur-ui) !important; display:flex; align-items:center; justify-content:center; z-index:100; }
  .panel { width:600px; max-height:85vh; border-radius:var(--radius-lg); display:flex; flex-direction:column; overflow:hidden; }
  .panel-header { display:flex; justify-content:space-between; align-items:center; padding:14px 18px; border-bottom:1px solid var(--border); font-weight:600; }
  .panel-header button { background:none;border:none;color:var(--text-dim);cursor:pointer;font-size:14px; }
  .panel-body { flex:1; overflow-y:auto; padding:12px; display:flex; flex-direction:column; gap:10px; }

  .cmd-list { display:flex; flex-direction:column; gap:4px; }
  .cmd-row { display:flex; align-items:center; justify-content:space-between; padding:8px 10px; border-radius:var(--radius); border:1px solid var(--border); background:var(--bg-tertiary); gap:10px; }
  .cmd-row.active-edit { border-color:var(--border-accent); }
  .cmd-info { display:flex; align-items:center; gap:10px; flex:1; min-width:0; }
  .cmd-icon { font-size:18px; flex-shrink:0; }
  .cmd-name { font-size:13px; font-weight:500; }
  .cmd-exec { font-family:var(--font-mono); font-size:10px; color:var(--text-dim); }
  .cmd-actions { display:flex; gap:5px; }

  .cmd-form { padding:14px; border-radius:var(--radius); display:flex; flex-direction:column; gap:10px; }
  .cmd-form h3 { font-size:13px; font-weight:600; color:var(--text-secondary); }
  .form-grid { display:grid; grid-template-columns:1fr 1fr; gap:8px; }
  .form-grid label { display:flex; flex-direction:column; gap:4px; font-size:11px; color:var(--text-dim); }
  .form-grid label.full { grid-column:1/-1; }
  .form-grid label.checkbox { flex-direction:row; align-items:center; gap:6px; color:var(--text-secondary); font-size:12px; }
  .form-actions { display:flex; gap:8px; }

  .tip { font-size:11px; color:var(--text-dim); padding:8px 10px; background:var(--bg-tertiary); border-radius:var(--radius-sm); }
  .tip code { font-family:var(--font-mono); color:var(--text-accent); background:var(--bg-secondary); padding:1px 4px; border-radius:3px; }
</style>
