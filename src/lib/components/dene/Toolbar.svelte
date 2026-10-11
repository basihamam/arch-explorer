<script>
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    currentPath, pathHistory, historyIndex,
    showHidden, viewMode, searchQuery, searchFiles,
    navigateTo, navigateBack, navigateForward, navigateUp,
    loadDirectory, notify, showPreview,
    activePaneId,
    rightPaneNavRequest, rightPaneCanBack, rightPaneCanForward,
    activeToolbarPath, activeViewMode, activeShowHidden,
  } from "../stores/explorer.js";

  const win = getCurrentWindow();

  let editingPath  = false;
  let pathInput    = $activeToolbarPath;
  let searchInput  = "";
  let searchTimeout;

  // pathInput her zaman aktif pane'in yolunu yansıtır
  $: pathInput = $activeToolbarPath;
  $: canBack    = $activePaneId === "right" ? $rightPaneCanBack    : $historyIndex > 0;
  $: canForward = $activePaneId === "right" ? $rightPaneCanForward : $historyIndex < $pathHistory.length - 1;

  // Breadcrumb segmentleri — aktif pane'e göre
  $: segments = $activeToolbarPath.split("/").filter(Boolean);

  function _right(type, extra = {}) { rightPaneNavRequest.set({ type, ...extra }); }

  function onPathSubmit() {
    editingPath = false;
    if (!pathInput || pathInput === $activeToolbarPath) return;
    if ($activePaneId === "right") _right("navigate", { path: pathInput });
    else navigateTo(pathInput);
  }

  function doBack()    { if ($activePaneId === "right") _right("back");    else navigateBack(); }
  function doForward() { if ($activePaneId === "right") _right("forward"); else navigateForward(); }
  function doUp()      { if ($activePaneId === "right") _right("up");      else navigateUp(); }
  function doRefresh() { if ($activePaneId === "right") _right("refresh"); else loadDirectory($currentPath); }

  function goToSegment(i) {
    const path = "/" + segments.slice(0, i + 1).join("/");
    if ($activePaneId === "right") _right("navigate", { path });
    else navigateTo(path);
  }

  function onSearchInput(e) {
    searchInput = e.target.value;
    searchQuery.set(searchInput);
    clearTimeout(searchTimeout);
    if (searchInput.trim()) {
      searchTimeout = setTimeout(() => searchFiles(searchInput), 280);
    } else {
      searchFiles("");
    }
  }

  function clearSearch() {
    searchInput = "";
    searchQuery.set("");
    searchFiles("");
  }

  function setActiveViewMode(mode) {
    if ($activePaneId === "right") _right("set-view-mode", { mode });
    else viewMode.set(mode);
  }

  function toggleActiveHidden() {
    if ($activePaneId === "right") _right("toggle-hidden");
    else { showHidden.update(v => !v); loadDirectory($currentPath); }
  }
  let dragOverSeg = null;

  async function onSegmentDrop(e, segIndex) {
    e.preventDefault();
    dragOverSeg = null;
    const srcPath = e.dataTransfer.getData("text/plain");
    if (!srcPath) return;
    const targetDir = "/" + segments.slice(0, segIndex + 1).join("/");
    const name      = srcPath.split("/").pop();
    const dstPath   = targetDir + "/" + name;
    if (srcPath === dstPath) return;
    try {
      await invoke("move_item", { src: srcPath, dst: dstPath });
      notify("success", `Moved ${name} → .../${segments[segIndex]}`);
      await loadDirectory($currentPath);
    } catch (err) {
      notify("error", String(err));
    }
  }
</script>

<div class="toolbar">
  <!-- Nav buttons -->
  <div class="nav-group">
    <button class="nav-btn" disabled={!canBack}    on:click={doBack}    title="Back (Alt+←)">‹</button>
    <button class="nav-btn" disabled={!canForward} on:click={doForward} title="Forward (Alt+→)">›</button>
    <button class="nav-btn" on:click={doUp}                             title="Up (Alt+↑)">↑</button>
    <button class="nav-btn" on:click={doRefresh}                        title="Refresh (F5)">↻</button>
  </div>

  <!-- Path bar / breadcrumb -->
  <div class="path-bar">
    {#if editingPath}
      <input
        class="path-input"
        bind:value={pathInput}
        on:blur={onPathSubmit}
        on:keydown={e => {
          if (e.key === "Enter")  onPathSubmit();
          if (e.key === "Escape") { editingPath = false; pathInput = $activeToolbarPath; }
        }}
        autofocus
      />
    {:else}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="breadcrumb"
        on:dblclick={() => editingPath = true}
        title="Double-click to edit path"
      >
        <!-- Root -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <span
          class="crumb root"
          on:click={() => { if ($activePaneId === "right") _right("navigate", { path: "/" }); else navigateTo("/"); }}
          on:dragover|preventDefault
          on:drop={e => {
            const src = e.dataTransfer.getData("text/plain");
            if (src) invoke("move_item", { src, dst: "/" + src.split("/").pop() })
              .then(() => loadDirectory($currentPath))
              .catch(err => notify("error", String(err)));
          }}
        >◈</span>

        {#each segments as seg, i}
          <span class="crumb-sep">/</span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span
            class="crumb"
            class:last={i === segments.length - 1}
            class:drag-over={dragOverSeg === i}
            on:click={() => goToSegment(i)}
            on:dragover|preventDefault={() => dragOverSeg = i}
            on:dragleave={() => dragOverSeg = null}
            on:drop={e => onSegmentDrop(e, i)}
          >{seg}</span>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Search -->
  <div class="search-wrap">
    <svg class="search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
      <circle cx="11" cy="11" r="8"/><path d="M21 21l-4.35-4.35"/>
    </svg>
    <input
      class="search-input"
      placeholder="Search..."
      value={searchInput}
      on:input={onSearchInput}
    />
    {#if searchInput}
      <button class="clear-btn" on:click={clearSearch}>✕</button>
    {/if}
  </div>

  <!-- View toggle -->
  <div class="view-group">
    <button class="view-btn" class:active={$activeViewMode === "grid"}
      on:click={() => setActiveViewMode("grid")} title="Grid view">▦</button>
    <button class="view-btn" class:active={$activeViewMode === "list"}
      on:click={() => setActiveViewMode("list")} title="List view">☰</button>
    <div class="divider"></div>
    <button class="view-btn" class:active={$activeShowHidden}
      on:click={toggleActiveHidden}
      title="Toggle hidden files">👁</button>
    <div class="divider"></div>
    <button class="view-btn" class:active={$showPreview}
      on:click={() => showPreview.update(v => !v)}
      title="Preview panel (F3)">◫</button>
  </div>
</div>

<style>
  .toolbar {
    height: 42px;
    display: flex;
    align-items: center;
    padding: 0 8px;
    gap: 6px;
    border-bottom: 1px solid var(--border);
    border-top: none; border-left: none; border-right: none;
    border-radius: 0;
    flex-shrink: 0;
  }

  .nav-group, .view-group { display: flex; align-items: center; gap: 2px; }

  .nav-btn {
    width: 28px; height: 28px;
    border-radius: var(--radius-sm);
    border: none; background: transparent;
    color: var(--text-secondary);
    font-size: 16px; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    transition: var(--transition-fast);
    user-select: none;
  }
  .nav-btn:hover:not(:disabled) { background: var(--bg-hover); color: var(--text-primary); }
  .nav-btn:disabled { opacity: 0.3; cursor: default; }

  .path-bar { flex: 1; min-width: 0; height: 30px; }

  .breadcrumb {
    display: flex;
    align-items: center;
    height: 100%;
    padding: 0 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    gap: 2px;
    cursor: default;
  }

  .crumb {
    font-size: 11px;
    color: var(--text-secondary);
    padding: 1px 5px;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
    transition: var(--transition-fast);
    user-select: none;
  }
  .crumb:hover    { background: var(--bg-hover); color: var(--text-primary); }
  .crumb.last     { color: var(--text-primary); font-weight: 500; }
  .crumb.root     { color: var(--accent); font-size: 15px; }
  .crumb.drag-over {
    background: var(--accent-dim);
    border: 1px dashed var(--accent);
    color: var(--accent);
  }
  .crumb-sep { color: var(--text-dim); font-size: 11px; user-select: none; }

  .path-input {
    width: 100%; height: 100%;
    background: var(--bg-tertiary);
    border: 1px solid var(--border-accent);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 0 8px;
  }

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 200px;
  }
  .search-icon { position: absolute; left: 8px; color: var(--text-dim); pointer-events: none; }
  .search-input {
    width: 100%; height: 28px;
    padding: 0 28px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 11px;
  }
  .search-input:focus { border-color: var(--border-accent); }
  .search-input::placeholder { color: var(--text-dim); }
  .clear-btn {
    position: absolute; right: 6px;
    background: none; border: none;
    color: var(--text-dim); cursor: pointer;
    font-size: 10px; padding: 2px;
  }
  .clear-btn:hover { color: var(--text-primary); }

  .view-btn {
    width: 28px; height: 28px;
    border-radius: var(--radius-sm);
    border: none; background: transparent;
    color: var(--text-dim); cursor: pointer;
    font-size: 14px;
    display: flex; align-items: center; justify-content: center;
    transition: var(--transition-fast);
    user-select: none;
  }
  .view-btn:hover  { background: var(--bg-hover); color: var(--text-primary); }
  .view-btn.active { background: var(--accent-dim); color: var(--accent); }

  .divider { width: 1px; height: 18px; background: var(--border); margin: 0 2px; }
</style>
