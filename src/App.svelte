<script>
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import Titlebar    from "./lib/components/Titlebar.svelte";
  import TabBar      from "./lib/components/TabBar.svelte";
  import Sidebar     from "./lib/components/Sidebar.svelte";
  import Toolbar     from "./lib/components/Toolbar.svelte";
  import FileGrid    from "./lib/components/FileGrid.svelte";
  import StatusBar   from "./lib/components/StatusBar.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Notifications from "./lib/components/Notifications.svelte";
  import MountPanel  from "./lib/components/MountPanel.svelte";
  import CommandsPanel from "./lib/components/CommandsPanel.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import PreviewPanel from "./lib/components/PreviewPanel.svelte";
  import BulkRename  from "./lib/components/BulkRename.svelte";

  import DevicesView from "./lib/components/DevicesView.svelte";

  import PaneColumn          from "./lib/components/PaneColumn.svelte";
  import PluginManager       from "./lib/components/PluginManager.svelte";
  import NetworkSharesPanel  from "./lib/components/NetworkSharesPanel.svelte";
  import FileConflictModal   from "./lib/components/FileConflictModal.svelte";
  import OpenWithPanel       from "./lib/components/OpenWithPanel.svelte";

  import { get } from "svelte/store";
  import {
    loadDirectory, loadDrives, loadBookmarks, loadUserCommands,
    loadBlockDevices,
    navigateBack, navigateForward, navigateUp,
    currentPath, theme, showHidden, showPreview,
    addTab, autoReload, showDevices, showDualPane, activePaneId,
    sudoDialog, openWithDialog, adminMode, alwaysLaunchSudo, sessionSudoPassword, requestSudo, notify,
    selectedPaths, rightSelectedPaths, rightEntries, displayEntries, openEntry,
    copySelected, cutSelected, pasteClipboard, selectAll, deleteItems, rightPaneNavRequest, renamingPath,
    setFolderViewMode,
  } from "./lib/stores/explorer.js";

  let globalPasswordInput = "";
  let globalPasswordError = "";
  let globalPasswordBusy  = false;
  let globalPasswordInputEl;

  $: if ($sudoDialog) {
    globalPasswordInput = "";
    globalPasswordError = "";
    globalPasswordBusy = false;
    setTimeout(() => globalPasswordInputEl?.focus(), 60);
  }

  async function submitGlobalPassword() {
    if (!globalPasswordInput || !$sudoDialog) return;
    globalPasswordBusy = true;
    globalPasswordError = "";
    try {
      await $sudoDialog.onSubmit(globalPasswordInput);
    } catch (e) {
      const msg = String(e);
      if (msg.includes("WRONG_PASSWORD") || msg.toLowerCase().includes("incorrect")) {
        globalPasswordError = "Yanlış şifre, tekrar deneyin";
        globalPasswordInput = "";
        setTimeout(() => globalPasswordInputEl?.focus(), 50);
      } else {
        globalPasswordError = msg;
      }
    } finally {
      globalPasswordBusy = false;
    }
  }

  let showMounts        = false;
  let mountTargetDevice = null;
  let showCommands      = false;
  let showSettings    = false;
  let showBulkRename  = false;
  let showPlugins     = false;
  let showNetwork     = false;

  // Dual pane — sağ pane path
  let rightPanePath = "/home";

  // ── Preview panel resize ──
  let previewWidth    = parseInt(localStorage.getItem("previewWidth") || "260", 10);
  let _prevDragging   = false;
  let _prevStartX     = 0;
  let _prevStartW     = 0;

  function onPreviewResizeStart(e) {
    _prevDragging = true;
    _prevStartX   = e.clientX;
    _prevStartW   = previewWidth;
  }

  function onPreviewMouseMove(e) {
    if (!_prevDragging) return;
    const delta = _prevStartX - e.clientX;
    previewWidth = Math.max(180, Math.min(600, _prevStartW + delta));
  }

  function onPreviewMouseUp() {
    if (_prevDragging) {
      _prevDragging = false;
      localStorage.setItem("previewWidth", String(previewWidth));
    }
  }

  // ── Sidebar resize ──
  let sidebarWidth  = parseInt(localStorage.getItem("sidebarWidth") || "250", 10);
  let _sideDragging = false;
  let _sideStartX   = 0;
  let _sideStartW   = 0;

  function onSidebarResizeStart(e) {
    _sideDragging = true;
    _sideStartX   = e.clientX;
    _sideStartW   = sidebarWidth;
  }

  function onSidebarMouseMove(e) {
    if (!_sideDragging) return;
    const delta = e.clientX - _sideStartX;
    sidebarWidth = Math.max(160, Math.min(500, _sideStartW + delta));
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);
  }

  function onSidebarMouseUp() {
    if (_sideDragging) {
      _sideDragging = false;
      localStorage.setItem("sidebarWidth", String(sidebarWidth));
    }
  }

  function onWindowMouseMove(e) {
    if (_prevDragging) onPreviewMouseMove(e);
    if (_sideDragging) onSidebarMouseMove(e);
  }

  function onWindowMouseUp() {
    if (_prevDragging) onPreviewMouseUp();
    if (_sideDragging) onSidebarMouseUp();
  }

  let _unlistenDir = null;
  let _refreshTimer = null;

  onMount(() => {
    // 1. Klavye kısayollarını HEMEN en başta kaydet — hiçbir async işlem bunu geciktiremez!
    window.addEventListener("keydown", handleKey, { capture: true });

    document.documentElement.setAttribute("data-theme", $theme);
    document.documentElement.style.setProperty("--sidebar-width", `${sidebarWidth}px`);

    // Async servisleri arkada güvenle başlat
    (async () => {
      if (get(alwaysLaunchSudo)) {
        requestSudo(
          "Yönetici Modu (Admin)",
          "Uygulamayı tam root yetkisiyle başlatmak için yönetici şifrenizi girin.",
          async (pwd) => {
            await invoke("verify_sudo_password", { password: pwd });
            sessionSudoPassword.set(pwd);
            adminMode.set(true);
            notify("success", "Yönetici (Admin) Modu aktif");
          }
        ).catch(() => {});
      }

      try {
        await Promise.all([loadBookmarks(), loadDrives(), loadBlockDevices(), loadUserCommands()]);
      } catch (err) {
        console.warn("Servis yükleme uyarısı:", err);
      }

      try {
        await loadDirectory($currentPath);
      } catch (err) {
        console.warn("Dizin yükleme uyarısı:", err);
      }
    })();

    // inotify auto-refresh
    listen("dir-changed", () => {
      if (!get(autoReload)) return;
      clearTimeout(_refreshTimer);
      _refreshTimer = setTimeout(() => loadDirectory($currentPath), 400);
    }).then(u => { _unlistenDir = u; });
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handleKey, { capture: true });
    if (_unlistenDir) _unlistenDir();
    if (_refreshTimer) clearTimeout(_refreshTimer);
  });

  function handleKey(e) {
    const ctrl  = e.ctrlKey || e.metaKey;
    const shift = e.shiftKey;
    const key   = e.key;
    const code  = e.code;
    const keyCode = e.keyCode || e.which;

    invoke("debug_log", { msg: `AppKey: key='${key}', code='${code}', keyCode=${keyCode}, ctrl=${ctrl}, target=${e.target?.tagName}` }).catch(() => {});

    // Hedef gerçekten bir input veya textarea içindeyse
    const targetTag = e.target?.tagName?.toLowerCase();
    const isTargetInput = targetTag === "input" || targetTag === "textarea" || e.target?.isContentEditable;

    // Eğer odak input'ta kalmışsa ama kullanıcı başka bir yere tıkladıysa blur et
    if (!isTargetInput && document.activeElement && (document.activeElement.tagName === "INPUT" || document.activeElement.tagName === "TEXTAREA")) {
      document.activeElement.blur();
    }

    if (isTargetInput) {
      if (key === "Escape" || keyCode === 27) {
        e.target.blur();
      }
      return;
    }

    const isRight = $activePaneId === "right" && $showDualPane;

    // ── Navigation ──
    if (e.altKey && (key === "ArrowLeft" || code === "ArrowLeft" || keyCode === 37))  {
      e.preventDefault();
      if (isRight) rightPaneNavRequest.set({ type: "back" }); else navigateBack();
      return;
    }
    if (e.altKey && (key === "ArrowRight" || code === "ArrowRight" || keyCode === 39)) {
      e.preventDefault();
      if (isRight) rightPaneNavRequest.set({ type: "forward" }); else navigateForward();
      return;
    }
    if ((e.altKey && (key === "ArrowUp" || code === "ArrowUp" || keyCode === 38)) || key === "Backspace" || keyCode === 8) {
      e.preventDefault();
      if (isRight) rightPaneNavRequest.set({ type: "up" }); else navigateUp();
      return;
    }
    if (key === "F5" || code === "F5" || keyCode === 116) {
      e.preventDefault();
      if (isRight) rightPaneNavRequest.set({ type: "refresh" }); else loadDirectory($currentPath);
      return;
    }
    if (key === "F3" || code === "F3" || keyCode === 114)  { e.preventDefault(); showPreview.update(v => !v); return; }
    if (ctrl && (key === "p" || key === "P" || code === "KeyP" || keyCode === 80)) { e.preventDefault(); showDualPane.update(v => !v); return; }

    // ── View toggles ──
    if (ctrl && (key === "h" || key === "H" || code === "KeyH" || keyCode === 72)) {
      e.preventDefault();
      showHidden.update(v => !v);
      loadDirectory($currentPath);
      return;
    }
    if (ctrl && (key === "1" || code === "Digit1" || keyCode === 49)) {
      e.preventDefault();
      setFolderViewMode($currentPath, "grid");
      return;
    }
    if (ctrl && (key === "2" || code === "Digit2" || keyCode === 50)) {
      e.preventDefault();
      setFolderViewMode($currentPath, "list");
      return;
    }

    // ── Adres Çubuğu & Arama (Ctrl+L, Ctrl+F) ──
    if (ctrl && (key === "l" || key === "L" || code === "KeyL" || keyCode === 76)) {
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("arch:edit-path"));
      return;
    }
    if (ctrl && (key === "f" || key === "F" || code === "KeyF" || keyCode === 70)) {
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("arch:focus-search"));
      return;
    }

    // ── Clipboard & Undo (Tüm Linux/WebKitGTK klavye düzenleri için tam uyum) ──
    const isC = code === "KeyC" || key === "c" || key === "C" || key === "\u0003" || keyCode === 67;
    const isX = code === "KeyX" || key === "x" || key === "X" || key === "\u0018" || keyCode === 88;
    const isV = code === "KeyV" || key === "v" || key === "V" || key === "\u0016" || keyCode === 86;
    const isA = code === "KeyA" || key === "a" || key === "A" || key === "\u0001" || keyCode === 65;
    const isZ = code === "KeyZ" || key === "z" || key === "Z" || key === "\u001a" || keyCode === 90;

    if (ctrl && !shift && isC) { e.preventDefault(); copySelected(); return; }
    if (ctrl && isX)           { e.preventDefault(); cutSelected();  return; }
    if (ctrl && isV)           { e.preventDefault(); pasteClipboard(); return; }
    if (ctrl && !shift && isZ) {
      e.preventDefault();
      invoke("undo_last").then(desc => {
        notify("success", `Geri alındı: ${desc}`);
        loadDirectory($currentPath);
        rightPaneNavRequest.set({ type: "refresh" });
      }).catch(err => {
        notify("info", String(err) || "Geri alınacak işlem yok");
      });
      return;
    }

    // ── Select all ──
    if (ctrl && isA) { e.preventDefault(); selectAll(); return; }

    // ── F2: Rename ──
    const isF2 = key === "F2" || code === "F2" || keyCode === 113;
    if (isF2) {
      let path = isRight ? [...$rightSelectedPaths][0] : [...$selectedPaths][0];
      if (!path && $previewEntry?.path) path = $previewEntry.path;
      if (path) {
        e.preventDefault();
        renamingPath.set(path);
        return;
      }
    }

    // ── Delete ──
    const isDel = key === "Delete" || key === "Del" || code === "Delete" || code === "NumpadDecimal" || keyCode === 46;
    if (isDel && !shift) {
      e.preventDefault();
      let sel = isRight ? [...$rightSelectedPaths] : [...$selectedPaths];
      if (!sel.length) sel = [...$selectedPaths, ...$rightSelectedPaths];
      if (!sel.length && $previewEntry?.path) sel = [$previewEntry.path];
      if (sel.length) deleteItems(sel, false);
      return;
    }
    if (isDel && shift) {
      e.preventDefault();
      let sel = isRight ? [...$rightSelectedPaths] : [...$selectedPaths];
      if (!sel.length) sel = [...$selectedPaths, ...$rightSelectedPaths];
      if (!sel.length && $previewEntry?.path) sel = [$previewEntry.path];
      if (sel.length && confirm(`Permanently delete ${sel.length} item(s)?`)) {
        deleteItems(sel, true);
      }
      return;
    }

    // ── Enter: Seçili dosyayı/klasörü aç ──
    const isEnter = key === "Enter" || code === "Enter" || code === "NumpadEnter" || keyCode === 13;
    if (isEnter) {
      let selSet = isRight ? $rightSelectedPaths : $selectedPaths;
      if (selSet.size === 0) selSet = $selectedPaths.size > 0 ? $selectedPaths : $rightSelectedPaths;
      let path = [...selSet][0];
      if (!path && $previewEntry?.path) path = $previewEntry.path;
      if (path) {
        const entry = [...$displayEntries, ...$rightEntries].find(en => en.path === path) || $previewEntry;
        if (entry) {
          e.preventDefault();
          openEntry(entry);
          return;
        }
      }
    }

    // ── Escape: close panels / clear selection ──
    if (key === "Escape") {
      if (showBulkRename) { showBulkRename = false; return; }
      if (showPlugins)    { showPlugins = false; return; }
      if (showNetwork)    { showNetwork = false; return; }
      if (showMounts)     { showMounts = false; return; }
      if (showCommands)   { showCommands = false; return; }
      if (showSettings)   { showSettings = false; return; }
      if (isRight) rightSelectedPaths.set(new Set());
      else selectedPaths.set(new Set());
    }
  }
</script>

<svelte:body on:selectstart|preventDefault />
<svelte:window on:mousemove={onWindowMouseMove} on:mouseup={onWindowMouseUp} />

<div class="app-shell glass">
  <Titlebar
    on:mounts={()     => { showMounts = !showMounts; mountTargetDevice = null; }}
    on:network={()    => showNetwork    = !showNetwork}
    on:dual-pane={()  => showDualPane.update(v => !v)}
    on:plugins={()    => showPlugins    = !showPlugins}
    on:commands={()   => showCommands   = !showCommands}
    on:settings={()   => showSettings   = !showSettings}
  />

  <!-- Tabs — Titlebar'ın hemen altında -->
  <TabBar />

  <div class="main-area">
    <Sidebar on:open-mounts={e => { showMounts = true; mountTargetDevice = e.detail?.device ?? null; }} />
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="sidebar-drag-handle" on:mousedown={onSidebarResizeStart}></div>
    <div class="content-area">
      <Toolbar />
      <div class="grid-preview-row">
        <!-- Sol pane -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          style="display:flex;flex:1;min-width:0;overflow:hidden"
          on:mousedown={() => { activePaneId.set("left"); }}
        >
          {#if $showDevices}
            <DevicesView
              on:navigate={() => showDevices.set(false)}
              on:open-mounts={() => { showMounts = true; }}
            />
          {:else}
            <FileGrid />
          {/if}
        </div>

        <!-- Sağ pane — her zaman DOM'da, CSS ile gizle -->
        <div class="pane-splitter" class:pane-hidden={!$showDualPane}></div>
        <div class:pane-hidden={!$showDualPane} style="display:flex;flex:1;min-width:0;overflow:hidden">
          <PaneColumn
            paneId="right"
            initialPath={rightPanePath}
            isActive={$activePaneId === "right"}
            otherPath={$currentPath}
            on:focus={() => activePaneId.set("right")}
            on:switch-focus={() => activePaneId.set($activePaneId === "right" ? "left" : "right")}
            on:reload-other={() => loadDirectory($currentPath)}
            on:open-in-new-tab={e => addTab(e.detail.path)}
          />
        </div>

        <!-- Preview panel — her zaman DOM'da, CSS ile gizle (WebKit transparency ve scroll sıfırlama önlemi) -->
        <div
          class="preview-resize-wrap"
          class:pane-hidden={!$showPreview}
          style="width:{$showPreview ? previewWidth : 0}px; flex-shrink:0"
        >
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="preview-drag-handle" on:mousedown={onPreviewResizeStart}></div>
          <PreviewPanel />
        </div>
      </div>
      <StatusBar />
    </div>
  </div>

  <ContextMenu on:bulk-rename={() => showBulkRename = true} />
  <Notifications />

  {#if showMounts}
    <MountPanel targetDevice={mountTargetDevice} on:close={() => { showMounts = false; mountTargetDevice = null; loadBlockDevices(); }} />
  {/if}
  {#if showCommands}
    <CommandsPanel on:close={() => showCommands = false} />
  {/if}
  {#if showSettings}
    <SettingsPanel on:close={() => showSettings = false} />
  {/if}
  {#if showPlugins}
    <PluginManager on:close={() => showPlugins = false} />
  {/if}
  {#if showNetwork}
    <NetworkSharesPanel on:close={() => showNetwork = false} />
  {/if}
  {#if showBulkRename}
    <BulkRename on:close={() => showBulkRename = false} />
  {/if}

  <!-- ── Çakışma Yönetimi Modalı ── -->
  <FileConflictModal />

  <!-- ── Birlikte Aç (Open With) Modalı ── -->
  {#if $openWithDialog}
    <OpenWithPanel
      filePath={$openWithDialog.path}
      fileName={$openWithDialog.name}
      on:close={() => openWithDialog.set(null)}
    />
  {/if}

  <!-- ── Global Sudo Password Dialog ── -->
  {#if $sudoDialog}
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="pw-overlay" on:click|self={$sudoDialog.onCancel}>
      <div class="pw-box glass">
        <div class="pw-title">{$sudoDialog.title}</div>
        <div class="pw-desc">{$sudoDialog.description}</div>
        <input
          class="pw-input"
          type="password"
          placeholder="Sudo / Root Şifresi"
          bind:value={globalPasswordInput}
          bind:this={globalPasswordInputEl}
          on:keydown={e => e.key === "Enter" && submitGlobalPassword()}
          disabled={globalPasswordBusy}
        />
        {#if globalPasswordError}
          <div class="pw-err">{globalPasswordError}</div>
        {/if}
        <div class="pw-actions">
          <button class="btn" on:click={$sudoDialog.onCancel} disabled={globalPasswordBusy}>İptal</button>
          <button class="btn btn-accent" on:click={submitGlobalPassword} disabled={globalPasswordBusy || !globalPasswordInput}>
            {globalPasswordBusy ? "Onaylanıyor…" : "Tamam"}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(*) {
    user-select: none !important;
    -webkit-user-select: none !important;
  }
  :global(input), :global(textarea), :global(select) {
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  .app-shell {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-lg);
    overflow: hidden;
    border: 1px solid var(--border);
    box-shadow: 0 32px 64px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.04);
  }
  .main-area {
    display: flex;
    flex: 1;
    overflow: hidden;
  }
  .content-area {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }
  .grid-preview-row {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .sidebar-drag-handle {
    width: 4px;
    background: var(--border);
    flex-shrink: 0;
    cursor: col-resize;
    transition: background 0.15s;
    z-index: 2;
  }
  .sidebar-drag-handle:hover,
  .sidebar-drag-handle:active {
    background: var(--accent);
  }

  .pane-splitter {
    width: 4px;
    background: var(--border);
    flex-shrink: 0;
    cursor: col-resize;
    transition: background 0.15s;
  }
  .pane-splitter:hover { background: var(--accent); }

  /* DOM'da kal ama görünme — WebKit transparency bug önlemi */
  .pane-hidden {
    width: 0 !important;
    min-width: 0 !important;
    max-width: 0 !important;
    overflow: hidden !important;
    border: none !important;
    padding: 0 !important;
    flex: 0 0 0 !important;
    visibility: hidden;
  }

  .preview-resize-wrap {
    display: flex;
    flex-direction: row;
    flex-shrink: 0;
    overflow: hidden;
    position: relative;
  }
  .preview-drag-handle {
    width: 4px;
    flex-shrink: 0;
    cursor: col-resize;
    background: var(--border);
    transition: background 0.15s;
    z-index: 2;
  }
  .preview-drag-handle:hover,
  .preview-drag-handle:active {
    background: var(--accent);
  }

  /* ── Global Sudo Dialog ── */
  .pw-overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.55);
    display: flex; align-items: center; justify-content: center;
    z-index: 300;
  }
  .pw-box {
    width: 380px;
    border-radius: var(--radius-lg);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border: 1px solid var(--border);
    box-shadow: 0 20px 40px rgba(0,0,0,0.5);
  }
  .pw-title { font-size: 15px; font-weight: 600; color: var(--text-primary); }
  .pw-desc  { font-size: 12px; color: var(--text-dim); line-height: 1.5; }
  .pw-input {
    width: 100%;
    padding: 9px 12px;
    font-size: 13px;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    outline: none;
  }
  .pw-input:focus { border-color: var(--accent); }
  .pw-err { color: var(--danger); font-size: 12px; }
  .pw-actions { display: flex; gap: 8px; justify-content: flex-end; margin-top: 6px; }
  .pw-actions .btn { min-width: 80px; }
</style>
