<script>
  import { invoke } from "@tauri-apps/api/core";
  import {
    displayEntries, viewMode, selectedPaths, currentPath,
    contextMenu, navigateTo, loadDirectory, notify,
    isLoading, loadError, searchQuery,
    copySelected, cutSelected, pasteClipboard, deleteItems,
    previewEntry, showPreview,
  } from "../stores/explorer.js";

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
    const ext = entry.extension?.toLowerCase();
    const extIcons = {
      rs: "🦀", py: "🐍", lua: "🌙", sh: "💲",
      md: "📖", toml: "⚙", json: "⚙", yaml: "⚙",
      js: "⚡", ts: "⚡", dart: "🎯", go: "🐹",
      c: "©", cpp: "©", h: "©",
    };
    if (extIcons[ext]) return extIcons[ext];
    const m = entry.mime_type;
    if (m.startsWith("image/"))  return "🖼";
    if (m.startsWith("video/"))  return "🎬";
    if (m.startsWith("audio/"))  return "🎵";
    if (m.includes("pdf"))       return "📄";
    if (m.includes("zip") || m.includes("tar") || m.includes("gzip")) return "🗜";
    if (m.startsWith("text/"))   return "📝";
    return "📄";
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    const units = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${units[i]}`;
  }

  // ── Selection ─────────────────────────────────────────────────────
  function toggleSelect(e, path) {
    e.stopPropagation();
    selectedPaths.update(s => {
      const next = new Set(s);
      if (e.ctrlKey) {
        if (next.has(path)) next.delete(path); else next.add(path);
      } else if (e.shiftKey) {
        const paths = $displayEntries.map(e => e.path);
        const last  = [...s].pop();
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

    // Tek dosya seçiliyse preview'ı güncelle
    if (!e.ctrlKey && !e.shiftKey && $showPreview) {
      const entry = $displayEntries.find(e => e.path === path) || null;
      previewEntry.set(entry);
    }
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
    e.stopPropagation();
    if (!$selectedPaths.has(entry.path)) {
      selectedPaths.update(() => new Set([entry.path]));
    }
    contextMenu.set({ x: e.clientX, y: e.clientY, target: entry });
  }

  function onBgContextMenu(e) {
    e.preventDefault();
    selectedPaths.set(new Set());
    contextMenu.set({ x: e.clientX, y: e.clientY, target: null });
  }

  function onBgClick() {
    selectedPaths.set(new Set());
    contextMenu.set(null);
  }

  // ── Drag & Drop (file items) ──────────────────────────────────────
  let dragSource = null;

  function onDragStart(e, entry) {
    dragSource = entry;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", entry.path);
  }

  async function onDropIntoDir(e, targetEntry) {
    e.preventDefault();
    e.stopPropagation();
    if (!dragSource || !targetEntry?.is_dir) return;
    if (dragSource.path === targetEntry.path) return;
    const dest = targetEntry.path + "/" + dragSource.name;
    try {
      await invoke("move_item", { src: dragSource.path, dst: dest });
      await loadDirectory($currentPath);
      notify("success", `Moved ${dragSource.name} → ${targetEntry.name}`);
    } catch (err) {
      notify("error", String(err));
    }
    dragSource = null;
  }

  // ── Selection Rectangle ───────────────────────────────────────────
  let fileArea;
  let selRect   = null;
  let selStart  = null;
  let selecting = false;

  function onAreaMouseDown(e) {
    if (e.button !== 0) return;
    // Only start rect if clicking on empty area (not a file item)
    if (e.target.closest("[data-path]")) return;
    if (e.ctrlKey || e.shiftKey) return;

    const rect = fileArea.getBoundingClientRect();
    selStart  = {
      x: e.clientX - rect.left + fileArea.scrollLeft,
      y: e.clientY - rect.top  + fileArea.scrollTop,
    };
    selecting = true;
    selRect   = null;

    if (!e.ctrlKey) selectedPaths.set(new Set());
    contextMenu.set(null);

    window.addEventListener("mousemove", onAreaMouseMove);
    window.addEventListener("mouseup",   onAreaMouseUp, { once: true });
    e.preventDefault();
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
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="file-area"
  bind:this={fileArea}
  on:click|self={onBgClick}
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
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="grid-item"
          class:selected
          data-path={entry.path}
          on:click={e => toggleSelect(e, entry.path)}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable="true"
          on:dragstart={e => onDragStart(e, entry)}
          on:dragover|preventDefault
          on:drop={e => onDropIntoDir(e, entry)}
          title={entry.path}
        >
          <div class="grid-icon" style="color:{getColor(entry.mime_type)}">
            {getIcon(entry)}
          </div>
          <div class="grid-name">{entry.name}</div>
          {#if !entry.is_dir && entry.size > 0}
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
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="list-row"
          class:selected
          data-path={entry.path}
          on:click={e => toggleSelect(e, entry.path)}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable="true"
          on:dragstart={e => onDragStart(e, entry)}
          on:dragover|preventDefault
          on:drop={e => onDropIntoDir(e, entry)}
        >
          <span class="col-name">
            <span class="row-icon" style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
            <span class="name-text" title={entry.path}>{entry.name}</span>
            {#if entry.is_symlink}<span class="symlink-badge">⤷</span>{/if}
          </span>
          <span class="col-size">{entry.is_dir ? "—" : formatSize(entry.size)}</span>
          <span class="col-date">{entry.modified}</span>
          <span class="col-perms">{entry.permissions}</span>
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

  /* ── Selection Rect ── */
  .sel-rect {
    position: absolute;
    pointer-events: none;
    background: rgba(130,100,255,0.12);
    border: 1px solid rgba(130,100,255,0.5);
    border-radius: 4px;
    z-index: 50;
  }

  /* ── Grid ── */
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
  .grid-item:hover   { background: var(--bg-hover); border-color: var(--border); }
  .grid-item.selected { background: var(--bg-selected); border-color: var(--border-accent); }

  .grid-icon { font-size: 32px; line-height: 1; }
  .grid-name { font-size: 11px; color: var(--text-primary); width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grid-size { font-size: 10px; color: var(--text-dim); }

  /* ── List ── */
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
  .list-row:hover    { background: var(--bg-hover); }
  .list-row.selected { background: var(--bg-selected); border-color: var(--border-accent); }

  .col-name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: var(--font-size-base);
  }
  .col-size, .col-date { font-size: var(--font-size-sm); color: var(--text-secondary); }
  .col-perms { font-size: 10px; color: var(--text-secondary); font-family: var(--font-mono); }
  .row-icon { font-size: 16px; flex-shrink: 0; }
  .name-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .symlink-badge { color: var(--accent); font-size: 12px; }

  .spinner {
    width: 28px; height: 28px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
