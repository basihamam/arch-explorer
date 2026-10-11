<script>
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import {
    clipboard, userCommands, notify,
    copySelected, cutSelected, pasteClipboard,
    bookmarks, addBookmark,
    showHidden, sortBy, sortAsc,
    autoReload,
    rightPanePath, rightPaneTabs, rightActiveTabId,
    rightPaneCanBack, rightPaneCanForward,
    rightPaneNavRequest, activePaneId,
    rightViewMode, rightShowHidden,
    rightDisplayCount, rightTotalSize, rightSelectedCount,
    rightSearchQuery,
    rightGridZoom, leftGridZoom,
    requestSudo,
    previewEntry, showPreview,
  } from "../stores/explorer.js";
  import { get } from "svelte/store";

  const dispatch = createEventDispatcher();

  // ── Props ──
  export let paneId       = "right";
  export let initialPath  = "/home";
  export let isActive     = false;
  export let otherPath    = "/home"; // karşı pane'in yolu (F5/F6 için)

  // ── Tab yönetimi ──
  let _tid = 100; // sol pane'den ayrı id'ler
  function mkTab(path) {
    const label = path.split("/").filter(Boolean).pop() || "/";
    return { id: ++_tid, path, label, history: [path], histIdx: 0 };
  }

  let tabs      = [mkTab(initialPath)];
  let activeTid = tabs[0].id;

  $: activeTab   = tabs.find(t => t.id === activeTid) || tabs[0];
  $: currentPath = activeTab?.path || initialPath;

  function addTab(path) {
    const t = mkTab(path || currentPath);
    tabs = [...tabs, t];
    activeTid = t.id;
    loadDir(t.path);
  }

  function closeTab(id) {
    if (tabs.length === 1) return;
    const idx  = tabs.findIndex(t => t.id === id);
    tabs       = tabs.filter(t => t.id !== id);
    if (activeTid === id) {
      activeTid = tabs[Math.max(0, idx - 1)].id;
      loadDir(currentPath);
    }
  }

  function switchTab(id) {
    activeTid = id;
    loadDir(currentPath);
  }

  function _updateTab(updater) {
    tabs = tabs.map(t => t.id === activeTid ? updater(t) : t);
  }

  // ── Local state ──
  let entries       = [];
  let isLoading     = false;
  let loadError     = null;
  let selectedPaths = new Set();
  let searchQuery   = "";
  let searchResults = [];

  // viewMode + showHidden: sağ pane ise store'dan sync — onMount'ta yapılır
  let _viewMode   = "grid";
  let _showHidden = false;

  $: displayEntries = (() => {
    const list = searchQuery.trim() ? searchResults : entries;
    return [...list].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
  })();

  $: segments = currentPath.split("/").filter(Boolean);
  $: canBack    = (activeTab?.histIdx || 0) > 0;
  $: canForward = (activeTab?.histIdx || 0) < (activeTab?.history?.length || 1) - 1;

  // ── Sağ pane ise store'lara sync ──
  $: if (paneId === "right") {
    rightPanePath.set(currentPath);
    rightPaneTabs.set(tabs);
    rightActiveTabId.set(activeTid);
    rightPaneCanBack.set(canBack);
    rightPaneCanForward.set(canForward);
    rightDisplayCount.set(displayEntries.length);
    rightTotalSize.set(displayEntries.reduce((s, e) => s + e.size, 0));
    rightSelectedCount.set(selectedPaths.size);
  }

  async function loadDir(path) {
    isLoading  = true;
    loadError  = null;
    searchQuery = "";
    try {
      const res = await invoke("list_directory", { path, showHidden: _showHidden });
      entries = res.entries;
      _updateTab(t => ({ ...t, path, label: path.split("/").filter(Boolean).pop() || "/" }));
      selectedPaths = new Set();
      invoke("start_watching", { path }).catch(() => {});
    } catch (e) {
      loadError = String(e);
    } finally {
      isLoading = false;
    }
  }

  function navigateTo(path) {
    _updateTab(t => {
      const hist = [...t.history.slice(0, t.histIdx + 1), path];
      return { ...t, history: hist, histIdx: hist.length - 1 };
    });
    loadDir(path);
    dispatch("focus", { paneId });
  }

  function navigateBack() {
    if (!canBack) return;
    _updateTab(t => ({ ...t, histIdx: t.histIdx - 1 }));
    loadDir(activeTab.history[activeTab.histIdx - 1]);
  }

  function navigateForward() {
    if (!canForward) return;
    _updateTab(t => ({ ...t, histIdx: t.histIdx + 1 }));
    loadDir(activeTab.history[activeTab.histIdx + 1]);
  }

  function navigateUp() {
    if (currentPath === "/") return;
    const parent = currentPath.split("/").slice(0, -1).join("/") || "/";
    navigateTo(parent);
  }

  // ── Selection ──
  function toggleSelect(e, path) {
    e.stopPropagation();
    const next = new Set(selectedPaths);
    if (e.ctrlKey) {
      if (next.has(path)) next.delete(path); else next.add(path);
    } else if (e.shiftKey) {
      const paths = displayEntries.map(e => e.path);
      const last  = [...selectedPaths].pop();
      if (last) {
        const a = paths.indexOf(last), b = paths.indexOf(path);
        paths.slice(Math.min(a,b), Math.max(a,b)+1).forEach(p => next.add(p));
      } else { next.add(path); }
    } else {
      next.clear(); next.add(path);
    }
    selectedPaths = next;
    dispatch("focus", { paneId });

    // Preview panel — tek seçimde entry güncelle
    if (!e.ctrlKey && !e.shiftKey) {
      showPreview.set(true);
      const entry = displayEntries.find(en => en.path === path) || null;
      previewEntry.set(entry);
    } else if (next.size === 0) {
      previewEntry.set(null);
    }
  }

  function clearSelection(e) {
    if (e?.target.closest("[data-path]")) return;
    selectedPaths = new Set();
  }

  // ── File ops ──
  function onDblClick(entry) {
    if (entry.is_dir) navigateTo(entry.path);
    else invoke("plugin:shell|open", { path: entry.path }).catch(() =>
      notify("error", `Cannot open: ${entry.name}`)
    );
  }

  // Cross-pane: F5 kopyala → karşı pane, F6 taşı → karşı pane
  async function copyToOther() {
    const sel = [...selectedPaths];
    if (!sel.length) return;
    for (const src of sel) {
      const name = src.split("/").pop();
      await invoke("copy_item", { src, dst: otherPath + "/" + name }).catch(e => notify("error", String(e)));
    }
    notify("success", `${sel.length} öğe → ${otherPath}`);
    dispatch("reload-other");
  }

  async function moveToOther() {
    const sel = [...selectedPaths];
    if (!sel.length) return;
    for (const src of sel) {
      const name = src.split("/").pop();
      await invoke("move_item", { src, dst: otherPath + "/" + name }).catch(e => notify("error", String(e)));
    }
    notify("success", `${sel.length} öğe taşındı → ${otherPath}`);
    await loadDir(currentPath);
    dispatch("reload-other");
  }

  // ── Context menu ──
  let ctxMenu = null;
  let paneRenamingPath = null;
  let paneRenameInput = "";
  let paneRenameInputEl;

  $: if (paneRenamingPath) {
    const entry = displayEntries.find(e => e.path === paneRenamingPath);
    if (entry && paneRenameInput !== entry.name) {
      paneRenameInput = entry.name;
      setTimeout(() => {
        paneRenameInputEl?.focus();
        const dotIdx = entry.name.lastIndexOf(".");
        if (dotIdx > 0 && !entry.is_dir) {
          paneRenameInputEl?.setSelectionRange(0, dotIdx);
        } else {
          paneRenameInputEl?.select();
        }
      }, 50);
    }
  }

  async function commitPaneRename(entry) {
    const newName = paneRenameInput.trim();
    paneRenamingPath = null;
    if (!newName || newName === entry.name) return;
    const newPath = entry.path.replace(/[^/]+$/, newName);
    try {
      await invoke("rename_item", { oldPath: entry.path, newPath });
      await loadDir(currentPath);
    } catch (e) { notify("error", String(e)); }
  }

  function cancelPaneRename() { paneRenamingPath = null; paneRenameInput = ""; }

  function onCtxMenu(e, entry) {
    e.preventDefault(); e.stopPropagation();
    if (!selectedPaths.has(entry?.path)) {
      selectedPaths = new Set(entry ? [entry.path] : []);
    }
    const menuW = 230;
    const menuH = 340;
    const posX = Math.min(e.clientX, window.innerWidth - menuW - 10);
    const posY = Math.min(e.clientY, window.innerHeight - menuH - 10);
    ctxMenu = { x: Math.max(10, posX), y: Math.max(10, posY), target: entry };
    dispatch("focus", { paneId });
  }

  function closeCtx() { ctxMenu = null; }

  function getUniquePaneItemName(baseName, isDir) {
    const existingNames = new Set((entries || []).map(e => e.name));
    if (!existingNames.has(baseName)) return baseName;
    let count = 2;
    let dotIdx = isDir ? -1 : baseName.lastIndexOf(".");
    while (true) {
      let candidate;
      if (dotIdx > 0) {
        const pre = baseName.slice(0, dotIdx);
        const ext = baseName.slice(dotIdx);
        candidate = `${pre} (${count})${ext}`;
      } else {
        candidate = `${baseName} (${count})`;
      }
      if (!existingNames.has(candidate)) return candidate;
      count++;
    }
  }

  async function ctxRename() {
    if (!ctxMenu?.target) return;
    const target = ctxMenu.target;
    closeCtx();
    selectedPaths = new Set([target.path]);
    paneRenamingPath = target.path;
  }

  async function ctxDelete(permanent) {
    const paths = [...selectedPaths].length ? [...selectedPaths] : [ctxMenu?.target?.path].filter(Boolean);
    if (!paths.length) return;
    if (permanent && !confirm(`Permanently delete ${paths.length} item(s)?`)) return;
    for (const p of paths) await invoke("delete_item", { path: p, permanent }).catch(() => {});
    notify("success", `${paths.length} item(s) deleted`);
    await loadDir(currentPath);
    closeCtx();
  }

  async function ctxNewFolder() {
    const base = "New Folder";
    const name = getUniquePaneItemName(base, true);
    const targetDir = ctxMenu?.target?.is_dir ? ctxMenu.target.path : currentPath;
    const newPath = targetDir.replace(/\/+$/, "") + "/" + name;
    closeCtx();
    try {
      await invoke("create_dir", { path: newPath });
      await loadDir(currentPath);
      selectedPaths = new Set([newPath]);
      paneRenamingPath = newPath;
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function ctxNewFile() {
    const base = "New File";
    const name = getUniquePaneItemName(base, false);
    const targetDir = ctxMenu?.target?.is_dir ? ctxMenu.target.path : currentPath;
    const newPath = targetDir.replace(/\/+$/, "") + "/" + name;
    closeCtx();
    try {
      await invoke("create_file", { path: newPath });
      await loadDir(currentPath);
      selectedPaths = new Set([newPath]);
      paneRenamingPath = newPath;
    } catch (e) {
      notify("error", String(e));
    }
  }

  // ── Drag & drop ──
  let dragSrc = null;

  async function onDrop(e, targetEntry) {
    e.preventDefault();
    if (!dragSrc || !targetEntry?.is_dir || dragSrc.path === targetEntry.path) return;
    const dst = targetEntry.path + "/" + dragSrc.name;
    await invoke("move_item", { src: dragSrc.path, dst }).catch(e => notify("error", String(e)));
    await loadDir(currentPath);
    notify("success", `${dragSrc.name} → ${targetEntry.name}`);
    dragSrc = null;
  }

  // ── inotify ──
  let _unlisten;
  let _refreshTimer;

  onMount(() => {
    loadDir(initialPath);

    // viewMode + showHidden store'larını dinle — sadece bir kere, reactive block değil
    let unsubVM, unsubSH;
    if (paneId === "right") {
      _viewMode   = get(rightViewMode);
      _showHidden = get(rightShowHidden);
      unsubVM = rightViewMode.subscribe(v => { _viewMode = v; });
      unsubSH = rightShowHidden.subscribe(v => { _showHidden = v; });
    }

    // navRequest subscribe — cycle önlemi için burada
    let navUnsub;
    if (paneId === "right") {
      navUnsub = rightPaneNavRequest.subscribe(req => {
        if (!req) return;
        rightPaneNavRequest.set(null);
        if (req.type === "navigate" && req.path)    navigateTo(req.path);
        if (req.type === "back")                    navigateBack();
        if (req.type === "forward")                 navigateForward();
        if (req.type === "up")                      navigateUp();
        if (req.type === "refresh")                 loadDir(activeTab?.path || initialPath);
        if (req.type === "new-tab")                 addTab(req.path);
        if (req.type === "close-tab"  && req.tabId) closeTab(req.tabId);
        // Bug 3 fix: tab switch → activeTid güncelle SONRA loadDir çağır
        if (req.type === "switch-tab" && req.tabId) {
          activeTid = req.tabId;
          const tab = tabs.find(t => t.id === req.tabId);
          if (tab) loadDir(tab.path);
        }
        if (req.type === "set-view-mode" && req.mode) { _viewMode = req.mode; rightViewMode.set(req.mode); }
        if (req.type === "toggle-hidden") { _showHidden = !_showHidden; rightShowHidden.set(_showHidden); loadDir(activeTab?.path || initialPath); }
      });
    }

    listen("dir-changed", () => {
      if (!get(autoReload)) return;
      clearTimeout(_refreshTimer);
      _refreshTimer = setTimeout(() => loadDir(activeTab?.path || initialPath), 400);
    }).then(u => _unlisten = u);

    return () => {
      navUnsub?.();
      unsubVM?.();
      unsubSH?.();
    };
  });

  onDestroy(() => {
    _unlisten?.();
    clearTimeout(_refreshTimer);
  });

  // Keyboard
  function onKeyDown(e) {
    if (!isActive) return;
    const tag = document.activeElement?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea") return;
    if (e.altKey && e.key === "ArrowLeft")  { e.preventDefault(); navigateBack(); }
    if (e.altKey && e.key === "ArrowRight") { e.preventDefault(); navigateForward(); }
    if (e.altKey && e.key === "ArrowUp")    { e.preventDefault(); navigateUp(); }
    if (e.key === "F5" && !e.shiftKey)      { e.preventDefault(); copyToOther(); }
    if (e.key === "F6")                     { e.preventDefault(); moveToOther(); }
    if (e.key === "F7")                     { e.preventDefault(); ctxNewFolder(); }
    if (e.ctrlKey && e.key === "a")         { e.preventDefault(); selectedPaths = new Set(displayEntries.map(e => e.path)); }
    if (e.key === "Tab")                    { e.preventDefault(); dispatch("switch-focus"); }
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    const u = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  }

  const TYPE_COLORS = {
    "inode/directory": "#8264ff", "image/": "#5aff8e",
    "video/": "#ffd166", "audio/": "#06d6a0",
    "text/": "#a8daff", "application/pdf": "#ff5f6d",
  };

  function getColor(mime) {
    for (const [k, v] of Object.entries(TYPE_COLORS)) if (mime?.startsWith(k)) return v;
    return "#8898aa";
  }

  function getIcon(e) {
    if (e.is_dir) return "📁";
    const m = e.mime_type;
    if (m?.startsWith("image/")) return "🖼";
    if (m?.startsWith("video/")) return "🎬";
    if (m?.startsWith("audio/")) return "🎵";
    if (m?.includes("pdf"))      return "📄";
    if (m?.startsWith("text/"))  return "📝";
    return "📄";
  }
  let paneFileArea;

  function _onPaneWheel(e) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const step = e.deltaY > 0 ? -10 : 10;
    rightGridZoom.update(v => Math.max(60, Math.min(220, v + step)));
  }

  onMount(() => {
    const handleOutside = (e) => {
      if (ctxMenu) {
        closeCtx();
      }
    };
    window.addEventListener("pointerdown", handleOutside, { capture: true });
    window.addEventListener("contextmenu", handleOutside, { capture: true });

    if (paneFileArea) {
      paneFileArea.addEventListener("wheel", _onPaneWheel, { passive: false });
    }

    return () => {
      window.removeEventListener("pointerdown", handleOutside, { capture: true });
      window.removeEventListener("contextmenu", handleOutside, { capture: true });
      if (paneFileArea) paneFileArea.removeEventListener("wheel", _onPaneWheel);
    };
  });
</script>

<svelte:window on:keydown={onKeyDown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="pane-column"
  class:active={isActive}
  on:mousedown={() => dispatch("focus", { paneId })}
>
  <!-- ── File area ── -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="pane-file-area"
    bind:this={paneFileArea}
    on:click={clearSelection}
    on:contextmenu={e => onCtxMenu(e, null)}
  >
    {#if isLoading}
      <div class="pane-state"><div class="pane-spinner"></div></div>
    {:else if loadError}
      <div class="pane-state error">⚠ {loadError}</div>
    {:else if displayEntries.length === 0}
      <div class="pane-state">◫ Boş</div>
    {:else if _viewMode === "grid"}
      <div class="pane-grid" style="grid-template-columns: repeat(auto-fill, minmax({$rightGridZoom}px, 1fr))">
        {#each displayEntries as entry (entry.path)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="pane-grid-item"
            class:selected={selectedPaths.has(entry.path)}
            data-path={entry.path}
            on:click={e => toggleSelect(e, entry.path)}
            on:dblclick={() => onDblClick(entry)}
            on:contextmenu={e => onCtxMenu(e, entry)}
            draggable="true"
            on:dragstart={e => { dragSrc = entry; e.dataTransfer.setData("text/plain", entry.path); }}
            on:dragover|preventDefault
            on:drop={e => onDrop(e, entry)}
          >
            <div class="pane-grid-icon" style="color:{getColor(entry.mime_type)}; font-size:{Math.max(20, Math.min($rightGridZoom * 0.38, 48))}px">{getIcon(entry)}</div>
            {#if paneRenamingPath === entry.path}
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <input
                class="pane-rename-input"
                bind:value={paneRenameInput}
                bind:this={paneRenameInputEl}
                on:keydown={e => {
                  if (e.key === "Enter")  { e.preventDefault(); commitPaneRename(entry); }
                  if (e.key === "Escape") { e.preventDefault(); cancelPaneRename(); }
                  e.stopPropagation();
                }}
                on:click|stopPropagation
                on:blur={() => commitPaneRename(entry)}
              />
            {:else}
              <div class="pane-grid-name">{entry.name}</div>
            {/if}
            {#if !entry.is_dir && entry.size > 0}
              <div class="pane-grid-size">{formatSize(entry.size)}</div>
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="pane-list">
        <div class="pane-list-header">
          <span class="pane-col-name">Ad</span>
          <span class="pane-col-size">Boyut</span>
          <span class="pane-col-date">Değiştirildi</span>
        </div>
        {#each displayEntries as entry (entry.path)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="pane-list-row"
            class:selected={selectedPaths.has(entry.path)}
            data-path={entry.path}
            on:click={e => toggleSelect(e, entry.path)}
            on:dblclick={() => onDblClick(entry)}
            on:contextmenu={e => onCtxMenu(e, entry)}
            draggable="true"
            on:dragstart={e => { dragSrc = entry; e.dataTransfer.setData("text/plain", entry.path); }}
            on:dragover|preventDefault
            on:drop={e => onDrop(e, entry)}
          >
            <span class="pane-col-name">
              <span style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
              {#if paneRenamingPath === entry.path}
                <!-- svelte-ignore a11y-no-static-element-interactions -->
                <input
                  class="pane-rename-input list"
                  bind:value={paneRenameInput}
                  bind:this={paneRenameInputEl}
                  on:keydown={e => {
                    if (e.key === "Enter")  { e.preventDefault(); commitPaneRename(entry); }
                    if (e.key === "Escape") { e.preventDefault(); cancelPaneRename(); }
                    e.stopPropagation();
                  }}
                  on:click|stopPropagation
                  on:blur={() => commitPaneRename(entry)}
                />
              {:else}
                <span class="pane-name-text" title={entry.path}>{entry.name}</span>
              {/if}
            </span>
            <span class="pane-col-size">{entry.is_dir ? "—" : formatSize(entry.size)}</span>
            <span class="pane-col-date">{entry.modified}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ── Context menu ── -->
  {#if ctxMenu}
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="pane-ctx"
      style="left:{ctxMenu.x}px; top:{ctxMenu.y}px"
      on:click|stopPropagation
      on:contextmenu|preventDefault|stopPropagation
    >
      {#if ctxMenu.target}
        <div class="pane-ctx-label">{ctxMenu.target.name}</div>
        <div class="pane-ctx-sep"></div>
        <button class="pane-ctx-item" on:click={ctxRename}>
          <span class="pane-ctx-icon">✏</span>
          <span class="pane-ctx-title">Rename</span>
          <span class="pane-ctx-sc">F2</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxNewFolder}>
          <span class="pane-ctx-icon">📁</span>
          <span class="pane-ctx-title">New Folder</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxNewFile}>
          <span class="pane-ctx-icon">📄</span>
          <span class="pane-ctx-title">New File</span>
        </button>
        {#if ctxMenu.target.is_dir}
          <button class="pane-ctx-item" on:click={() => { navigateTo(ctxMenu.target.path); closeCtx(); }}>
            <span class="pane-ctx-icon">▶</span>
            <span class="pane-ctx-title">Open</span>
          </button>
        {/if}
        <button class="pane-ctx-item" on:click={() => { navigator.clipboard.writeText(ctxMenu.target.path); notify("success","Path copied"); closeCtx(); }}>
          <span class="pane-ctx-icon">◈</span>
          <span class="pane-ctx-title">Copy Path</span>
        </button>
        <div class="pane-ctx-sep"></div>
        <button class="pane-ctx-item" on:click={copyToOther}>
          <span class="pane-ctx-icon">⎘</span>
          <span class="pane-ctx-title">Copy to Other Pane</span>
          <span class="pane-ctx-sc">F5</span>
        </button>
        <button class="pane-ctx-item" on:click={moveToOther}>
          <span class="pane-ctx-icon">→</span>
          <span class="pane-ctx-title">Move to Other Pane</span>
          <span class="pane-ctx-sc">F6</span>
        </button>
        <div class="pane-ctx-sep"></div>
        <button class="pane-ctx-item" on:click={() => ctxDelete(false)}>
          <span class="pane-ctx-icon">🗑</span>
          <span class="pane-ctx-title">Trash</span>
          <span class="pane-ctx-sc">Del</span>
        </button>
        <button class="pane-ctx-item danger" on:click={() => ctxDelete(true)}>
          <span class="pane-ctx-icon">✕</span>
          <span class="pane-ctx-title">Delete</span>
          <span class="pane-ctx-sc">Shift+Del</span>
        </button>
      {:else}
        <button class="pane-ctx-item" on:click={ctxNewFolder}>
          <span class="pane-ctx-icon">📁</span>
          <span class="pane-ctx-title">New Folder</span>
          <span class="pane-ctx-sc">F7</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxNewFile}>
          <span class="pane-ctx-icon">📄</span>
          <span class="pane-ctx-title">New File</span>
        </button>
        <div class="pane-ctx-sep"></div>
        <button class="pane-ctx-item" on:click={() => { dispatch("open-in-new-tab", { path: currentPath }); closeCtx(); }}>
          <span class="pane-ctx-icon">⊞</span>
          <span class="pane-ctx-title">Open in Left Tab</span>
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .pane-column {
    display: flex; flex-direction: column;
    flex: 1; min-width: 0;
    border-left: 1px solid var(--border);
    background: var(--bg-primary);
    position: relative;
    user-select: none;
  }
  .pane-column.active { border-left-color: var(--accent); }

  /* File area */
  .pane-file-area { flex: 1; overflow-y: auto; overflow-x: hidden; padding: 6px; position: relative; }
  .pane-state { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-dim); font-size: 12px; gap: 10px; flex-direction: column; }
  .pane-state.error { color: var(--danger); }
  .pane-spinner { width: 22px; height: 22px; border: 2px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Grid */
  .pane-grid { display: grid; gap: 3px; }
  .pane-grid-item { display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 6px 3px 5px; border-radius: var(--radius-sm); border: 1px solid transparent; cursor: pointer; text-align: center; user-select: none; }
  .pane-grid-item:hover { background: var(--bg-hover); border-color: var(--border); }
  .pane-grid-item.selected { background: var(--bg-selected); border-color: var(--border-accent); }
  .pane-grid-icon { line-height: 1; }
  .pane-grid-name { font-size: 10px; color: var(--text-primary); width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pane-grid-size { font-size: 9px; color: var(--text-dim); }

  /* List */
  .pane-list { display: flex; flex-direction: column; }
  .pane-list-header { display: grid; grid-template-columns: 1fr 70px 120px; padding: 3px 6px; font-size: 10px; color: var(--text-dim); border-bottom: 1px solid var(--border); position: sticky; top: 0; background: var(--bg-secondary); }
  .pane-list-row { display: grid; grid-template-columns: 1fr 70px 120px; align-items: center; padding: 3px 6px; border-radius: var(--radius-sm); border: 1px solid transparent; cursor: pointer; }
  .pane-list-row:hover { background: var(--bg-hover); }
  .pane-list-row.selected { background: var(--bg-selected); border-color: var(--border-accent); }
  .pane-col-name { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: 11px; }
  .pane-col-size, .pane-col-date { font-size: 10px; color: var(--text-secondary); }
  .pane-name-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* Context menu */
  .pane-ctx {
    position: fixed; z-index: 9999;
    min-width: 220px;
    border-radius: var(--radius);
    padding: 5px;
    box-shadow: 0 12px 36px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.08);
    background: var(--bg-panel, rgba(20, 20, 28, 0.94));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    user-select: none;
  }
  .pane-ctx-label { padding: 4px 10px; font-size: 10px; color: var(--text-dim); font-weight: 600; text-transform: uppercase; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pane-ctx-sep { height: 1px; background: var(--border, rgba(255,255,255,0.07)); margin: 4px 0; }
  .pane-ctx-item {
    display: flex; align-items: center; gap: 9px; width: 100%; padding: 6px 10px;
    border-radius: var(--radius-sm); border: none; background: none; color: var(--text-primary);
    font-size: 12px; cursor: pointer; text-align: left;
  }
  .pane-ctx-item:hover { background: var(--bg-hover); }
  .pane-ctx-item.danger { color: var(--danger); }
  .pane-ctx-item.danger:hover { background: var(--danger-soft, rgba(239,68,68,0.12)); }
  .pane-ctx-icon { font-size: 13px; width: 18px; text-align: center; display: inline-flex; align-items: center; justify-content: center; }
  .pane-ctx-title { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pane-ctx-sc { margin-left: auto; font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); }

  /* Rename inputs */
  .pane-rename-input {
    width: 90%;
    font-size: 11px;
    text-align: center;
    background: var(--bg-input, #1c1d27);
    border: 1px solid var(--accent, #6366f1);
    color: var(--text-primary, #fff);
    border-radius: 4px;
    padding: 2px 4px;
    outline: none;
    box-shadow: 0 0 0 2px rgba(99,102,241,0.25);
  }
  .pane-rename-input.list {
    text-align: left;
    width: 220px;
  }
</style>
