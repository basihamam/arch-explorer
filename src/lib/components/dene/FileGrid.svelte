<script>
  import { invoke } from "@tauri-apps/api/core";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import {
    displayEntries, viewMode, selectedPaths, currentPath,
    contextMenu, navigateTo, loadDirectory, notify,
    isLoading, loadError, searchQuery,
    copySelected, cutSelected, pasteClipboard, deleteItems,
    previewEntry, showPreview,
    gitStatus, renamingPath,
    gridZoom, thumbnailMode, thumbnailMinSize,
  } from "../stores/explorer.js";

  // ── Git badge renkleri ──
  const GIT_COLORS = { M: "#ffd166", A: "#5aff8e", D: "#ff5f6d", "?": "#8898aa", R: "#06d6a0" };
  const GIT_LABELS = { M: "M", A: "A", D: "D", "?": "?", R: "R" };

  function gitBadge(entry) {
    const files = $gitStatus?.files || {};
    return files[entry.path] || files[entry.name] || null;
  }

  // ── Inline rename ──
  let renameInput    = "";
  let renameInputEl;

  // $renamingPath dışarıdan (örn. ContextMenu'den yeni dosya/klasör oluşturulunca) set edilirse
  $: if ($renamingPath) {
    const entry = $displayEntries.find(e => e.path === $renamingPath);
    if (entry && renameInput !== entry.name) {
      renameInput = entry.name;
      setTimeout(() => {
        renameInputEl?.focus();
        const dotIdx = entry.name.lastIndexOf(".");
        if (dotIdx > 0 && !entry.is_dir) {
          renameInputEl?.setSelectionRange(0, dotIdx);
        } else {
          renameInputEl?.select();
        }
      }, 50);
    }
  }

  function startRename(entry) {
    renamingPath.set(entry.path);
    renameInput = entry.name;
    // input'a focus — DOM güncellemesi sonrası
    setTimeout(() => {
      renameInputEl?.focus();
      // Uzantı hariç seç
      const dotIdx = entry.name.lastIndexOf(".");
      if (dotIdx > 0 && !entry.is_dir) {
        renameInputEl?.setSelectionRange(0, dotIdx);
      } else {
        renameInputEl?.select();
      }
    }, 30);
  }

  async function commitRename(entry) {
    const newName = renameInput.trim();
    renamingPath.set(null);
    if (!newName || newName === entry.name) return;
    const newPath = entry.path.replace(/[^/]+$/, newName);
    try {
      await invoke("rename_item", { oldPath: entry.path, newPath });
      await loadDirectory($currentPath);
    } catch (e) { notify("error", String(e)); }
  }

  function cancelRename() { renamingPath.set(null); renameInput = ""; }

  // ── Thumbnail ── (yeni tanımlar aşağıda currentThumbSize bloğunda)

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
    // Rename aktifse önce iptal et
    if ($renamingPath && $renamingPath !== path) cancelRename();
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
    if ($renamingPath === entry.path) return; // rename modunda çift tık engelle
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

  // ── Thumbnail tipler ──
  const IMAGE_EXTS   = new Set(["jpg","jpeg","png","webp","bmp","tiff","tif","avif","ico"]);
  const DIRECT_EXTS  = new Set(["gif","svg"]); // Rust'a gönderme, tarayıcı handle eder

  function isThumbable(entry) {
    if (entry.is_dir) return false;
    const ext = (entry.extension || "").toLowerCase();
    return IMAGE_EXTS.has(ext) || DIRECT_EXTS.has(ext) || entry.mime_type?.startsWith("image/");
  }

  function isDirectThumb(entry) {
    // GIF: animasyon korunur | SVG: tarayıcı render eder
    const ext = (entry.extension || "").toLowerCase();
    return DIRECT_EXTS.has(ext);
  }

  // Zoom → thumbnail boyut tieri
  // Zoom → thumbnail boyut tieri (yüksek kalite tier'ları)
  function thumbSizeForZoom(zoom) {
    if (zoom < 85) return 64;
    if (zoom < 135) return 128;
    if (zoom < 185) return 256;
    return 384;
  }

  $: currentThumbSize = thumbSizeForZoom($gridZoom);

  // ── Thumbnail cache ──
  let thumbCache   = {};
  let thumbLoading = new Set();
  let _thumbGen    = 0;
  let _lastSize    = 0;
  let _lastPath2   = "";
  let _zoomDebounce = null;

  function getCachedThumb(entry) {
    // 1. Önce tam boyutu kontrol et
    const exact = thumbCache[`${entry.path}_${currentThumbSize}`];
    if (exact) return exact;
    // 2. Henüz yüksek kalite yüklenmediyse, eldeki en iyi çözünürlüğü göster (asla kaybolmasın)
    return thumbCache[`${entry.path}_384`] ||
           thumbCache[`${entry.path}_256`] ||
           thumbCache[`${entry.path}_128`] ||
           thumbCache[`${entry.path}_64`] ||
           null;
  }

  async function loadThumbnailsAtSize(entries, size) {
    const myGen = _thumbGen;
    const imgs = entries.filter(e =>
      isThumbable(e) && !isDirectThumb(e) &&
      !thumbCache[`${e.path}_${size}`] &&
      !thumbLoading.has(`${e.path}_${size}`)
    );
    const BATCH = 4;
    for (let i = 0; i < imgs.length; i += BATCH) {
      if (_thumbGen !== myGen) return;
      const batch = imgs.slice(i, i + BATCH);
      await Promise.all(batch.map(async e => {
        if (_thumbGen !== myGen) return;
        const key = `${e.path}_${size}`;
        thumbLoading.add(key);
        try {
          const dataUrl = await invoke("get_thumbnail", { path: e.path, maxSize: size });
          if (_thumbGen === myGen && dataUrl) {
            thumbCache[key] = dataUrl;
          }
        } catch {
          if (_thumbGen === myGen) thumbCache[key] = null;
        } finally { thumbLoading.delete(key); }
      }));
      if (_thumbGen !== myGen) return;
      thumbCache = { ...thumbCache };
      await new Promise(r => setTimeout(r, 0));
    }
  }

  $: if ($currentPath !== _lastPath2) {
    _lastPath2 = $currentPath;
    _thumbGen++;
    thumbCache = {};
    thumbLoading = new Set();
    loadThumbnailsAtSize($displayEntries, currentThumbSize);
  }

  $: if (currentThumbSize !== _lastSize) {
    _lastSize = currentThumbSize;
    clearTimeout(_zoomDebounce);
    _zoomDebounce = setTimeout(() => {
      _thumbGen++;
      loadThumbnailsAtSize($displayEntries, currentThumbSize);
    }, 150);
  }

  // Ctrl+scroll — non-passive wheel listener (Svelte passive sorununu aşar)
  function _onWheel(e) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const step = e.deltaY > 0 ? -10 : 10;
    gridZoom.update(v => Math.max(60, Math.min(220, v + step)));
  }

  onMount(() => {
    if (fileArea) {
      fileArea.addEventListener("wheel", _onWheel, { passive: false });
    }
  });

  onDestroy(() => {
    clearTimeout(_zoomDebounce);
    if (fileArea) fileArea.removeEventListener("wheel", _onWheel);
  });

  // ── Thumbnail görünürlük ──
  function showThumb(entry) {
    if (!isThumbable(entry)) return false;
    if ($thumbnailMode === "always")  return true;
    if ($thumbnailMode === "auto")    return $gridZoom >= 100;
    if ($thumbnailMode === "custom")  return $gridZoom >= $thumbnailMinSize;
    return false;
  }

  // ── Ctrl + scroll → zoom ──
  // Non-passive listener onMount'ta ekleniyor (yukarıda)

  // F2 → seçili tek dosyayı inline rename'e al
  function onKeyDown(e) {
    if (e.key === "F2" && $selectedPaths.size === 1) {
      e.preventDefault();
      const path  = [...$selectedPaths][0];
      const entry = $displayEntries.find(en => en.path === path);
      if (entry) startRename(entry);
    }
  }
</script>

<svelte:window on:keydown={onKeyDown} />

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
    <div class="grid" style="grid-template-columns: repeat(auto-fill, minmax({$gridZoom}px, 1fr))">
      {#each $displayEntries as entry (entry.path)}
        {@const selected = $selectedPaths.has(entry.path)}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="grid-item"
          class:selected
          class:renaming={$renamingPath === entry.path}
          data-path={entry.path}
          on:click={e => toggleSelect(e, entry.path)}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable={$renamingPath !== entry.path}
          on:dragstart={e => onDragStart(e, entry)}
          on:dragover|preventDefault
          on:drop={e => onDropIntoDir(e, entry)}
          title={entry.path}
        >
          <!-- Thumbnail veya emoji ikon -->
          <div class="grid-icon" style="color:{getColor(entry.mime_type)}">
            {#if showThumb(entry)}
              {#if isDirectThumb(entry)}
                <!-- GIF: animasyonlu | SVG: tarayıcı render -->
                <img
                  class="grid-thumb"
                  src={convertFileSrc(entry.path)}
                  alt={entry.name}
                  loading="lazy"
                  style="width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px; max-width:100%; object-fit:contain"
                  on:error={e => e.target.style.display = "none"}
                />
              {:else if getCachedThumb(entry)}
                <!-- JPEG/PNG/vb. → Rust cache -->
                <img
                  class="grid-thumb"
                  src={getCachedThumb(entry)}
                  alt={entry.name}
                  style="width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px; max-width:100%; object-fit:contain"
                />
              {:else}
                <!-- Yükleniyor — emoji göster -->
                <span style="font-size:{Math.max(20, Math.min($gridZoom * 0.38, 48))}px">{getIcon(entry)}</span>
              {/if}
            {:else}
              <span style="font-size:{Math.max(20, Math.min($gridZoom * 0.38, 48))}px">{getIcon(entry)}</span>
            {/if}
          </div>

          <!-- Git badge -->
          {#if gitBadge(entry)}
            <span class="git-badge" style="background:{GIT_COLORS[gitBadge(entry)] || '#8898aa'}">
              {GIT_LABELS[gitBadge(entry)] || gitBadge(entry)}
            </span>
          {/if}

          <!-- İsim veya inline rename input -->
          {#if $renamingPath === entry.path}
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <input
              class="rename-input"
              bind:value={renameInput}
              bind:this={renameInputEl}
              on:keydown={e => {
                if (e.key === "Enter")  { e.preventDefault(); commitRename(entry); }
                if (e.key === "Escape") { e.preventDefault(); cancelRename(); }
                e.stopPropagation();
              }}
              on:click|stopPropagation
              on:blur={() => commitRename(entry)}
            />
          {:else}
            <div class="grid-name">{entry.name}</div>
          {/if}

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
            {#if showThumb(entry)}
              {#if isDirectThumb(entry)}
                <img class="list-thumb" src={convertFileSrc(entry.path)} alt={entry.name} loading="lazy" on:error={e => e.target.style.display="none"} />
              {:else if getCachedThumb(entry)}
                <img class="list-thumb" src={getCachedThumb(entry)} alt={entry.name} />
              {:else}
                <span class="row-icon" style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
              {/if}
            {:else}
              <span class="row-icon" style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
            {/if}
            {#if $renamingPath === entry.path}
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <input
                class="rename-input-list"
                bind:value={renameInput}
                bind:this={renameInputEl}
                on:keydown={e => {
                  if (e.key === "Enter")  { e.preventDefault(); commitRename(entry); }
                  if (e.key === "Escape") { e.preventDefault(); cancelRename(); }
                  e.stopPropagation();
                }}
                on:click|stopPropagation
                on:blur={() => commitRename(entry)}
              />
            {:else}
              <span class="name-text" title={entry.path}>{entry.name}</span>
            {/if}
            {#if entry.is_symlink}<span class="symlink-badge">⤷</span>{/if}
            {#if gitBadge(entry)}
              <span class="git-badge-list" style="color:{GIT_COLORS[gitBadge(entry)] || '#8898aa'}">
                {GIT_LABELS[gitBadge(entry)]}
              </span>
            {/if}
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
    position: relative;
  }
  .grid-item:hover   { background: var(--bg-hover); border-color: var(--border); }
  .grid-item.selected { background: var(--bg-selected); border-color: var(--border-accent); }

  .grid-icon { font-size: 32px; line-height: 1; position: relative; }

  /* Thumbnail */
  .grid-thumb {
    width: 52px; height: 52px;
    object-fit: cover;
    border-radius: 4px;
    display: block;
  }
  .grid-thumb-fallback { font-size: 32px; }

  /* Git badge — grid */
  .git-badge {
    position: absolute;
    top: 2px; right: 2px;
    font-size: 8px; font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    color: #000;
    line-height: 1.4;
    pointer-events: none;
  }

  /* Git badge — list */
  .git-badge-list {
    font-size: 10px; font-weight: 700;
    margin-left: 4px;
    flex-shrink: 0;
  }

  /* Renaming state */
  .grid-item.renaming { border-color: var(--border-accent) !important; }

  /* Inline rename — grid */
  .rename-input {
    width: 100%;
    font-size: 11px;
    padding: 2px 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-accent);
    border-radius: 3px;
    color: var(--text-primary);
    text-align: center;
    outline: none;
    user-select: text !important;
    -webkit-user-select: text !important;
  }

  /* Inline rename — list */
  .rename-input-list {
    flex: 1;
    font-size: var(--font-size-base);
    padding: 1px 5px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-accent);
    border-radius: 3px;
    color: var(--text-primary);
    outline: none;
    min-width: 0;
    user-select: text !important;
    -webkit-user-select: text !important;
  }
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

  /* List view thumbnail */
  .list-thumb {
    width: 20px;
    height: 20px;
    object-fit: cover;
    border-radius: 3px;
    flex-shrink: 0;
  }
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
