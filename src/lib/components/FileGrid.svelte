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
    gridZoom, thumbnailMode, thumbnailMinSize, videoHoverScrubbing,
    requestSudo, activeTabId, saveTabScroll, openEntry,
    safeBatchFileOp, rightPaneNavRequest, activePaneId, showDualPane,
    sortBy, sortAsc, toggleSort,
  } from "../stores/explorer.js";
  import { t } from "../i18n.js";

  // ── List View Column Widths & Resizing ──
  let colWidths = { size: 90, date: 140, perms: 110 };
  try {
    const saved = localStorage.getItem("listColWidths");
    if (saved) colWidths = { ...colWidths, ...JSON.parse(saved) };
  } catch {}

  $: listGridStyle = `grid-template-columns: minmax(130px, 1fr) ${colWidths.size}px ${colWidths.date}px ${colWidths.perms}px;`;

  let resizingCol = null;
  let resizeStartX = 0;
  let resizeStartW = 0;

  function onColResizeStart(e, colKey) {
    e.preventDefault();
    e.stopPropagation();
    resizingCol = colKey;
    resizeStartX = e.clientX;
    resizeStartW = colWidths[colKey] || 100;
    window.addEventListener("mousemove", onColResizeMove);
    window.addEventListener("mouseup", onColResizeEnd);
  }

  function onColResizeMove(e) {
    if (!resizingCol) return;
    const delta = e.clientX - resizeStartX;
    const limits = { size: [45, 300], date: [60, 360], perms: [50, 280] };
    const [minW, maxW] = limits[resizingCol] || [50, 300];
    colWidths[resizingCol] = Math.max(minW, Math.min(maxW, resizeStartW + delta));
    colWidths = { ...colWidths };
  }

  function onColResizeEnd() {
    if (resizingCol) {
      localStorage.setItem("listColWidths", JSON.stringify(colWidths));
      resizingCol = null;
    }
    window.removeEventListener("mousemove", onColResizeMove);
    window.removeEventListener("mouseup", onColResizeEnd);
  }

  function formatFolderSize(entry) {
    if (!entry.is_dir) return formatSize(entry.size);
    if (entry.size > 0) return formatSize(entry.size);
    if (entry.children_count !== undefined && entry.children_count !== null) {
      return `${entry.children_count} öğe`;
    }
    return "0 öğe";
  }

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
  let _lastRenamingPath = null;
  let isCommittingRename = false;

  // $renamingPath değiştiğinde input'a başlangıç değerini ata ve focus yap
  $: if ($renamingPath && $renamingPath !== _lastRenamingPath) {
    _lastRenamingPath = $renamingPath;
    const entry = $displayEntries.find(e => e.path === $renamingPath);
    if (entry) {
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
  } else if (!$renamingPath) {
    _lastRenamingPath = null;
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
    if (isCommittingRename) return;
    const targetEntry = entry;
    const newName = renameInput.trim();
    isCommittingRename = true;
    renamingPath.set(null);

    if (!newName || !targetEntry || newName === targetEntry.name) {
      isCommittingRename = false;
      return;
    }

    const cleanOldPath = targetEntry.path.replace(/\/+$/, "");
    const lastSlash = cleanOldPath.lastIndexOf("/");
    const parentDir = lastSlash >= 0 ? cleanOldPath.slice(0, lastSlash) : "";
    const newPath = (parentDir === "" ? "" : parentDir) + "/" + newName;

    try {
      await invoke("tracked_rename", { oldPath: targetEntry.path, newPath });
      await loadDirectory($currentPath);
    } catch (e) {
      const errStr = String(e).toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("os error 13") || errStr.includes("yetki") || errStr.includes("access denied")) {
        try {
          await requestSudo(
            "Yeniden Adlandırma (Sudo)",
            `'${targetEntry.name}' öğesini '${newName}' olarak adlandırmak için yönetici yetkisi gereklidir.`,
            async (pwd) => {
              await invoke("sudo_rename_item", { oldPath: targetEntry.path, newPath, password: pwd });
            }
          );
          await loadDirectory($currentPath);
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(e));
      }
    } finally {
      isCommittingRename = false;
    }
  }

  function cancelRename() {
    isCommittingRename = false;
    renamingPath.set(null);
    renameInput = "";
  }

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
      exe: "🪟", msi: "🪟", bat: "📜", cmd: "📜", dll: "🧩",
      rs: "🦀", py: "🐍", lua: "🌙", sh: "💲",
      md: "📖", toml: "⚙", json: "⚙", yaml: "⚙",
      js: "⚡", ts: "⚡", dart: "🎯", go: "🐹",
      c: "©", cpp: "©", h: "©",
    };
    if (extIcons[ext]) return extIcons[ext];
    const m = entry.mime_type || "";
    if (m.includes("x-msdownload") || m.includes("x-msdos-program") || m.includes("x-dosexec")) return "🪟";
    if (m.startsWith("image/"))  return "🖼";
    if (m.startsWith("video/"))  return "🎬";
    if (m.startsWith("audio/"))  return "🎵";
    if (m.includes("pdf"))       return "📄";
    if (m.includes("zip") || m.includes("tar") || m.includes("gzip") || m.includes("7z") || m.includes("rar")) return "🗜";
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
  let selectionAnchor = null;

  function toggleSelect(e, path) {
    // Rename aktifse önce iptal et
    if ($renamingPath && $renamingPath !== path) cancelRename();
    if (document.activeElement && document.activeElement.tagName === "INPUT" && $renamingPath !== path) {
      document.activeElement.blur();
    }
    activePaneId.set("left");
    e.stopPropagation();

    const paths = $displayEntries.map(en => en.path);
    const targetIdx = paths.indexOf(path);

    selectedPaths.update(s => {
      const next = new Set(s);

      if (e.shiftKey) {
        // Shift ile aralık seçimi: Anchor'dan tıklanan öğeye kadar olan tüm öğeleri seç
        const anchorIdx = selectionAnchor ? paths.indexOf(selectionAnchor) : -1;
        const start = anchorIdx !== -1 ? anchorIdx : (targetIdx !== -1 ? targetIdx : 0);
        const [lo, hi] = [Math.min(start, targetIdx), Math.max(start, targetIdx)];
        if (!e.ctrlKey) next.clear();
        for (let i = lo; i <= hi; i++) {
          if (paths[i]) next.add(paths[i]);
        }
      } else if (e.ctrlKey) {
        // Ctrl ile tekli ekleme/çıkarma
        if (next.has(path)) {
          next.delete(path);
        } else {
          next.add(path);
        }
        selectionAnchor = path;
      } else {
        // Normal sol tık: Sadece bu öğeyi seç
        next.clear();
        next.add(path);
        selectionAnchor = path;
      }

      return next;
    });

    // Preview panel — son tıklanan veya tek seçili entry'i güncelle
    const clickedEntry = $displayEntries.find(en => en.path === path) || null;
    if (clickedEntry) {
      previewEntry.set(clickedEntry);
    }

    fileArea?.focus();
  }

  function onItemDblClick(entry) {
    if ($renamingPath === entry.path) return; // rename modunda çift tık engelle
    openEntry(entry);
  }

  function onContextMenu(e, entry) {
    e.preventDefault();
    e.stopPropagation();
    activePaneId.set("left");
    if (!$selectedPaths.has(entry.path)) {
      selectedPaths.update(() => new Set([entry.path]));
    }
    contextMenu.set({ x: e.clientX, y: e.clientY, target: entry });
  }

  function onBgContextMenu(e) {
    e.preventDefault();
    activePaneId.set("left");
    selectedPaths.set(new Set());
    contextMenu.set({ x: e.clientX, y: e.clientY, target: null });
  }

  function onBgClick() {
    activePaneId.set("left");
    selectedPaths.set(new Set());
    contextMenu.set(null);
    fileArea?.focus();
  }

  // ── Drag & Drop (file items) ──────────────────────────────────────
  let dragSource = null;
  let dragOverPath = null;

  function setCustomDragImage(e, entry, count) {
    const ghost = document.createElement("div");
    ghost.style.position = "absolute";
    ghost.style.top = "-9999px";
    ghost.style.left = "-9999px";
    ghost.style.display = "flex";
    ghost.style.alignItems = "center";
    ghost.style.gap = "8px";
    ghost.style.padding = "6px 12px";
    ghost.style.background = "rgba(24, 24, 37, 0.95)";
    ghost.style.border = "1px solid rgba(130, 100, 255, 0.8)";
    ghost.style.borderRadius = "8px";
    ghost.style.boxShadow = "0 8px 24px rgba(0, 0, 0, 0.6)";
    ghost.style.color = "#ffffff";
    ghost.style.fontSize = "12px";
    ghost.style.fontWeight = "600";
    ghost.style.pointerEvents = "none";
    ghost.style.zIndex = "99999";

    const iconSpan = document.createElement("span");
    iconSpan.style.fontSize = "18px";
    iconSpan.textContent = getIcon(entry);
    ghost.appendChild(iconSpan);

    const nameSpan = document.createElement("span");
    nameSpan.textContent = entry.name;
    nameSpan.style.maxWidth = "220px";
    nameSpan.style.overflow = "hidden";
    nameSpan.style.textOverflow = "ellipsis";
    nameSpan.style.whiteSpace = "nowrap";
    ghost.appendChild(nameSpan);

    if (count > 1) {
      const badge = document.createElement("span");
      badge.style.background = "#8264ff";
      badge.style.color = "#ffffff";
      badge.style.borderRadius = "10px";
      badge.style.padding = "1px 7px";
      badge.style.fontSize = "11px";
      badge.style.fontWeight = "700";
      badge.textContent = `+${count - 1}`;
      ghost.appendChild(badge);
    }

    document.body.appendChild(ghost);
    e.dataTransfer.setDragImage(ghost, 20, 20);
    setTimeout(() => {
      if (ghost.parentNode) ghost.parentNode.removeChild(ghost);
    }, 0);
  }

  function onDragStart(e, entry) {
    dragSource = entry;
    e.dataTransfer.effectAllowed = "move";
    const sel = [...$selectedPaths];
    const paths = sel.includes(entry.path) && sel.length > 1 ? sel : [entry.path];
    e.dataTransfer.setData("text/plain", JSON.stringify(paths));
    try {
      setCustomDragImage(e, entry, paths.length);
    } catch {}
  }

  async function onDropIntoDir(e, targetEntry) {
    e.preventDefault();
    e.stopPropagation();
    if (!targetEntry?.is_dir) return;
    let paths = [];
    try {
      paths = JSON.parse(e.dataTransfer.getData("text/plain"));
    } catch {
      const raw = e.dataTransfer.getData("text/plain");
      if (raw) paths = [raw];
    }
    if (!paths.length && dragSource) paths = [dragSource.path];
    paths = paths.filter(p => p !== targetEntry.path);
    if (!paths.length) return;
    dragSource = null;
    await safeBatchFileOp(paths, targetEntry.path, "move");
    await loadDirectory($currentPath);
    rightPaneNavRequest.set({ type: "refresh" });
  }

  async function onDropIntoBg(e) {
    e.preventDefault();
    e.stopPropagation();
    let paths = [];
    try {
      paths = JSON.parse(e.dataTransfer.getData("text/plain"));
    } catch {
      const raw = e.dataTransfer.getData("text/plain");
      if (raw) paths = [raw];
    }
    if (!paths.length && dragSource) paths = [dragSource.path];
    if (!paths.length) return;
    dragSource = null;
    await safeBatchFileOp(paths, $currentPath, "move");
    await loadDirectory($currentPath);
    rightPaneNavRequest.set({ type: "refresh" });
  }

  // ── Selection Rectangle ───────────────────────────────────────────
  let fileArea;
  let selRect   = null;
  let selStart  = null;
  let selecting = false;
  let hasDragged = false;
  let lastClientX = 0;
  let lastClientY = 0;

  let pendingClickPath = null;

  function onAreaMouseDown(e) {
    if (e.button !== 0) return;
    activePaneId.set("left");
    if (document.activeElement && (document.activeElement.tagName === "INPUT" || document.activeElement.tagName === "TEXTAREA")) {
      document.activeElement.blur();
    }
    // Only return if clicking directly on interactive elements (file name, icon, resizers, inputs, buttons)
    if (e.target.closest(".grid-icon, .grid-name, .col-name, .col-resizer, input, button")) return;

    if (e.ctrlKey || e.shiftKey) {
      const row = e.target.closest("[data-path]");
      if (row?.dataset?.path) {
        toggleSelect(e, row.dataset.path);
      }
      return;
    }

    const rect = fileArea.getBoundingClientRect();
    lastClientX = e.clientX;
    lastClientY = e.clientY;
    selStart  = {
      x: e.clientX - rect.left + fileArea.scrollLeft,
      y: e.clientY - rect.top  + fileArea.scrollTop,
    };
    selecting = true;
    hasDragged = false;
    selRect   = null;

    // Eğer boş alana tıklanıyorsa veya item dış kenarına tıklanıyorsa
    pendingClickPath = null;

    window.addEventListener("mousemove", onAreaMouseMove);
    window.addEventListener("mouseup",   onAreaMouseUp, { once: true });
    e.preventDefault();
  }

  let autoScrollRaf = null;

  function updateAutoScroll() {
    if (!selecting || !fileArea) {
      if (autoScrollRaf) {
        cancelAnimationFrame(autoScrollRaf);
        autoScrollRaf = null;
      }
      return;
    }

    const rect = fileArea.getBoundingClientRect();
    const edgeThreshold = 40;
    const maxSpeed = 16;
    let scrollDelta = 0;

    if (lastClientY < rect.top + edgeThreshold) {
      // Yukarı doğru yaklaştıkça hızlanarak scroll et
      const intensity = Math.max(0, Math.min(1, (rect.top + edgeThreshold - lastClientY) / edgeThreshold));
      scrollDelta = -Math.round(intensity * maxSpeed);
    } else if (lastClientY > rect.bottom - edgeThreshold) {
      // Aşağı doğru yaklaştıkça hızlanarak scroll et
      const intensity = Math.max(0, Math.min(1, (lastClientY - (rect.bottom - edgeThreshold)) / edgeThreshold));
      scrollDelta = Math.round(intensity * maxSpeed);
    }

    if (scrollDelta !== 0) {
      fileArea.scrollTop += scrollDelta;
      recalculateSelection(lastClientY);
    }

    autoScrollRaf = requestAnimationFrame(updateAutoScroll);
  }

  function recalculateSelection(clientY) {
    if (!selecting || !selStart || !fileArea) return;
    const rect = fileArea.getBoundingClientRect();
    const curX = lastClientX - rect.left + fileArea.scrollLeft;
    const curY = clientY - rect.top + fileArea.scrollTop;

    selRect = {
      x: Math.min(selStart.x, curX),
      y: Math.min(selStart.y, curY),
      w: Math.abs(curX - selStart.x),
      h: Math.abs(curY - selStart.y),
    };

    if (selRect.w > 4 || selRect.h > 4) {
      hasDragged = true;
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

  function onAreaMouseMove(e) {
    if (!selecting || !selStart || !fileArea) return;
    lastClientX = e.clientX;
    lastClientY = e.clientY;

    if (!autoScrollRaf) {
      autoScrollRaf = requestAnimationFrame(updateAutoScroll);
    }

    recalculateSelection(e.clientY);
  }

  function onAreaMouseUp(e) {
    if (!selecting) return;
    selecting = false;
    if (autoScrollRaf) {
      cancelAnimationFrame(autoScrollRaf);
      autoScrollRaf = null;
    }
    window.removeEventListener("mousemove", onAreaMouseMove);

    // Eğer sürükleme olmadıysa (saf boşluk tıklaması ise) seçimi temizle
    if (!hasDragged && (!selRect || (selRect.w <= 4 && selRect.h <= 4))) {
      selectedPaths.set(new Set());
      contextMenu.set(null);
    }
    // Sürükleme ile seçim yapıldıysa (hasDragged === true), seçilenler KORUNUR!
    selRect   = null;
    selStart  = null;
    hasDragged = false;
  }

  // ── Thumbnail tipler ──
  const IMAGE_EXTS   = new Set(["jpg","jpeg","png","webp","bmp","tiff","tif","avif","ico"]);
  const VIDEO_EXTS   = new Set(["mp4","mkv","webm","avi","mov","wmv","flv","m4v","ts","mpg","mpeg","3gp","ogv"]);
  const DIRECT_EXTS  = new Set(["gif","svg"]); // Rust'a gönderme, tarayıcı handle eder

  function isThumbable(entry) {
    if (!entry || entry.is_dir) return false;
    const ext = (entry.extension || "").toLowerCase();
    if (entry.path?.startsWith("adb://")) {
      return IMAGE_EXTS.has(ext);
    }
    return IMAGE_EXTS.has(ext) || VIDEO_EXTS.has(ext) || DIRECT_EXTS.has(ext) || entry.mime_type?.startsWith("image/") || entry.mime_type?.startsWith("video/");
  }

  function isDirectThumb(entry) {
    if (!entry || entry.path?.startsWith("adb://")) return false;
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

  // ── High Performance Virtual Batching & RAM Optimization ──
  let renderLimit = 150;
  const CHUNK_SIZE = 100;

  $: renderedEntries = ($displayEntries && $displayEntries.length > renderLimit)
    ? $displayEntries.slice(0, renderLimit)
    : ($displayEntries || []);

  function checkRenderMore() {
    if (!fileArea) return;
    const scrollBottom = fileArea.scrollHeight - fileArea.scrollTop - fileArea.clientHeight;
    if (scrollBottom < 600 && renderLimit < ($displayEntries?.length || 0)) {
      renderLimit = Math.min($displayEntries.length, renderLimit + CHUNK_SIZE);
      if ($displayEntries && $displayEntries.length > 0) {
        loadThumbnailsAtSize($displayEntries, currentThumbSize);
      }
    }
  }

  // ── Thumbnail cache with LRU memory cap ──
  let thumbCache   = {};
  let thumbLoading = new Set();
  let _thumbGen    = 0;
  let _lastSize    = 0;
  let _lastPath2   = "";
  let _zoomDebounce = null;
  const MAX_THUMB_CACHE = 150;

  function putCacheThumb(key, val) {
    const keys = Object.keys(thumbCache);
    if (keys.length >= MAX_THUMB_CACHE) {
      for (let i = 0; i < 40; i++) {
        delete thumbCache[keys[i]];
      }
    }
    thumbCache[key] = val;
  }

  function getCachedThumb(entry, cache = thumbCache) {
    if (!cache) return null;
    // 1. Önce tam boyutu kontrol et
    const exact = cache[`${entry.path}_${currentThumbSize}`];
    if (exact) return exact;
    // 2. Henüz yüksek kalite yüklenmediyse, eldeki en iyi çözünürlüğü göster (asla kaybolmasın)
    return cache[`${entry.path}_384`] ||
           cache[`${entry.path}_256`] ||
           cache[`${entry.path}_128`] ||
           cache[`${entry.path}_64`] ||
           null;
  }

  // ── Video Hover Scrubbing ──
  let activeScrubbing = {}; // path -> { url: string, pct: number }
  let scrubDebounce = null;

  function isVideoEntry(entry) {
    const ext = (entry.extension || "").toLowerCase();
    return VIDEO_EXTS.has(ext) || entry.mime_type?.startsWith("video/");
  }

  let lastScrubPct = {};
  function onVideoMouseMove(e, entry) {
    if (!$videoHoverScrubbing || !isVideoEntry(entry)) return;
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));

    const prevPct = lastScrubPct[entry.path] ?? -1;
    if (Math.abs(pct - prevPct) < 0.015 && activeScrubbing[entry.path]) return;
    lastScrubPct[entry.path] = pct;

    const existingUrl = activeScrubbing[entry.path]?.url || getCachedThumb(entry, thumbCache);
    activeScrubbing[entry.path] = { url: existingUrl, pct };
    activeScrubbing = { ...activeScrubbing };

    clearTimeout(scrubDebounce);
    scrubDebounce = setTimeout(async () => {
      try {
        const dataUrl = await invoke("get_video_hover_frame", {
          path: entry.path,
          percent: pct,
          maxSize: currentThumbSize,
        });
        if (dataUrl && activeScrubbing[entry.path]) {
          activeScrubbing[entry.path] = { url: dataUrl, pct };
          activeScrubbing = { ...activeScrubbing };
        }
      } catch {}
    }, 45);
  }

  function onVideoMouseLeave(entry) {
    delete lastScrubPct[entry.path];
    if (activeScrubbing[entry.path]) {
      delete activeScrubbing[entry.path];
      activeScrubbing = { ...activeScrubbing };
    }
  }

  async function loadThumbnailsAtSize(entries, size) {
    const myGen = _thumbGen;
    const targetSlice = entries.slice(0, Math.min(entries.length, renderLimit + 40));
    const imgs = targetSlice.filter(e =>
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
            putCacheThumb(key, dataUrl);
          }
        } catch {
          if (_thumbGen === myGen) putCacheThumb(key, null);
        } finally { thumbLoading.delete(key); }
      }));
      if (_thumbGen !== myGen) return;
      thumbCache = { ...thumbCache };
      await new Promise(r => setTimeout(r, 0));
    }
  }

  let _lastLoadedEntries = null;

  $: if ($currentPath !== _lastPath2) {
    _lastPath2 = $currentPath;
    _thumbGen++;
    renderLimit = 150;
    thumbLoading = new Set();
    _lastLoadedEntries = null;

    // RAM optimizasyonu: Eski klasörün thumbnail'lerini bellekten sil
    const p = $currentPath;
    for (const k of Object.keys(thumbCache)) {
      if (!k.startsWith(p)) {
        delete thumbCache[k];
      }
    }
    thumbCache = { ...thumbCache };

    if ($displayEntries && $displayEntries.length > 0) {
      loadThumbnailsAtSize($displayEntries, currentThumbSize);
    }
  }

  $: if ($searchQuery !== undefined) {
    renderLimit = 150;
  }

  // displayEntries dolduğunda veya değiştiğinde thumbnailleri yükle
  $: if ($displayEntries && $displayEntries.length > 0 && $displayEntries !== _lastLoadedEntries) {
    _lastLoadedEntries = $displayEntries;
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

  // Preview panel açıldığında veya kapandığında scroll yüksekliğini koru
  let _lastShowPreview = undefined;
  $: if ($showPreview !== _lastShowPreview) {
    _lastShowPreview = $showPreview;
    if (fileArea) {
      const top = fileArea.scrollTop;
      requestAnimationFrame(() => {
        if (fileArea && Math.abs(fileArea.scrollTop - top) > 2) {
          fileArea.scrollTop = top;
        }
      });
      setTimeout(() => {
        if (fileArea && Math.abs(fileArea.scrollTop - top) > 2) {
          fileArea.scrollTop = top;
        }
      }, 60);
    }
  }

  function handleFileAreaScroll() {
    if (fileArea && $activeTabId) {
      // Eğer yükleme sırasında DOM küçüldüyse ve scrollTop 0 olduysa, kullanıcının gerçek scroll'unu silme!
      if ($isLoading && fileArea.scrollTop === 0) return;
      saveTabScroll($activeTabId, fileArea.scrollTop);
    }
    checkRenderMore();
  }

  let onRestoreTabScroll = null;

  onMount(() => {
    if (fileArea) {
      fileArea.addEventListener("wheel", _onWheel, { passive: false });
    }
    if ($displayEntries && $displayEntries.length > 0) {
      loadThumbnailsAtSize($displayEntries, currentThumbSize);
    }
    onRestoreTabScroll = (e) => {
      const top = e.detail?.scrollTop ?? 0;
      requestAnimationFrame(() => {
        if (fileArea) fileArea.scrollTop = top;
      });
      setTimeout(() => {
        if (fileArea && Math.abs(fileArea.scrollTop - top) > 2) {
          fileArea.scrollTop = top;
        }
      }, 50);
    };
    const onFreeRam = () => {
      thumbCache = {};
      activeScrubbing = {};
      renderLimit = 150;
      notify("success", "RAM önbelleği temizlendi");
    };
    window.addEventListener("arch:free-ram", onFreeRam);
    window.addEventListener("arch:restore-tab-scroll", onRestoreTabScroll);
    setTimeout(() => fileArea?.focus(), 100);
  });

  onDestroy(() => {
    clearTimeout(_zoomDebounce);
    if (fileArea) fileArea.removeEventListener("wheel", _onWheel);
    if (onRestoreTabScroll) window.removeEventListener("arch:restore-tab-scroll", onRestoreTabScroll);
  });

  // ── Thumbnail görünürlük ──
  function showThumb(entry) {
    if (!isThumbable(entry)) return false;
    if ($thumbnailMode === "never")   return false;
    if ($thumbnailMode === "custom")  return $gridZoom >= $thumbnailMinSize;
    return true; // "always" ve "auto" modunda doğrudan göster!
  }

  // ── Ctrl + scroll → zoom ──
  // Non-passive listener onMount'ta ekleniyor (yukarıda)

  // F2 → seçili tek dosyayı inline rename'e al & Ok tuşları ile gezin
  function onKeyDown(e) {
    try {
      invoke("debug_log", { msg: `FileGridKey: key='${e.key}', code='${e.code}', which=${e.which}, keyCode=${e.keyCode}, selCount=${$selectedPaths.size}` }).catch(() => {});
      const isF2 = e.key === "F2" || e.code === "F2" || e.keyCode === 113;
      if (isF2) {
        let path = [...$selectedPaths][0];
        if (!path && $previewEntry?.path) path = $previewEntry.path;
        if (path) {
          e.preventDefault();
          const entry = $displayEntries.find(en => en.path === path);
          if (entry) startRename(entry);
          return;
        }
      }

      if (e.altKey) return; // Alt+Arrows are handled by App.svelte for navigation

      const isDown  = e.key === "ArrowDown"  || e.key === "Down"  || e.code === "ArrowDown"  || e.keyCode === 40;
      const isUp    = e.key === "ArrowUp"    || e.key === "Up"    || e.code === "ArrowUp"    || e.keyCode === 38;
      const isLeft  = e.key === "ArrowLeft"  || e.key === "Left"  || e.code === "ArrowLeft"  || e.keyCode === 37;
      const isRight = e.key === "ArrowRight" || e.key === "Right" || e.code === "ArrowRight" || e.keyCode === 39;

      if (isDown || isUp || isLeft || isRight) {
        const tag = document.activeElement?.tagName?.toLowerCase();
        if (tag === "input" || tag === "textarea") {
          invoke("debug_log", { msg: `ArrowIgnored: input focused (${tag})` }).catch(() => {});
          return;
        }
        if ($showDualPane && $activePaneId !== "left") {
          invoke("debug_log", { msg: `ArrowIgnored: activePaneId=${$activePaneId}` }).catch(() => {});
          return;
        }
        const entries = $displayEntries;
        if (!entries.length) {
          invoke("debug_log", { msg: `ArrowIgnored: entries empty` }).catch(() => {});
          return;
        }

        const currentList = [...$selectedPaths];
        let currentIdx = -1;
        if (currentList.length > 0) {
          currentIdx = entries.findIndex(en => en.path === currentList[currentList.length - 1]);
        }
        if (currentIdx === -1 && $previewEntry) {
          currentIdx = entries.findIndex(en => en.path === $previewEntry.path);
        }

        let step = 1;
        if ($viewMode === "grid") {
          const itemWidth = $gridZoom + 16;
          const containerWidth = fileArea?.clientWidth || 800;
          const cols = Math.max(1, Math.floor(containerWidth / itemWidth));
          if (isDown) step = cols;
          else if (isUp) step = -cols;
          else if (isRight) step = 1;
          else if (isLeft) step = -1;
        } else {
          if (isDown || isRight) step = 1;
          else if (isUp || isLeft) step = -1;
        }

        let nextIdx = currentIdx === -1 ? 0 : Math.max(0, Math.min(entries.length - 1, currentIdx + step));
        if (nextIdx >= renderLimit) {
          renderLimit = Math.min(entries.length, nextIdx + 50);
        }
        e.preventDefault();
        const nextEntry = entries[nextIdx];
        invoke("debug_log", { msg: `ArrowSuccess: curIdx=${currentIdx}, nextIdx=${nextIdx}, path=${nextEntry?.name}` }).catch(() => {});
        selectedPaths.set(new Set([nextEntry.path]));
        previewEntry.set(nextEntry);

        requestAnimationFrame(() => {
          const el = fileArea?.querySelector(`[data-path="${CSS.escape(nextEntry.path)}"]`);
          if (el) el.scrollIntoView({ block: "nearest" });
        });
        return;
      }
    } catch (err) {
      invoke("debug_log", { msg: `onKeyDown error: ${String(err)}` }).catch(() => {});
    }
  }
</script>

<svelte:window on:keydown={onKeyDown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="file-area"
  bind:this={fileArea}
  class:grid-mode={$viewMode === "grid"}
  class:list-mode={$viewMode === "list"}
  tabindex="0"
  on:scroll={handleFileAreaScroll}
  on:click|self={onBgClick}
  on:contextmenu={onBgContextMenu}
  on:mousedown={onAreaMouseDown}
  on:dragover|preventDefault
  on:drop={onDropIntoBg}
>
  <!-- Selection rectangle -->
  {#if selRect && (selRect.w > 4 || selRect.h > 4)}
    <div
      class="sel-rect"
      style="left:{selRect.x}px; top:{selRect.y}px; width:{selRect.w}px; height:{selRect.h}px;"
    ></div>
  {/if}

  {#if $isLoading && $displayEntries.length === 0}
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
    {#if $isLoading}
      <div class="loading-overlay-bar"></div>
    {/if}
    <div class="grid" style="grid-template-columns: repeat(auto-fill, minmax({$gridZoom}px, 1fr))">
      {#each renderedEntries as entry (entry.path)}
        {@const selected = $selectedPaths.has(entry.path)}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="grid-item"
          class:selected
          class:renaming={$renamingPath === entry.path}
          class:drag-over={dragOverPath === entry.path}
          data-path={entry.path}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          draggable={$renamingPath !== entry.path}
          on:dragstart={e => onDragStart(e, entry)}
          on:dragenter={e => {
            if (entry.is_dir) {
              e.stopPropagation();
              dragOverPath = entry.path;
            }
          }}
          on:dragleave={e => {
            if (dragOverPath === entry.path) dragOverPath = null;
          }}
          on:dragover|preventDefault={e => {
            if (entry.is_dir) {
              e.dataTransfer.dropEffect = "move";
              dragOverPath = entry.path;
            }
          }}
          on:drop={e => {
            dragOverPath = null;
            onDropIntoDir(e, entry);
          }}
          title={entry.path}
        >
          <!-- Thumbnail veya emoji ikon -->
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="grid-icon"
            class:is-scrubbing={!!activeScrubbing[entry.path]}
            style="color:{getColor(entry.mime_type)}"
            on:mousedown={e => {
              if (e.button === 0 && !e.target.closest("input")) {
                toggleSelect(e, entry.path);
              }
            }}
            on:mousemove={e => onVideoMouseMove(e, entry)}
            on:mouseleave={() => onVideoMouseLeave(entry)}
          >
            {#if activeScrubbing[entry.path]?.url}
              <!-- Canlı Kare Önizleme (Hover Scrubbing) -->
              <div class="scrub-container" style="position:relative; width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px">
                <img
                  class="grid-thumb scrubbing-thumb"
                  src={activeScrubbing[entry.path].url}
                  alt={entry.name}
                  style="width:100%; height:100%; object-fit:contain"
                />
                <div class="scrub-progress-track">
                  <div class="scrub-progress-bar" style="width:{Math.round((activeScrubbing[entry.path].pct || 0) * 100)}%"></div>
                </div>
              </div>
            {:else if showThumb(entry)}
              {#if isDirectThumb(entry)}
                <!-- GIF: animasyonlu | SVG: tarayıcı render (erişilemezse get_thumbnail base64 fallback) -->
                <img
                  class="grid-thumb"
                  src={convertFileSrc(entry.path)}
                  alt={entry.name}
                  loading="lazy"
                  style="width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px; max-width:100%; object-fit:contain"
                  on:error={async (e) => {
                    try {
                      const dataUrl = await invoke("get_thumbnail", { path: entry.path, maxSize: currentThumbSize });
                      if (dataUrl) {
                        e.target.src = dataUrl;
                        e.target.style.opacity = "1";
                        thumbCache[`${entry.path}_${currentThumbSize}`] = dataUrl;
                        thumbCache = { ...thumbCache };
                      } else {
                        e.target.style.opacity = "0";
                      }
                    } catch {
                      e.target.style.opacity = "0";
                    }
                  }}
                />
              {:else if getCachedThumb(entry, thumbCache)}
                <!-- Rust cache (video veya önbelleklenmiş resim) -->
                <img
                  class="grid-thumb"
                  src={getCachedThumb(entry, thumbCache)}
                  alt={entry.name}
                  style="width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px; max-width:100%; object-fit:contain"
                />
              {:else if IMAGE_EXTS.has((entry.extension || "").toLowerCase()) || entry.mime_type?.startsWith("image/")}
                <!-- Resimler: convertFileSrc ile anında göster — reload beklemesin! -->
                <img
                  class="grid-thumb"
                  src={convertFileSrc(entry.path)}
                  alt={entry.name}
                  loading="lazy"
                  style="width:{Math.max(40, $gridZoom - 16)}px; height:{Math.max(40, $gridZoom - 16)}px; max-width:100%; object-fit:contain"
                  on:error={async () => {
                    try {
                      const dataUrl = await invoke("get_thumbnail", { path: entry.path, maxSize: currentThumbSize });
                      if (dataUrl) {
                        thumbCache[`${entry.path}_${currentThumbSize}`] = dataUrl;
                        thumbCache = { ...thumbCache };
                      }
                    } catch {}
                  }}
                />
              {:else}
                <!-- Videolar veya yüklenenler — emoji göster -->
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
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div
              class="grid-name"
              on:mousedown={e => {
                if (e.button === 0) {
                  toggleSelect(e, entry.path);
                }
              }}
            >{entry.name}</div>
          {/if}

          {#if !entry.is_dir && entry.size > 0}
            <div class="grid-size">{formatSize(entry.size)}</div>
          {/if}
        </div>
      {/each}
    </div>
    {#if renderLimit < $displayEntries.length}
      <div class="virtual-batch-pill">Gösterilen: {renderedEntries.length} / {$displayEntries.length} dosya (Aşağı kaydırdıkça otomatik yüklenir)</div>
    {/if}

  {:else}
    <!-- List view -->
    <div class="list">
      {#if $isLoading}
        <div class="loading-overlay-bar"></div>
      {/if}
      <div class="list-header" style={listGridStyle}>
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div class="col-header col-name sortable" on:click={() => toggleSort("name")} title="İsme göre sırala">
          <span>{$t("name")}</span>
          {#if $sortBy === "name"}
            <span class="sort-indicator">{$sortAsc ? "▲" : "▼"}</span>
          {/if}
        </div>
        <div class="col-header col-size sortable" style="position:relative">
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-title" on:click={() => toggleSort("size")} title="Boyuta göre sırala">
            <span>{$t("size")}</span>
            {#if $sortBy === "size"}
              <span class="sort-indicator">{$sortAsc ? "▲" : "▼"}</span>
            {/if}
          </div>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-resizer" on:mousedown={e => onColResizeStart(e, "size")} title="Genişliği ayarla"></div>
        </div>
        <div class="col-header col-date sortable" style="position:relative">
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-title" on:click={() => toggleSort("modified")} title="Tarihe göre sırala">
            <span>{$t("modified")}</span>
            {#if $sortBy === "modified" || $sortBy === "date"}
              <span class="sort-indicator">{$sortAsc ? "▲" : "▼"}</span>
            {/if}
          </div>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-resizer" on:mousedown={e => onColResizeStart(e, "date")} title="Genişliği ayarla"></div>
        </div>
        <div class="col-header col-perms sortable" style="position:relative">
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-title" on:click={() => toggleSort("perms")} title="İzinlere göre sırala">
            <span>{$t("permissions")}</span>
            {#if $sortBy === "perms" || $sortBy === "permissions"}
              <span class="sort-indicator">{$sortAsc ? "▲" : "▼"}</span>
            {/if}
          </div>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div class="col-resizer" on:mousedown={e => onColResizeStart(e, "perms")} title="Genişliği ayarla"></div>
        </div>
      </div>
      {#each renderedEntries as entry (entry.path)}
        {@const selected = $selectedPaths.has(entry.path)}
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="list-row"
          style={listGridStyle}
          class:selected
          class:drag-over={dragOverPath === entry.path}
          data-path={entry.path}
          on:dblclick={() => onItemDblClick(entry)}
          on:contextmenu={e => onContextMenu(e, entry)}
          on:dragenter={e => {
            if (entry.is_dir) {
              e.stopPropagation();
              dragOverPath = entry.path;
            }
          }}
          on:dragleave={e => {
            if (dragOverPath === entry.path) dragOverPath = null;
          }}
          on:dragover|preventDefault={e => {
            if (entry.is_dir) {
              e.dataTransfer.dropEffect = "move";
              dragOverPath = entry.path;
            }
          }}
          on:drop={e => {
            dragOverPath = null;
            onDropIntoDir(e, entry);
          }}
        >
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span
            class="col-name"
            draggable={$renamingPath !== entry.path}
            on:dragstart={e => onDragStart(e, entry)}
            on:mousedown={e => {
              if (e.button === 0 && !e.target.closest("input")) {
                toggleSelect(e, entry.path);
              }
            }}
          >
            {#if showThumb(entry)}
              {#if isDirectThumb(entry)}
                <img
                  class="list-thumb"
                  src={convertFileSrc(entry.path)}
                  alt={entry.name}
                  loading="lazy"
                  on:error={async (e) => {
                    try {
                      const dataUrl = await invoke("get_thumbnail", { path: entry.path, maxSize: 64 });
                      if (dataUrl) {
                        e.target.src = dataUrl;
                        e.target.style.opacity = "1";
                        thumbCache[`${entry.path}_${currentThumbSize}`] = dataUrl;
                        thumbCache = { ...thumbCache };
                      } else {
                        e.target.style.opacity = "0";
                      }
                    } catch {
                      e.target.style.opacity = "0";
                    }
                  }}
                />
              {:else if getCachedThumb(entry, thumbCache)}
                <img class="list-thumb" src={getCachedThumb(entry, thumbCache)} alt={entry.name} />
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
          <span class="col-size" title={entry.is_dir ? (entry.children_count != null ? `${entry.children_count} öğe` : "") : formatSize(entry.size)}>{formatFolderSize(entry)}</span>
          <span class="col-date" title={entry.modified}>{entry.modified}</span>
          <span class="col-perms" title={entry.permissions}>{entry.permissions}</span>
        </div>
      {/each}
      {#if renderLimit < $displayEntries.length}
        <div class="virtual-batch-pill">Gösterilen: {renderedEntries.length} / {$displayEntries.length} dosya (Aşağı kaydırdıkça otomatik yüklenir)</div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .virtual-batch-pill {
    padding: 8px 18px;
    margin: 14px auto;
    font-size: 11px;
    color: var(--text-dim);
    background: rgba(255, 255, 255, 0.05);
    border: 1px dashed var(--border);
    border-radius: 20px;
    width: fit-content;
    text-align: center;
    user-select: none;
    pointer-events: none;
  }

  .file-area {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 8px;
    position: relative;
    user-select: none;
    -webkit-user-select: none;
    overflow-anchor: none !important;
  }
  .file-area.list-mode {
    padding-top: 0;
    padding-left: 0;
    padding-right: 0;
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

  .loading-overlay-bar {
    position: sticky;
    top: 0;
    left: 0;
    right: 0;
    height: 3px;
    background: linear-gradient(90deg, transparent, var(--accent, #8264ff), transparent);
    background-size: 200% 100%;
    animation: barSlide 1.2s infinite ease-in-out;
    z-index: 30;
    border-radius: 2px;
  }
  @keyframes barSlide {
    0% { background-position: -200% 0; }
    100% { background-position: 200% 0; }
  }

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
    gap: 8px;
    padding: 6px;
  }

  .grid-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 6px 4px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    cursor: pointer;
    transition: var(--transition-fast);
    text-align: center;
    user-select: none;
    position: relative;
    background: transparent;
  }
  /* Nemo-style selection: Boş alanlarda tıklanabilirliği artırmak için tüm kutu yerine sadece ikon/thumbnail ve metin seçilir */
  .grid-item.selected .grid-icon {
    outline: 2px solid var(--border-accent, #8264ff);
    outline-offset: 3px;
    background: rgba(130, 100, 255, 0.22);
    border-radius: 6px;
    box-shadow: 0 0 12px rgba(130, 100, 255, 0.4);
  }
  .grid-item.selected .grid-name {
    background: var(--accent, #8264ff);
    color: #fff;
    border-radius: 4px;
    padding: 1px 6px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.4);
  }
  .grid-item:hover:not(.selected) .grid-icon {
    background: var(--bg-hover, rgba(255,255,255,0.06));
    border-radius: 6px;
  }
  .grid-item:hover:not(.selected) .grid-name {
    background: var(--bg-hover, rgba(255,255,255,0.08));
    border-radius: 4px;
    padding: 1px 6px;
  }

  .grid-icon {
    font-size: 32px;
    line-height: 1;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    transition: all 0.15s ease;
  }

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
  .list {
    display: flex;
    flex-direction: column;
    min-width: 100%;
    width: 100%;
  }

  .list-header {
    display: grid;
    align-items: center;
    padding: 6px 12px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
    user-select: none;
    position: sticky;
    top: 0;
    background: rgba(18, 18, 28, 0.90);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: 25;
    min-width: fit-content;
    width: 100%;
    box-sizing: border-box;
  }

  .col-header {
    display: flex;
    align-items: center;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    user-select: none;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
  }
  .col-header.sortable {
    cursor: pointer;
  }
  .col-header.sortable:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.04);
  }
  .col-title {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }
  .col-name.sortable {
    gap: 4px;
  }
  .sort-indicator {
    font-size: 10px;
    color: var(--accent, #8264ff);
    margin-left: 3px;
    line-height: 1;
  }

  .col-resizer {
    position: absolute;
    right: -4px;
    top: -4px;
    bottom: -4px;
    width: 8px;
    cursor: col-resize;
    z-index: 10;
    background: transparent;
    transition: background 0.15s ease;
  }
  .col-resizer:hover, .col-resizer:active {
    background: var(--accent, #8264ff);
    box-shadow: 0 0 6px var(--accent, #8264ff);
  }

  .list-row {
    display: grid;
    align-items: center;
    padding: 4px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
    transition: var(--transition-fast);
    user-select: none;
    min-width: fit-content;
    width: 100%;
    box-sizing: border-box;
  }
  .list-row:hover    { background: var(--bg-hover); }
  .list-row.selected { background: var(--bg-selected); border-color: var(--border-accent); }
  .list-row.drag-over,
  .grid-item.drag-over {
    background: rgba(130, 100, 255, 0.28) !important;
    outline: 2px dashed var(--accent, #8264ff) !important;
    outline-offset: -2px;
    box-shadow: 0 0 16px rgba(130, 100, 255, 0.5) !important;
  }

  .col-name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 120px;
    overflow: hidden;
    padding-right: 8px;
    font-size: var(--font-size-base);
  }
  .col-size, .col-date, .col-perms {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 6px;
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

  /* Video Hover Scrubbing */
  .grid-icon.is-scrubbing {
    transform: scale(1.04);
    transition: transform 0.15s ease;
  }
  .scrub-container,
  .scrub-container * {
    pointer-events: none !important;
  }
  .scrub-container {
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm, 6px);
    overflow: hidden;
  }
  .scrubbing-thumb {
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.45);
  }
  .scrub-progress-track {
    position: absolute;
    bottom: 0;
    left: 4px;
    right: 4px;
    height: 3px;
    background: rgba(0, 0, 0, 0.6);
    border-radius: 2px;
    overflow: hidden;
  }
  .scrub-progress-bar {
    height: 100%;
    background: var(--accent, #8264ff);
    box-shadow: 0 0 6px var(--accent, #8264ff);
    transition: width 0.05s linear;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
