<script>
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Titlebar from "./lib/components/Titlebar.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toolbar from "./lib/components/Toolbar.svelte";
  import FileGrid from "./lib/components/FileGrid.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import Notifications from "./lib/components/Notifications.svelte";
  import MountPanel from "./lib/components/MountPanel.svelte";
  import CommandsPanel from "./lib/components/CommandsPanel.svelte";
  import SettingsPanel from "./lib/components/SettingsPanel.svelte";
  import PreviewPanel from "./lib/components/PreviewPanel.svelte";

  import {
    loadDirectory, loadDrives, loadBookmarks, loadUserCommands,
    loadBlockDevices,
    navigateBack, navigateForward, navigateUp,
    currentPath, theme, showHidden, showPreview,
    copySelected, cutSelected, pasteClipboard, selectAll,
    selectedPaths, deleteItems, notify,
  } from "./lib/stores/explorer.js";

  let showMounts   = false;
  let showCommands = false;
  let showSettings = false;

  onMount(async () => {
    document.documentElement.setAttribute("data-theme", $theme);
    await Promise.all([loadBookmarks(), loadDrives(), loadBlockDevices(), loadUserCommands()]);
    await loadDirectory($currentPath);

    // inotify: dir-changed event'ini dinle, 400ms debounce ile yenile
    let refreshTimer = null;
    const unlisten = await listen("dir-changed", () => {
      clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => {
        loadDirectory($currentPath);
      }, 400);
    });

    window.addEventListener("keydown", handleKey);
    return () => {
      unlisten();
      clearTimeout(refreshTimer);
      window.removeEventListener("keydown", handleKey);
    };
  });

  function handleKey(e) {
    const ctrl  = e.ctrlKey;
    const shift = e.shiftKey;
    const key   = e.key;

    // ── Navigation ──
    if (e.altKey && key === "ArrowLeft")  { e.preventDefault(); navigateBack(); return; }
    if (e.altKey && key === "ArrowRight") { e.preventDefault(); navigateForward(); return; }
    if (e.altKey && key === "ArrowUp")    { e.preventDefault(); navigateUp(); return; }
    if (key === "F5")                     { e.preventDefault(); loadDirectory($currentPath); return; }
    if (key === "F3")                     { e.preventDefault(); showPreview.update(v => !v); return; }

    // ── View toggles ──
    if (ctrl && key === "h") { e.preventDefault(); showHidden.update(v => !v); loadDirectory($currentPath); return; }

    // ── Clipboard ──
    if (ctrl && key === "c" && !shift) { e.preventDefault(); copySelected(); return; }
    if (ctrl && key === "x")           { e.preventDefault(); cutSelected();  return; }
    if (ctrl && key === "v")           { e.preventDefault(); pasteClipboard(); return; }

    // ── Select all ──
    if (ctrl && key === "a")           { e.preventDefault(); selectAll(); return; }

    // ── Delete ──
    if (key === "Delete" && !shift) {
      e.preventDefault();
      const sel = [...$selectedPaths];
      if (sel.length) deleteItems(sel, false);
      return;
    }
    if (key === "Delete" && shift) {
      e.preventDefault();
      const sel = [...$selectedPaths];
      if (sel.length && confirm(`Permanently delete ${sel.length} item(s)?`)) {
        deleteItems(sel, true);
      }
      return;
    }

    // ── Escape: close panels / clear selection ──
    if (key === "Escape") {
      if (showMounts)   { showMounts = false; return; }
      if (showCommands) { showCommands = false; return; }
      if (showSettings) { showSettings = false; return; }
      selectedPaths.set(new Set());
    }
  }
</script>

<!-- Block text selection everywhere except inputs -->
<svelte:body on:selectstart|preventDefault />

<div class="app-shell glass">
  <Titlebar
    on:mounts={()   => showMounts   = !showMounts}
    on:commands={() => showCommands = !showCommands}
    on:settings={() => showSettings = !showSettings}
  />

  <div class="main-area">
    <Sidebar on:open-mounts={() => showMounts = true} />
    <div class="content-area">
      <Toolbar />
      <div class="grid-preview-row">
        <FileGrid />
        <PreviewPanel />
      </div>
      <StatusBar />
    </div>
  </div>

  <!-- Always mounted so context menu works everywhere -->
  <ContextMenu />
  <Notifications />

  {#if showMounts}
    <MountPanel on:close={() => showMounts = false} />
  {/if}
  {#if showCommands}
    <CommandsPanel on:close={() => showCommands = false} />
  {/if}
  {#if showSettings}
    <SettingsPanel on:close={() => showSettings = false} />
  {/if}
</div>

<style>
  /* Block text selection globally */
  :global(*) {
    user-select: none !important;
    -webkit-user-select: none !important;
  }
  :global(input), :global(textarea) {
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
</style>
