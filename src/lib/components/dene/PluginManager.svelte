<script>
  import { createEventDispatcher, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { notify } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();

  let plugins      = [];
  let loading      = true;
  let pluginsDir   = "";
  let runningId    = null;
  let testOutput   = "";

  onMount(async () => {
    pluginsDir = await invoke("get_plugins_dir").catch(() => "");
    await loadPlugins();
  });

  async function loadPlugins() {
    loading = true;
    try { plugins = await invoke("list_plugins"); }
    catch (e) { notify("error", String(e)); }
    finally { loading = false; }
  }

  async function toggle(plugin) {
    try {
      await invoke("set_plugin_enabled", { pluginId: plugin.id, enabled: !plugin.enabled });
      await loadPlugins();
    } catch (e) { notify("error", String(e)); }
  }

  async function testRun(plugin) {
    runningId  = plugin.id;
    testOutput = "";
    try {
      const res = await invoke("run_plugin", {
        pluginId: plugin.id,
        selectedPath: pluginsDir,
        currentDir: pluginsDir,
      });
      testOutput = res.output || (res.success ? "OK — çıktı yok" : "Başarısız");
    } catch (e) {
      testOutput = String(e);
    } finally { runningId = null; }
  }

  async function createExample() {
    try {
      const path = await invoke("create_example_plugin");
      notify("success", `Örnek plugin oluşturuldu: ${path}`);
      await loadPlugins();
    } catch (e) { notify("error", String(e)); }
  }

  function openDir() {
    invoke("run_user_command", {
      commandId: "open-terminal",
      selectedPath: pluginsDir,
      currentDir: pluginsDir,
    }).catch(() => notify("info", `Plugin dizini: ${pluginsDir}`));
  }

  const WHEN_LABELS = {
    always: "Her zaman",
    file:   "Dosya seçiliyken",
    dir:    "Klasör seçiliyken",
    archive:"Arşiv seçiliyken",
  };
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">

    <div class="panel-header">
      <span>⚡ Plugin Yöneticisi</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <div class="panel-body">

      <!-- Info bar -->
      <div class="info-bar">
        <span class="info-path">{pluginsDir}</span>
        <button class="btn" on:click={openDir} title="Terminalde aç">⬛ Aç</button>
        <button class="btn" on:click={loadPlugins}>↻</button>
      </div>

      <!-- Plugin listesi -->
      {#if loading}
        <div class="state-msg"><div class="spinner"></div></div>
      {:else if plugins.length === 0}
        <div class="state-msg">
          <span style="font-size:32px">⚡</span>
          <span>Henüz plugin yok</span>
          <button class="btn btn-accent" on:click={createExample}>Örnek Plugin Oluştur</button>
        </div>
      {:else}
        <div class="plugin-list">
          {#each plugins as p}
            <div class="plugin-row" class:disabled={!p.enabled}>
              <div class="plugin-icon">{p.icon}</div>
              <div class="plugin-info">
                <div class="plugin-name">{p.name}
                  <span class="plugin-version">v{p.version}</span>
                  {#if p.author !== "unknown"}
                    <span class="plugin-author">by {p.author}</span>
                  {/if}
                </div>
                <div class="plugin-desc">{p.description}</div>
                <div class="plugin-meta">
                  <span class="plugin-when">{WHEN_LABELS[p.when] || p.when}</span>
                  {#if p.shortcut}<span class="plugin-shortcut">{p.shortcut}</span>{/if}
                </div>
              </div>
              <div class="plugin-actions">
                <button
                  class="btn"
                  class:btn-accent={!p.enabled}
                  on:click={() => toggle(p)}
                  title={p.enabled ? "Devre dışı bırak" : "Etkinleştir"}
                >
                  {p.enabled ? "Kapat" : "Aç"}
                </button>
                <button
                  class="btn"
                  on:click={() => testRun(p)}
                  disabled={runningId === p.id}
                >
                  {runningId === p.id ? "..." : "Test"}
                </button>
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Test output -->
      {#if testOutput}
        <div class="test-output">
          <div class="test-output-title">Test Çıktısı</div>
          <pre class="test-output-pre">{testOutput}</pre>
        </div>
      {/if}

      <!-- Yeni plugin oluştur -->
      <div class="create-section">
        <button class="btn btn-accent" on:click={createExample}>+ Örnek Plugin Oluştur</button>
        <p class="hint">
          Plugin dizinine bir klasör oluştur, içine <code>plugin.toml</code> + <code>run.sh</code> koy.
          Env var'lar: <code>ARCH_PATH</code> <code>ARCH_DIR</code> <code>ARCH_SELECTED</code>
        </p>
      </div>

    </div>
  </div>
</div>

<style>
  .overlay { position:fixed; inset:0; background:rgba(0,0,0,0.45); backdrop-filter:var(--blur-ui) !important; -webkit-backdrop-filter:var(--blur-ui) !important; display:flex; align-items:center; justify-content:center; z-index:110; }
  .panel { width:580px; max-height:80vh; border-radius:var(--radius-lg); display:flex; flex-direction:column; overflow:hidden; }
  .panel-header { display:flex; justify-content:space-between; align-items:center; padding:14px 18px; border-bottom:1px solid var(--border); font-weight:600; font-size:13px; flex-shrink:0; }
  .panel-header button { background:none; border:none; color:var(--text-dim); cursor:pointer; font-size:14px; }
  .panel-body { flex:1; overflow-y:auto; padding:12px; display:flex; flex-direction:column; gap:10px; }

  .info-bar { display:flex; align-items:center; gap:8px; padding:6px 8px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); }
  .info-path { flex:1; font-family:var(--font-mono); font-size:10px; color:var(--text-dim); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .state-msg { display:flex; flex-direction:column; align-items:center; gap:12px; padding:32px; color:var(--text-dim); font-size:12px; }

  .plugin-list { display:flex; flex-direction:column; gap:6px; }
  .plugin-row { display:flex; align-items:flex-start; gap:10px; padding:10px 12px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius); transition:border-color 0.1s; }
  .plugin-row:hover { border-color:var(--border-accent); }
  .plugin-row.disabled { opacity:0.5; }
  .plugin-icon { font-size:22px; flex-shrink:0; margin-top:2px; }
  .plugin-info { flex:1; min-width:0; display:flex; flex-direction:column; gap:3px; }
  .plugin-name { font-size:13px; font-weight:600; color:var(--text-primary); display:flex; align-items:center; gap:6px; flex-wrap:wrap; }
  .plugin-version { font-size:10px; color:var(--text-dim); font-weight:400; font-family:var(--font-mono); }
  .plugin-author { font-size:10px; color:var(--text-dim); font-weight:400; }
  .plugin-desc { font-size:11px; color:var(--text-secondary); }
  .plugin-meta { display:flex; gap:6px; align-items:center; }
  .plugin-when { font-size:9px; padding:1px 6px; background:var(--bg-secondary); border:1px solid var(--border); border-radius:4px; color:var(--text-dim); }
  .plugin-shortcut { font-size:9px; padding:1px 6px; background:var(--accent-dim); border:1px solid var(--border-accent); border-radius:4px; color:var(--accent); font-family:var(--font-mono); }
  .plugin-actions { display:flex; gap:5px; flex-shrink:0; }

  .test-output { background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); overflow:hidden; }
  .test-output-title { padding:5px 10px; font-size:10px; font-weight:600; color:var(--text-dim); text-transform:uppercase; letter-spacing:0.08em; border-bottom:1px solid var(--border); }
  .test-output-pre { margin:0; padding:8px 10px; font-family:var(--font-mono); font-size:10px; color:var(--text-secondary); overflow:auto; max-height:120px; white-space:pre-wrap; }

  .create-section { display:flex; flex-direction:column; gap:8px; padding:10px; background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-sm); }
  .hint { font-size:11px; color:var(--text-dim); line-height:1.6; margin:0; }
  .hint code { font-family:var(--font-mono); color:var(--text-accent); background:var(--bg-secondary); padding:1px 4px; border-radius:3px; }

  .spinner { width:24px; height:24px; border:2px solid var(--border); border-top-color:var(--accent); border-radius:50%; animation:spin 0.7s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
</style>
