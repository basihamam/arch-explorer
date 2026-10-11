<script>
  import { invoke } from "@tauri-apps/api/core";
  import {
    displayEntries, viewMode, selectedPaths, currentPath,
    contextMenu, navigateTo, loadDirectory, notify,
    isLoading, loadError, searchQuery,
    previewEntry, showPreview,
  } from "../stores/explorer.js";

  // File type → color mapping
  const TYPE_COLORS = {
    "inode/directory":        "#8264ff",
    "application/pdf":        "#ff5f6d",
    "image/":                 "#5aff8e",
    "video/":                 "#ffd166",
    "audio/":                 "#06d6a0",
    "text/":                  "#a8daff",
    "application/zip":        "#f4a261",
    "application/x-tar":      "#f4a261",
    "application/gzip":       "#f4a261",
    "application/javascript": "#ffd166",
    "text/x-rust":            "#ff6b35",
    "application/json":       "#ffd166",
  };

  function getColor(mime) {
    for (const [key, color] of Object.entries(TYPE_COLORS)) {
      if (mime.startsWith(key)) return color;
    }
    return "#8898aa";
  }

  function getIcon(entry) {
    if (entry.is_dir) return "📁";
    const m = entry.mime_type;
    if (m.startsWith("image/"))  return "🖼";
    if (m.startsWith("video/"))  return "🎬";
    if (m.startsWith("audio/"))  return "🎵";
    if (m.includes("pdf"))       return "📄";
    if (m.includes("zip") || m.includes("tar") || m.includes("gzip")) return "🗜";
    if (m.includes("javascript") || m.includes("json")) return "⚡";
    if (m.startsWith("text/"))   return "📝";
    return "📄";
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    const units = ["B","KB","MB","GB","TB"];
    let i = 0;
    let n = bytes;
    while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${units[i]}`;
  }

  // Selection
  function toggleSelect(e, path) {
    selectedPaths.update(s => {
      const next = new Set(s);
      if (e.ctrlKey) {
        if (next.has(path)) next.delete(path); else next.add(path);
      } else if (e.shiftKey) {
        // Range select
        const paths = $displayEntries.map(e => e.path);
        const last = [...s].pop();
        if (last) {
          const a = paths.indexOf(last);
          const b = paths.indexOf(path);
          const [lo, hi] = [Math.min(a,b), Math.max(a,b)];
          paths.slice(lo, hi + 1).forEach(p => next.add(p));
        } else {
          next.add(path);
        }
      } else {
        next.clear();
        next.add(path);
      }
      return next;
    });
  }

  function onItemClick(e, entry) {
    toggleSelect(e, entry.path);
    if (!e.ctrlKey && !e.shiftKey && $showPreview) {
      previewEntry.set(entry);
    }
  }

  // ── Selection Rectangle ───────────────────────────────────────
  let fileArea;
  let selRect  = null;
  let selStart = null;
  let selecting = false;

  function onAreaMouseDown(e) {
    if (e.button !== 0) return;
    if (e.target.closest("[data-path]")) return; // dosyaya tıklama → rect başlatma
    if (e.ctrlKey || e.shiftKey) return;

    const rect = fileArea.getBoundingClientRect();
    selStart = {
      x: e.clientX - rect.left + fileArea.scrollLeft,
      y: e.clientY - rect.top  + fileArea.scrollTop,
    };
    selecting = true;
    selRect   = null;
    selectedPaths.set(new Set());
    contextMenu.set(null);

    // NOT: e.preventDefault() KALDIRILDI — WebKit'te transparency'ye neden oluyordu
    // user-select: none CSS ile text seçimi zaten engelleniyor

    window.addEventListener("mousemove", onAreaMouseMove);
    window.addEventListener("mouseup",   onAreaMouseUp, { once: true });
  }

  function onAreaMouseMove(e) {
    if (!selecting || !selStart || !fileArea) return;
    const rect = fileArea.getBoundingClientRect();
    const curX = e.clientX - rect.left + fileArea.scrollLeft;
    const curY = e.clientY - rect.top  + fileArea.scrollTop;

    selRect = {
      x: Math.min(selStart.x, curX),
      y: Math.min(selStart.y, curY),
      w: Math.abs(curX - selStart.x),
      h: Math.abs(curY - selStart.y),
    };

    if (selRect.w > 4 || selRect.h > 4) {
      const newSel = new Set();
      fileArea.querySelectorAll("[data-path]").forEach(item => {
        const ir    = item.getBoundingClientRect();
        const itemX = ir.left - rect.left + fileArea.scrollLeft;
        const itemY = ir.top  - rect.top  + fileArea.scrollTop;
        const overlaps = !(
          itemX > selRect.x + selRect.w ||
          itemX + ir.width  < selRect.x ||
          itemY > selRect.y + selRect.h ||
          itemY + ir.height < selRect.y
        );
        if (overlaps) newSel.add(item.dataset.path);
      });
      selectedPaths.set(newSel);
    }
  }

  function onAreaMouseUp() {
    selecting = false;
    selRect   = null;
    selStart  = null;
    window.removeEventListener("mousemove", onAreaMouseMove);
  }

  function onItemDblClick(entry) {
    if (entry.is_dir) {
      navigateTo(entry.path);
    } else {
      invoke("plugin:shell|open", { path: entry.path }).catch(() =>
        notify("error", `Cannot open: ${entry.name}`)
      );
    }
  }

  function onContextMenu(e, entry) {
    e.preventDefault();
    if (!$selectedPaths.has(entry.path)) {
      selectedPaths.update(s => { s.clear(); s.add(entry.path); return new Set(s); });
    }
    contextMenu.set({ x: e.clientX, y: e.clientY, target: entry });
  }

  function onBgContextMenu(e) {
    e.preventDefault();
    if (e.target === e.currentTarget) {
      selectedPaths.set(new Set());
      contextMenu.set({ x: e.clientX, y: e.clientY, target: null });
    }
  }

  function onBgClick(e) {
    if (e.target === e.currentTarget) {
      selectedPaths.set(new Set());
      contextMenu.set(null);
    }
  }

  // Drag & drop
  let dragSource = null;

  function onDragStart(e, entry) {
    dragSource = entry;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", entry.path);
  }

  async function onDrop(e, targetEntry) {
    e.preventDefault();
    if (!dragSource || !targetEntry?.is_dir) return;
    const dest = targetEntry.path + "/" + dragSource.name;
    try {
      await invoke("move_item", { src: dragSource.path, dst: dest });
      await loadDirectory($currentPath);
      notify("success", `Moved ${dragSource.name}`);
    } catch (err) {
      notify("error", String(err));
    }
    dragSource = null;
  }
</script>

<div
  class="file-area"
  bind:this={fileArea}
  class:grid-mode={$viewMode === "grid"}
  class:list-mode={$viewMode === "list"}
  on:click={onBgClick}
  on:contextmenu={onBgContextMenu}
  on:mousedown={onAreaMouseDown}
>
  <!-- Selection rectangle -->
  {#if selRect && (selRect.w > 4 || selRect.h > 4)}
    <div
      class="sel-rect"
      style="left:{selRect.x}px; top:{selRect.y}px; width:{selRect.w}px; height:{selRect.h}px;"
    ></div>
  {/if}
  {#if $isLoading}
    <div class="state-msg">
      <div class="spinner"></div>
      <span>Loading...</span>
    </div>

  {:else if $loadError}
    <div class="state-msg error">
      <span style="font-size:32px">⚠</span>
      <span>{$loadError}</span>
    </div>

  {:else if $displayEntries.length === 0}
    <div class="state-msg">
      <span style="font-size:40px">◫</span>
      <span>{$searchQuery ? "No results" : "Empty folder"}</span>
    </div>

  {:else if $viewMode === "grid"}
    <div class="grid">
      {#each $displayEntries as entry (entry.path)}
        {@const selected = $selectedPaths.has(entry.path)}
        <div
          class="grid-item"
          class:selected
          data-path={entry.path}
          on:click={e => onItemClick(e, entry)}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable="true"
          on:dragstart={e => onDragStart(e, entry)}
          on:dragover|preventDefault
          on:drop={e => onDrop(e, entry)}
          title={entry.path}
        >
          <div class="grid-icon" style="color: {getColor(entry.mime_type)}">
            {getIcon(entry)}
          </div>
          <div class="grid-name truncate">{entry.name}</div>
          {#if !entry.is_dir}
            <div class="grid-size">{formatSize(entry.size)}</div>
          {/if}
        </div>
      {/each}
    </div>

  {:else}
    <!-- List view -->
    <div class="list">
      <div class="list-header">
        <span class="col-name">Name</span>
        <span class="col-size">Size</span>
        <span class="col-date">Modified</span>
        <span class="col-perms">Permissions</span>
      </div>
      {#each $displayEntries as entry (entry.path)}
        {@const selected = $selectedPaths.has(entry.path)}
        <div
          class="list-row"
          class:selected
          data-path={entry.path}
          on:click={e => onItemClick(e, entry)}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable="true"
          on:dragstart={e => onDragStart(e, entry)}
          on:dragover|preventDefault
          on:drop={e => onDrop(e, entry)}
        >
          <span class="col-name">
            <span class="row-icon" style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
            <span class="truncate" title={entry.path}>{entry.name}</span>
            {#if entry.is_symlink}<span class="symlink-badge">⤷</span>{/if}
          </span>
          <span class="col-size text-dim">{entry.is_dir ? "—" : formatSize(entry.size)}</span>
          <span class="col-date text-dim">{entry.modified}</span>
          <span class="col-perms text-mono">{entry.permissions}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .file-area {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 8px;
    position: relative;
    user-select: none;
    -webkit-user-select: none;
  }

  /* Selection rectangle */
  .sel-rect {
    position: absolute;
    pointer-events: none;
    background: rgba(130,100,255,0.10);
    border: 1px solid rgba(130,100,255,0.45);
    border-radius: 3px;
    z-index: 50;
  }

  .state-msg {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 100%;
    color: var(--text-dim);
    font-size: 14px;
  }
  .state-msg.error { color: var(--danger); }

  /* ---- Grid ---- */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(90px, 1fr));
    gap: 4px;
  }

  .grid-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 4px 6px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    cursor: pointer;
    transition: var(--transition-fast);
    text-align: center;
    user-select: none;
  }
  .grid-item:hover {
    background: var(--bg-hover);
    border-color: var(--border);
  }
  .grid-item.selected {
    background: var(--bg-selected);
    border-color: var(--border-accent);
  }

  .grid-icon { font-size: 32px; line-height: 1; }
  .grid-name { font-size: 11px; color: var(--text-primary); width: 100%; }
  .grid-size { font-size: 10px; color: var(--text-dim); }

  /* ---- List ---- */
  .list { display: flex; flex-direction: column; }

  .list-header {
    display: grid;
    grid-template-columns: 1fr 80px 140px 100px;
    padding: 4px 8px;
    font-size: 11px;
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
    user-select: none;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .list-row {
    display: grid;
    grid-template-columns: 1fr 80px 140px 100px;
    align-items: center;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
    transition: var(--transition-fast);
    user-select: none;
  }
  .list-row:hover   { background: var(--bg-hover); }
  .list-row.selected { background: var(--bg-selected); border-color: var(--border-accent); }

  .col-name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: var(--font-size-base);
  }
  .col-size, .col-date, .col-perms {
    font-size: var(--font-size-sm);
  }
  .col-perms { font-family: var(--font-mono); font-size: 10px; }

  .row-icon { font-size: 16px; flex-shrink: 0; }
  .symlink-badge { color: var(--accent); font-size: 12px; }

  /* Spinner */
  .spinner {
    width: 28px;
    height: 28px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
