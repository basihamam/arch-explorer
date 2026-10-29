<script>
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
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
    requestSudo, fetchDirectoryListing,
    previewEntry, showPreview, isRubberbandSelecting,
    thumbnailMode, thumbnailMinSize, runCommand,
    safeBatchFileOp, rightSelectedPaths, rightEntries, openEntry,
    toggleSort, setFolderSort,
    isTrashPath, restoreTrashPaths, restoreAllTrashItems, emptyTrashNow,
  } from "../stores/explorer.js";
  import { get } from "svelte/store";
  import { t } from "../i18n.js";

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
  $: inTrash = isTrashPath(currentPath);

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
    const raw = searchQuery.trim() ? searchResults : entries;
    const list = Array.isArray(raw) ? raw : [];
    return [...list].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      switch ($sortBy) {
        case "name":
          cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
          break;
        case "size":
          cmp = (a.size || 0) - (b.size || 0);
          break;
        case "modified":
        case "date": {
          const dateA = a.modified ? new Date(a.modified).getTime() : 0;
          const dateB = b.modified ? new Date(b.modified).getTime() : 0;
          if (!isNaN(dateA) && !isNaN(dateB) && (dateA > 0 || dateB > 0)) {
            cmp = dateA - dateB;
          } else {
            cmp = (a.modified || "").localeCompare(b.modified || "");
          }
          break;
        }
        case "perms":
        case "permissions":
          cmp = (a.permissions || "").localeCompare(b.permissions || "");
          break;
        case "type":
          cmp = (a.extension || "").localeCompare(b.extension || "");
          break;
        default:
          cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
      }
      if (cmp === 0) {
        cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
      }
      return $sortAsc ? cmp : -cmp;
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
    selectedPaths = new Set();
    previewEntry.set(null);
    try {
      const res = await fetchDirectoryListing(path, _showHidden);
      entries = Array.isArray(res?.entries) ? res.entries : (Array.isArray(res) ? res : []);
      const targetPath = res.path || path;
      _updateTab(t => ({ ...t, path: targetPath, label: targetPath.split("/").filter(Boolean).pop() || "/" }));
      selectedPaths = new Set();
      previewEntry.set(null);
      invoke("start_watching", { path: targetPath }).catch(() => {});
    } catch (e) {
      loadError = String(e);
    } finally {
      isLoading = false;
    }
  }

  function navigateTo(path) {
    selectedPaths = new Set();
    previewEntry.set(null);
    _updateTab(t => {
      const hist = [...t.history.slice(0, t.histIdx + 1), path];
      return { ...t, history: hist, histIdx: hist.length - 1, path, label: path.split("/").filter(Boolean).pop() || "/" };
    });
    loadDir(path);
    dispatch("focus", { paneId });
  }

  function navigateBack() {
    if (!canBack) return;
    const targetPath = activeTab.history[activeTab.histIdx - 1];
    selectedPaths = new Set();
    previewEntry.set(null);
    _updateTab(t => ({ ...t, histIdx: t.histIdx - 1, path: targetPath, label: targetPath.split("/").filter(Boolean).pop() || "/" }));
    loadDir(targetPath);
  }

  function navigateForward() {
    if (!canForward) return;
    const targetPath = activeTab.history[activeTab.histIdx + 1];
    selectedPaths = new Set();
    previewEntry.set(null);
    _updateTab(t => ({ ...t, histIdx: t.histIdx + 1, path: targetPath, label: targetPath.split("/").filter(Boolean).pop() || "/" }));
    loadDir(targetPath);
  }

  function navigateUp() {
    const p = (currentPath || "").replace(/\/+$/, "") || "/";
    if (p === "/") return;
    const parent = p.split("/").slice(0, -1).join("/") || "/";
    navigateTo(parent);
  }

  // ── Selection ──
  let paneSelectionAnchor = null;

  function toggleSelect(e, path) {
    e.stopPropagation();
    const paths = displayEntries.map(en => en.path);
    const targetIdx = paths.indexOf(path);
    const next = new Set(selectedPaths);

    if (e.shiftKey) {
      // Shift ile aralık seçimi
      const anchorIdx = paneSelectionAnchor ? paths.indexOf(paneSelectionAnchor) : -1;
      const start = anchorIdx !== -1 ? anchorIdx : (targetIdx !== -1 ? targetIdx : 0);
      const [lo, hi] = [Math.min(start, targetIdx), Math.max(start, targetIdx)];
      if (!e.ctrlKey) next.clear();
      for (let i = lo; i <= hi; i++) {
        if (paths[i]) next.add(paths[i]);
      }
    } else if (e.ctrlKey) {
      if (next.has(path)) next.delete(path); else next.add(path);
      paneSelectionAnchor = path;
    } else {
      next.clear();
      next.add(path);
      paneSelectionAnchor = path;
    }

    selectedPaths = next;
    dispatch("focus", { paneId });

    // Preview panel — son tıklanan entry güncelle
    const entry = displayEntries.find(en => en.path === path) || null;
    if (entry) {
      previewEntry.set(entry);
    } else if (next.size === 0) {
      previewEntry.set(null);
    }
  }

  let justFinishedDraggingSelection = false;

  function clearSelection(e) {
    if (justFinishedDraggingSelection) {
      justFinishedDraggingSelection = false;
      return;
    }
    if (e?.target.closest("[data-path]")) return;
    selectedPaths = new Set();
  }

  // ── Selection Rectangle (Rubber-band Selection) ──
  let paneFileArea;
  let selRect   = null;
  let selStart  = null;
  let selecting = false;
  let hasDragged = false;
  let lastClientX = 0;
  let lastClientY = 0;

  function onAreaMouseDown(e) {
    if (e.button !== 0) return;
    if (document.activeElement && (document.activeElement.tagName === "INPUT" || document.activeElement.tagName === "TEXTAREA")) {
      document.activeElement.blur();
    }
    // Only return if clicking directly on interactive elements (file name, icon, resizers, inputs, buttons)
    if (e.target.closest(".pane-grid-icon, .pane-grid-name, .pane-col-name, input, button")) return;

    if (e.ctrlKey || e.shiftKey) {
      const row = e.target.closest("[data-path]");
      if (row?.dataset?.path) {
        toggleSelect(e, row.dataset.path);
      }
      return;
    }

    if (!paneFileArea) return;
    const rect = paneFileArea.getBoundingClientRect();
    lastClientX = e.clientX;
    lastClientY = e.clientY;
    selStart  = {
      x: e.clientX - rect.left + paneFileArea.scrollLeft,
      y: e.clientY - rect.top  + paneFileArea.scrollTop,
    };
    selecting = true;
    hasDragged = false;
    selRect   = null;

    closeCtx();
    dispatch("focus", { paneId });

    window.addEventListener("mousemove", onAreaMouseMove);
    window.addEventListener("mouseup",   onAreaMouseUp, { once: true });
    window.addEventListener("blur",      cancelAreaSelection, { once: true });
    window.addEventListener("contextmenu", cancelAreaSelection, { once: true });
  }

  function cancelAreaSelection() {
    if (!selecting) return;
    selecting = false;
    isRubberbandSelecting.set(false);
    if (autoScrollRaf) {
      cancelAnimationFrame(autoScrollRaf);
      autoScrollRaf = null;
    }
    window.removeEventListener("mousemove", onAreaMouseMove);
    window.removeEventListener("mouseup", onAreaMouseUp);
    window.removeEventListener("blur", cancelAreaSelection);
    window.removeEventListener("contextmenu", cancelAreaSelection);
    selRect = null;
  }

  let autoScrollRaf = null;

  function updateAutoScroll() {
    if (!selecting || !paneFileArea) {
      if (autoScrollRaf) {
        cancelAnimationFrame(autoScrollRaf);
        autoScrollRaf = null;
      }
      return;
    }

    const rect = paneFileArea.getBoundingClientRect();
    const edgeThreshold = 40;
    const maxSpeed = 16;
    let scrollDelta = 0;

    if (lastClientY < rect.top + edgeThreshold) {
      const intensity = Math.max(0, Math.min(1, (rect.top + edgeThreshold - lastClientY) / edgeThreshold));
      scrollDelta = -Math.round(intensity * maxSpeed);
    } else if (lastClientY > rect.bottom - edgeThreshold) {
      const intensity = Math.max(0, Math.min(1, (lastClientY - (rect.bottom - edgeThreshold)) / edgeThreshold));
      scrollDelta = Math.round(intensity * maxSpeed);
    }

    if (scrollDelta !== 0) {
      paneFileArea.scrollTop += scrollDelta;
      recalculateSelection(lastClientY);
    }

    autoScrollRaf = requestAnimationFrame(updateAutoScroll);
  }

  function recalculateSelection(clientY) {
    if (!selecting || !selStart || !paneFileArea) return;
    const rect = paneFileArea.getBoundingClientRect();
    const curX = lastClientX - rect.left + paneFileArea.scrollLeft;
    const curY = clientY - rect.top + paneFileArea.scrollTop;

    selRect = {
      x: Math.min(selStart.x, curX),
      y: Math.min(selStart.y, curY),
      w: Math.abs(curX - selStart.x),
      h: Math.abs(curY - selStart.y),
    };

    if (selRect.w > 4 || selRect.h > 4) {
      if (!hasDragged) {
        hasDragged = true;
        isRubberbandSelecting.set(true);
      }
      const newSel = new Set();
      paneFileArea.querySelectorAll("[data-path]").forEach(item => {
        const ir    = item.getBoundingClientRect();
        const itemX = ir.left - rect.left + paneFileArea.scrollLeft;
        const itemY = ir.top  - rect.top  + paneFileArea.scrollTop;

        const overlaps = !(
          itemX > selRect.x + selRect.w ||
          itemX + ir.width  < selRect.x ||
          itemY > selRect.y + selRect.h ||
          itemY + ir.height < selRect.y
        );
        if (overlaps && item.dataset.path) newSel.add(item.dataset.path);
      });
      selectedPaths = newSel;
    }
  }

  function onAreaMouseMove(e) {
    if (!selecting || !selStart || !paneFileArea) return;
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
    window.removeEventListener("blur", cancelAreaSelection);
    window.removeEventListener("contextmenu", cancelAreaSelection);

    if (hasDragged) {
      isRubberbandSelecting.set(false);
      previewEntry.set(null);
      justFinishedDraggingSelection = true;
      setTimeout(() => {
        justFinishedDraggingSelection = false;
      }, 120);
    } else {
      isRubberbandSelecting.set(false);
      // Eğer sürükleme olmadıysa (saf boşluk tıklaması ise) seçimi temizle
      selectedPaths = new Set();
      previewEntry.set(null);
    }
    selRect   = null;
    selStart  = null;
    hasDragged = false;
  }

  // ── Thumbnail Support ──
  const IMAGE_EXTS   = new Set(["jpg","jpeg","png","webp","bmp","tiff","tif","avif","ico"]);
  const VIDEO_EXTS   = new Set(["mp4","mkv","webm","avi","mov","wmv","flv","m4v","ts","mpg","mpeg","3gp","ogv"]);
  const DIRECT_EXTS  = new Set(["gif","svg"]);

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
    const ext = (entry?.extension || "").toLowerCase();
    return DIRECT_EXTS.has(ext);
  }

  function thumbSizeForZoom(zoom) {
    if (zoom < 85) return 64;
    if (zoom < 135) return 128;
    if (zoom < 185) return 256;
    return 384;
  }

  $: currentThumbSize = thumbSizeForZoom($rightGridZoom);

  // ── High Performance Virtual Batching & RAM Optimization ──
  let renderLimit = 150;
  const CHUNK_SIZE = 100;
  $: renderedEntries = (displayEntries && displayEntries.length > renderLimit)
    ? displayEntries.slice(0, renderLimit)
    : (displayEntries || []);

  function checkRenderMore() {
    if (!paneFileArea) return;
    const scrollBottom = paneFileArea.scrollHeight - paneFileArea.scrollTop - paneFileArea.clientHeight;
    if (scrollBottom < 600 && renderLimit < (displayEntries?.length || 0)) {
      renderLimit = Math.min(displayEntries.length, renderLimit + CHUNK_SIZE);
      if (displayEntries && displayEntries.length > 0) {
        loadThumbnailsAtSize(displayEntries, currentThumbSize);
      }
    }
  }

  let thumbCache   = {};
  let thumbLoading = new Set();
  let _thumbGen    = 0;
  let _lastSize    = 0;
  let _lastPath2   = "";
  let _zoomDebounce = null;
  const MAX_THUMB_CACHE = 120;

  function putCacheThumb(key, val) {
    const keys = Object.keys(thumbCache);
    if (keys.length >= MAX_THUMB_CACHE) {
      for (let i = 0; i < 30; i++) {
        delete thumbCache[keys[i]];
      }
    }
    thumbCache[key] = val;
  }

  function getCachedThumb(entry, cache = thumbCache) {
    if (!cache) return null;
    const exact = cache[`${entry.path}_${currentThumbSize}`];
    if (exact) return exact;
    return cache[`${entry.path}_384`] ||
           cache[`${entry.path}_256`] ||
           cache[`${entry.path}_128`] ||
           cache[`${entry.path}_64`] ||
           null;
  }

  async function loadThumbnailsAtSize(entriesList, size) {
    const myGen = _thumbGen;
    const targetSlice = entriesList.slice(0, Math.min(entriesList.length, renderLimit + 40));
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

  $: if (currentPath !== _lastPath2) {
    _lastPath2 = currentPath;
    _thumbGen++;
    renderLimit = 150;
    thumbLoading = new Set();
    _lastLoadedEntries = null;

    // RAM optimizasyonu
    const p = currentPath;
    for (const k of Object.keys(thumbCache)) {
      if (!k.startsWith(p)) {
        delete thumbCache[k];
      }
    }
    thumbCache = { ...thumbCache };

    if (displayEntries && displayEntries.length > 0) {
      loadThumbnailsAtSize(displayEntries, currentThumbSize);
    }
  }

  // displayEntries dolduğunda veya değiştiğinde thumbnailleri yükle
  $: if (displayEntries && displayEntries.length > 0 && displayEntries !== _lastLoadedEntries) {
    _lastLoadedEntries = displayEntries;
    loadThumbnailsAtSize(displayEntries, currentThumbSize);
  }

  $: if (currentThumbSize !== _lastSize) {
    _lastSize = currentThumbSize;
    clearTimeout(_zoomDebounce);
    _zoomDebounce = setTimeout(() => {
      _thumbGen++;
      loadThumbnailsAtSize(displayEntries, currentThumbSize);
    }, 150);
  }

  function showThumb(entry) {
    if (!isThumbable(entry)) return false;
    if ($thumbnailMode === "never")   return false;
    if ($thumbnailMode === "custom")  return $rightGridZoom >= $thumbnailMinSize;
    return true; // "always" ve "auto" modunda doğrudan göster!
  }

  // ── File ops ──
  function onDblClick(entry) {
    if (entry.is_dir) navigateTo(entry.path);
    else openEntry(entry);
  }

  // Cross-pane: F5 kopyala → karşı pane, F6 taşı → karşı pane
  async function copyToOther() {
    const sel = [...selectedPaths];
    if (!sel.length) return;
    await safeBatchFileOp(sel, otherPath, "copy");
    dispatch("reload-other");
  }

  async function moveToOther() {
    const sel = [...selectedPaths];
    if (!sel.length) return;
    await safeBatchFileOp(sel, otherPath, "move");
    await loadDir(currentPath);
    dispatch("reload-other");
  }

  // ── Context menu ──
  let ctxMenu = null;
  let paneRenamingPath = null;
  let paneRenameInput = "";
  let paneRenameInputEl;
  let _lastPaneRenamingPath = null;
  let isPaneCommittingRename = false;
  let showMoreCtx = false;

  $: if (paneRenamingPath && paneRenamingPath !== _lastPaneRenamingPath) {
    _lastPaneRenamingPath = paneRenamingPath;
    const entry = displayEntries.find(e => e.path === paneRenamingPath);
    if (entry) {
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
  } else if (!paneRenamingPath) {
    _lastPaneRenamingPath = null;
  }

  async function commitPaneRename(entry) {
    if (isPaneCommittingRename) return;
    const targetEntry = entry;
    const newName = paneRenameInput.trim();
    isPaneCommittingRename = true;
    paneRenamingPath = null;

    if (!newName || !targetEntry || newName === targetEntry.name) {
      isPaneCommittingRename = false;
      return;
    }

    const cleanOldPath = targetEntry.path.replace(/\/+$/, "");
    const lastSlash = cleanOldPath.lastIndexOf("/");
    const parentDir = lastSlash >= 0 ? cleanOldPath.slice(0, lastSlash) : "";
    const newPath = (parentDir === "" ? "" : parentDir) + "/" + newName;

    try {
      await invoke("tracked_rename", { oldPath: targetEntry.path, newPath });
      await loadDir(currentPath);
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
          await loadDir(currentPath);
        } catch (sudoErr) {
          notify("error", String(sudoErr));
        }
      } else {
        notify("error", String(e));
      }
    } finally {
      isPaneCommittingRename = false;
    }
  }

  function cancelPaneRename() {
    isPaneCommittingRename = false;
    paneRenamingPath = null;
    paneRenameInput = "";
  }

  function onCtxMenu(e, entry) {
    e.preventDefault(); e.stopPropagation();
    if (entry && !selectedPaths.has(entry.path)) {
      selectedPaths = new Set([entry.path]);
    } else if (!entry) {
      selectedPaths = new Set();
    }
    showMoreCtx = false;
    const menuW = 240;
    const menuH = 460;
    const posX = Math.min(e.clientX, window.innerWidth - menuW - 10);
    const posY = Math.min(e.clientY, window.innerHeight - menuH - 10);
    ctxMenu = { x: Math.max(10, posX), y: Math.max(10, posY), target: entry };
    dispatch("focus", { paneId });
  }

  function closeCtx() { ctxMenu = null; showMoreCtx = false; }

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
    const isPerma = permanent || inTrash;
    if (isPerma && !confirm(`Permanently delete ${paths.length} item(s)? / Seçili ${paths.length} öğe kalıcı olarak silinsin mi?`)) return;
    for (const p of paths) await invoke("delete_item", { path: p, permanent: isPerma }).catch(() => {});
    notify("success", `${paths.length} item(s) deleted`);
    await loadDir(currentPath);
    closeCtx();
  }

  async function ctxRestore() {
    const paths = [...selectedPaths].length ? [...selectedPaths] : [ctxMenu?.target?.path].filter(Boolean);
    if (!paths.length) return;
    await restoreTrashPaths(paths);
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

  function ctxCopy() {
    const paths = [...selectedPaths].length ? [...selectedPaths] : [ctxMenu?.target?.path].filter(Boolean);
    if (!paths.length) return;
    clipboard.set({ op: "copy", paths });
    notify("info", `Copied ${paths.length} item(s)`);
    closeCtx();
  }

  function ctxCut() {
    const paths = [...selectedPaths].length ? [...selectedPaths] : [ctxMenu?.target?.path].filter(Boolean);
    if (!paths.length) return;
    clipboard.set({ op: "cut", paths });
    notify("info", `Cut ${paths.length} item(s)`);
    closeCtx();
  }

  async function ctxPaste() {
    const cb = get(clipboard);
    if (!cb?.paths?.length) return;
    const targetDir = ctxMenu?.target?.is_dir ? ctxMenu.target.path : currentPath;
    const op = cb.op === "cut" ? "move" : "copy";
    const res = await safeBatchFileOp(cb.paths, targetDir, op);
    if (cb.op === "cut" && res.success_count > 0) clipboard.set({ op: "copy", paths: [] });
    await loadDir(currentPath);
    if (res.created_paths && res.created_paths.length > 0) {
      selectedPaths = new Set(res.created_paths);
    }
    closeCtx();
  }

  async function ctxExtract() {
    const path = ctxMenu?.target?.path;
    if (!path) return;
    closeCtx();
    try {
      notify("info", "Çıkarılıyor...");
      const result = await invoke("extract_archive", { archivePath: path });
      notify("success", result.message);
      await loadDir(currentPath);
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function ctxCompress() {
    const paths = [...selectedPaths].length ? [...selectedPaths] : [ctxMenu?.target?.path].filter(Boolean);
    if (!paths.length) return;
    closeCtx();
    const first = paths[0].split("/").pop() || "archive";
    const dotIdx = first.lastIndexOf(".");
    const archiveName = dotIdx > 0 ? first.slice(0, dotIdx) : first;
    try {
      const result = await invoke("compress_files", {
        paths,
        destDir: currentPath,
        archiveName,
        format: "zip",
      });
      notify("success", result.message);
      await loadDir(currentPath);
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function ctxChecksum() {
    const path = ctxMenu?.target?.path;
    if (!path) return;
    closeCtx();
    try {
      const res = await invoke("get_checksum", { path });
      if (res?.sha256) {
        await navigator.clipboard.writeText(res.sha256);
        notify("success", `SHA256 panoya kopyalandı: ${res.sha256.slice(0, 16)}…`);
      }
    } catch (e) {
      notify("error", String(e));
    }
  }

  async function ctxTerminal() {
    const target = ctxMenu?.target;
    const dir = target?.is_dir ? target.path : (target ? target.path.replace(/\/[^/]+$/, "") : currentPath);
    closeCtx();
    try {
      await invoke("run_user_command", {
        commandId: "open-terminal",
        selectedPath: dir,
        currentDir: dir,
      });
    } catch {
      await invoke("plugin:shell|execute", {
        program: "kitty",
        args: ["--directory", dir],
        options: {}
      }).catch(() => notify("error", "Cannot open terminal"));
    }
  }

  async function ctxRunCmd(cmd) {
    const target = ctxMenu?.target;
    closeCtx();
    await runCommand(cmd.id, target?.path);
  }

  // ── Drag & drop ──
  let dragSrc = null;
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

  function formatFileUri(path) {
    const clean = path.replace(/\\/g, "/");
    const parts = clean.split("/").map(seg => encodeURIComponent(seg));
    return "file://" + parts.join("/");
  }

  function extractDroppedPaths(e) {
    let paths = [];
    try {
      const internal = e.dataTransfer.getData("application/x-arch-explorer-paths");
      if (internal) paths = JSON.parse(internal);
    } catch {}
    if (!paths.length) {
      try {
        const uriList = e.dataTransfer.getData("text/uri-list");
        if (uriList) {
          paths = uriList
            .split(/[\r\n]+/)
            .map(u => u.trim())
            .filter(u => u.startsWith("file://"))
            .map(u => decodeURIComponent(u.replace(/^file:\/\//, "")))
            .filter(Boolean);
        }
      } catch {}
    }
    if (!paths.length) {
      try {
        const raw = e.dataTransfer.getData("text/plain");
        if (raw.startsWith("[") && raw.endsWith("]")) {
          paths = JSON.parse(raw);
        } else if (raw) {
          paths = raw.split("\n").map(s => s.trim()).filter(Boolean);
        }
      } catch {}
    }
    return paths;
  }

  function onDragStart(e, entry) {
    dragSrc = entry;
    e.dataTransfer.effectAllowed = "copyMove";
    const sel = [...selectedPaths];
    const paths = sel.includes(entry.path) && sel.length > 1 ? sel : [entry.path];

    // 1. text/uri-list: used by Firefox, Chrome, and desktop apps for file drops
    const uriList = paths.map(formatFileUri).join("\r\n") + "\r\n";
    try { e.dataTransfer.setData("text/uri-list", uriList); } catch {}

    // 2. x-special/gnome-copied-files: GTK / GNOME drop support
    try {
      e.dataTransfer.setData(
        "x-special/gnome-copied-files",
        "copy\n" + paths.map(formatFileUri).join("\n")
      );
    } catch {}

    // 3. Plain text paths (newline-separated)
    try { e.dataTransfer.setData("text/plain", paths.join("\n")); } catch {}

    // 4. Internal application format
    try { e.dataTransfer.setData("application/x-arch-explorer-paths", JSON.stringify(paths)); } catch {}

    // 5. DownloadURL header
    if (paths.length === 1 && !entry.is_dir) {
      try {
        const mime = entry.mime_type || "application/octet-stream";
        e.dataTransfer.setData("DownloadURL", `${mime}:${entry.name}:${formatFileUri(entry.path)}`);
      } catch {}
    }

    try {
      setCustomDragImage(e, entry, paths.length);
    } catch {}
  }

  async function onDrop(e, targetEntry) {
    e.preventDefault();
    e.stopPropagation();
    if (!targetEntry?.is_dir) return;
    let paths = extractDroppedPaths(e);
    if (!paths.length && dragSrc) paths = [dragSrc.path];
    paths = paths.filter(p => p !== targetEntry.path);
    if (!paths.length) return;
    dragSrc = null;
    await safeBatchFileOp(paths, targetEntry.path, "move");
    await loadDir(currentPath);
    dispatch("reload-other");
  }

  async function onDropBg(e) {
    e.preventDefault();
    e.stopPropagation();
    let paths = extractDroppedPaths(e);
    if (!paths.length && dragSrc) paths = [dragSrc.path];
    if (!paths.length) return;
    dragSrc = null;
    await safeBatchFileOp(paths, currentPath, "move");
    await loadDir(currentPath);
    dispatch("reload-other");
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

  $: rightSelectedPaths.set(selectedPaths);
  $: rightEntries.set(entries);

  // Keyboard
  function onKeyDown(e) {
    if (!isActive) return;
    const tag = document.activeElement?.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea") return;

    const isC = e.code === "KeyC" || e.key === "c" || e.key === "C" || e.key === "\u0003" || e.keyCode === 67;
    const isX = e.code === "KeyX" || e.key === "x" || e.key === "X" || e.key === "\u0018" || e.keyCode === 88;
    const isV = e.code === "KeyV" || e.key === "v" || e.key === "V" || e.key === "\u0016" || e.keyCode === 86;
    const isA = e.code === "KeyA" || e.key === "a" || e.key === "A" || e.key === "\u0001" || e.keyCode === 65;

    if (e.ctrlKey && !e.shiftKey && isC) { e.preventDefault(); copySelected(); return; }
    if (e.ctrlKey && isX)                { e.preventDefault(); cutSelected();  return; }
    if (e.ctrlKey && isV)                { e.preventDefault(); pasteClipboard(); return; }

    const isF2 = e.key === "F2" || e.code === "F2" || e.keyCode === 113;
    if (isF2) {
      e.preventDefault();
      ctxRename();
      return;
    }

    const isDel = e.key === "Delete" || e.key === "Del" || e.code === "Delete" || e.keyCode === 46;
    if (isDel) {
      e.preventDefault();
      ctxDelete(e.shiftKey);
      return;
    }

    const isDown  = e.key === "ArrowDown"  || e.key === "Down"  || e.code === "ArrowDown"  || e.keyCode === 40;
    const isUp    = e.key === "ArrowUp"    || e.key === "Up"    || e.code === "ArrowUp"    || e.keyCode === 38;
    const isLeft  = e.key === "ArrowLeft"  || e.key === "Left"  || e.code === "ArrowLeft"  || e.keyCode === 37;
    const isRight = e.key === "ArrowRight" || e.key === "Right" || e.code === "ArrowRight" || e.keyCode === 39;

    if (!e.altKey && (isDown || isUp || isLeft || isRight)) {
      const items = displayEntries;
      if (!items.length) return;
      const currentList = [...selectedPaths];
      let currentIdx = -1;
      if (currentList.length > 0) {
        currentIdx = items.findIndex(en => en.path === currentList[currentList.length - 1]);
      }
      let step = 1;
      if (_viewMode === "grid") {
        const itemWidth = 120;
        const containerWidth = paneArea?.clientWidth || 800;
        const cols = Math.max(1, Math.floor(containerWidth / itemWidth));
        if (isDown) step = cols;
        else if (isUp) step = -cols;
        else if (isRight) step = 1;
        else if (isLeft) step = -1;
      } else {
        if (isDown || isRight) step = 1;
        else if (isUp || isLeft) step = -1;
      }
      let nextIdx = currentIdx === -1 ? 0 : Math.max(0, Math.min(items.length - 1, currentIdx + step));
      e.preventDefault();
      const nextEntry = items[nextIdx];
      selectedPaths = new Set([nextEntry.path]);
      previewEntry.set(nextEntry);
      requestAnimationFrame(() => {
        const el = paneFileArea?.querySelector(`[data-path="${CSS.escape(nextEntry.path)}"]`);
        if (el) el.scrollIntoView({ block: "nearest" });
      });
      return;
    }

    if (e.altKey && e.key === "ArrowLeft")  { e.preventDefault(); navigateBack(); }
    if (e.altKey && e.key === "ArrowRight") { e.preventDefault(); navigateForward(); }
    if (e.altKey && e.key === "ArrowUp")    { e.preventDefault(); navigateUp(); }
    if (e.key === "F5" && !e.shiftKey)      { e.preventDefault(); copyToOther(); }
    if (e.key === "F6")                     { e.preventDefault(); moveToOther(); }
    if (e.key === "F7")                     { e.preventDefault(); ctxNewFolder(); }
    if (e.ctrlKey && isA)                   { e.preventDefault(); selectedPaths = new Set(displayEntries.map(e => e.path)); }
    if (e.key === "Tab")                    { e.preventDefault(); dispatch("switch-focus"); }
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    const u = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  }

  function formatFolderSize(entry) {
    if (!entry.is_dir) return formatSize(entry.size);
    if (entry.size > 0) return formatSize(entry.size);
    if (entry.children_count !== undefined && entry.children_count !== null) {
      return `${entry.children_count} ${$t("items_count")}`;
    }
    return `0 ${$t("items_count")}`;
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
    const ext = (e.extension || "").toLowerCase();
    if (ext === "exe" || ext === "msi") return "🪟";
    if (ext === "bat" || ext === "cmd") return "📜";
    if (ext === "dll") return "🧩";
    const m = e.mime_type || "";
    if (m.includes("x-msdownload") || m.includes("x-msdos-program") || m.includes("x-dosexec")) return "🪟";
    if (m.startsWith("image/")) return "🖼";
    if (m.startsWith("video/")) return "🎬";
    if (m.startsWith("audio/")) return "🎵";
    if (m.includes("pdf"))      return "📄";
    if (m.startsWith("text/"))  return "📝";
    return "📄";
  }

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
    class:list-mode={_viewMode === "list"}
    bind:this={paneFileArea}
    on:scroll={checkRenderMore}
    on:click={clearSelection}
    on:contextmenu={e => onCtxMenu(e, null)}
    on:mousedown={onAreaMouseDown}
    on:dragover|preventDefault
    on:drop={onDropBg}
  >


    <!-- Selection rectangle -->
    {#if selRect && (selRect.w > 4 || selRect.h > 4)}
      <div
        class="sel-rect"
        style="left:{selRect.x}px; top:{selRect.y}px; width:{selRect.w}px; height:{selRect.h}px;"
      ></div>
    {/if}

    {#if isLoading}
      <div class="pane-state"><div class="pane-spinner"></div></div>
    {:else if loadError}
      <div class="pane-state error">⚠ {loadError}</div>
    {:else if displayEntries.length === 0}
      <div class="pane-state">◫ Boş</div>
    {:else if _viewMode === "grid"}
      <div class="pane-grid" style="grid-template-columns: repeat(auto-fill, minmax({$rightGridZoom}px, 1fr))">
        {#each renderedEntries as entry (entry.path)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="pane-grid-item"
            class:selected={selectedPaths.has(entry.path)}
            class:drag-over={dragOverPath === entry.path}
            data-path={entry.path}
            on:dblclick={() => onDblClick(entry)}
            on:contextmenu={e => onCtxMenu(e, entry)}
            draggable="true"
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
              onDrop(e, entry);
            }}
          >
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div
              class="pane-grid-icon"
              style="color:{getColor(entry.mime_type)}"
              on:mousedown={e => {
                if (e.button === 0 && !e.target.closest("input")) {
                  toggleSelect(e, entry.path);
                }
              }}
            >
              {#if showThumb(entry)}
                {#if isDirectThumb(entry)}
                  <img
                    class="pane-grid-thumb"
                    src={convertFileSrc(entry.path)}
                    alt={entry.name}
                    loading="lazy"
                    style="width:{Math.max(36, $rightGridZoom - 16)}px; height:{Math.max(36, $rightGridZoom - 16)}px; max-width:100%; object-fit:contain"
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
                  <img
                    class="pane-grid-thumb"
                    src={getCachedThumb(entry, thumbCache)}
                    alt={entry.name}
                    style="width:{Math.max(36, $rightGridZoom - 16)}px; height:{Math.max(36, $rightGridZoom - 16)}px; max-width:100%; object-fit:contain"
                  />
                {:else if IMAGE_EXTS.has((entry.extension || "").toLowerCase()) || entry.mime_type?.startsWith("image/")}
                  <img
                    class="pane-grid-thumb"
                    src={convertFileSrc(entry.path)}
                    alt={entry.name}
                    loading="lazy"
                    style="width:{Math.max(36, $rightGridZoom - 16)}px; height:{Math.max(36, $rightGridZoom - 16)}px; max-width:100%; object-fit:contain"
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
                  <span style="font-size:{Math.max(20, Math.min($rightGridZoom * 0.38, 48))}px">{getIcon(entry)}</span>
                {/if}
              {:else}
                <span style="font-size:{Math.max(20, Math.min($rightGridZoom * 0.38, 48))}px">{getIcon(entry)}</span>
              {/if}
            </div>
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
              <!-- svelte-ignore a11y-no-static-element-interactions -->
              <div
                class="pane-grid-name"
                on:mousedown={e => {
                  if (e.button === 0) {
                    toggleSelect(e, entry.path);
                  }
                }}
              >{entry.name}</div>
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
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span class="pane-col-name" style="cursor:pointer" on:click={() => toggleSort("name")}>
            {$t("name")} {#if $sortBy === "name"}<span style="color:var(--accent); font-size:9px">{$sortAsc ? "▲" : "▼"}</span>{/if}
          </span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span class="pane-col-size" style="cursor:pointer" on:click={() => toggleSort("size")}>
            {$t("size")} {#if $sortBy === "size"}<span style="color:var(--accent); font-size:9px">{$sortAsc ? "▲" : "▼"}</span>{/if}
          </span>
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <span class="pane-col-date" style="cursor:pointer" on:click={() => toggleSort("modified")}>
            {$t("modified")} {#if $sortBy === "modified" || $sortBy === "date"}<span style="color:var(--accent); font-size:9px">{$sortAsc ? "▲" : "▼"}</span>{/if}
          </span>
        </div>
        {#each renderedEntries as entry (entry.path)}
          <!-- svelte-ignore a11y-no-static-element-interactions -->
          <div
            class="pane-list-row"
            class:selected={selectedPaths.has(entry.path)}
            class:drag-over={dragOverPath === entry.path}
            data-path={entry.path}
            on:dblclick={() => onDblClick(entry)}
            on:contextmenu={e => onCtxMenu(e, entry)}
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
              onDrop(e, entry);
            }}
          >
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <span
              class="pane-col-name"
              draggable={paneRenamingPath !== entry.path}
              on:dragstart={e => onDragStart(e, entry)}
              on:mousedown={e => {
                if (e.button === 0 && !e.target.closest("input")) {
                  toggleSelect(e, entry.path);
                }
              }}
            >
              {#if showThumb(entry) && isThumbable(entry)}
                {#if isDirectThumb(entry)}
                  <img
                    class="pane-list-thumb"
                    src={convertFileSrc(entry.path)}
                    alt=""
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
                  <img class="pane-list-thumb" src={getCachedThumb(entry, thumbCache)} alt="" />
                {:else}
                  <span style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
                {/if}
              {:else}
                <span style="color:{getColor(entry.mime_type)}">{getIcon(entry)}</span>
              {/if}
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
            <span class="pane-col-size" title={entry.is_dir ? (entry.children_count != null ? `${entry.children_count} öğe` : "") : formatSize(entry.size)}>{formatFolderSize(entry)}</span>
            <span class="pane-col-date">{entry.modified}</span>
          </div>
        {/each}
      </div>
    {/if}

    {#if renderLimit < displayEntries.length}
      <div class="virtual-batch-pill">
        ⚡ {renderLimit} / {displayEntries.length} dosya yüklendi (Kaydırarak devam edin)
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

        {#if ctxMenu.target.is_dir}
          <button class="pane-ctx-item" on:click={() => { navigateTo(ctxMenu.target.path); closeCtx(); }}>
            <span class="pane-ctx-icon">▶</span>
            <span class="pane-ctx-title">Open</span>
          </button>
          <button class="pane-ctx-item" on:click={() => { dispatch("open-in-new-tab", { path: ctxMenu.target.path }); closeCtx(); }}>
            <span class="pane-ctx-icon">⊞</span>
            <span class="pane-ctx-title">Open in Left Tab</span>
          </button>
        {:else}
          <button class="pane-ctx-item" on:click={() => { onDblClick(ctxMenu.target); closeCtx(); }}>
            <span class="pane-ctx-icon">▶</span>
            <span class="pane-ctx-title">Open</span>
          </button>
        {/if}

        {#if /\.(zip|tar\.gz|tgz|tar\.bz2|tbz2|tar\.xz|txz|tar\.zst|tar|7z|rar)$/i.test(ctxMenu.target.name)}
          <button class="pane-ctx-item" on:click={ctxExtract}>
            <span class="pane-ctx-icon">📦</span>
            <span class="pane-ctx-title">Buraya Çıkar (Extract Here)</span>
          </button>
        {/if}

        <button class="pane-ctx-item" on:click={ctxCopy}>
          <span class="pane-ctx-icon">⎘</span>
          <span class="pane-ctx-title">Copy</span>
          <span class="pane-ctx-sc">Ctrl+C</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxCut}>
          <span class="pane-ctx-icon">✂</span>
          <span class="pane-ctx-title">Cut</span>
          <span class="pane-ctx-sc">Ctrl+X</span>
        </button>
        {#if $clipboard.paths.length > 0}
          <button class="pane-ctx-item" on:click={ctxPaste}>
            <span class="pane-ctx-icon">📋</span>
            <span class="pane-ctx-title">Paste</span>
            <span class="pane-ctx-sc">Ctrl+V</span>
          </button>
        {/if}
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

        <div class="pane-ctx-sep"></div>

        <button class="pane-ctx-item" on:click={() => { navigator.clipboard.writeText(ctxMenu.target.path); notify("success","Path copied"); closeCtx(); }}>
          <span class="pane-ctx-icon">◈</span>
          <span class="pane-ctx-title">Copy Path</span>
        </button>
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

        {#if inTrash}
          <button class="pane-ctx-item" on:click={ctxRestore}>
            <span class="pane-ctx-icon">⤺</span>
            <span class="pane-ctx-title">{$t("restore")}</span>
          </button>
          <button class="pane-ctx-item danger" on:click={() => ctxDelete(true)}>
            <span class="pane-ctx-icon">✕</span>
            <span class="pane-ctx-title">{$t("permanent_delete")}</span>
            <span class="pane-ctx-sc">Del</span>
          </button>
          <div class="pane-ctx-sep"></div>
          <button class="pane-ctx-item" on:click={async () => { await restoreAllTrashItems(); await loadDir(currentPath); closeCtx(); }}>
            <span class="pane-ctx-icon">⤺</span>
            <span class="pane-ctx-title">{$t("restore_all")}</span>
          </button>
          <button class="pane-ctx-item danger" on:click={async () => { await emptyTrashNow(); await loadDir(currentPath); closeCtx(); }}>
            <span class="pane-ctx-icon">🗑</span>
            <span class="pane-ctx-title">{$t("empty_trash")}</span>
          </button>
        {:else}
          <button class="pane-ctx-item" on:click={() => ctxDelete(false)}>
            <span class="pane-ctx-icon">🗑</span>
            <span class="pane-ctx-title">{$t("move_to_trash")}</span>
            <span class="pane-ctx-sc">Del</span>
          </button>
          <button class="pane-ctx-item danger" on:click={() => ctxDelete(true)}>
            <span class="pane-ctx-icon">✕</span>
            <span class="pane-ctx-title">{$t("delete")}</span>
            <span class="pane-ctx-sc">Shift+Del</span>
          </button>
        {/if}

        <div class="pane-ctx-sep"></div>

        <!-- More Options -->
        <button class="pane-ctx-item" on:click={() => showMoreCtx = !showMoreCtx}>
          <span class="pane-ctx-icon">⋯</span>
          <span class="pane-ctx-title">{$t("more_options")}</span>
          <span class="pane-ctx-sc">{showMoreCtx ? "▲" : "▼"}</span>
        </button>

        {#if showMoreCtx}
          <div class="pane-ctx-sub">
            <button class="pane-ctx-item" on:click={() => { previewEntry.set(ctxMenu.target); showPreview.set(true); closeCtx(); }}>
              <span class="pane-ctx-icon">◫</span>
              <span class="pane-ctx-title">Preview</span>
              <span class="pane-ctx-sc">F3</span>
            </button>
            <button class="pane-ctx-item" on:click={ctxCompress}>
              <span class="pane-ctx-icon">🗜</span>
              <span class="pane-ctx-title">Compress…</span>
            </button>
            {#if !ctxMenu.target.is_dir}
              <button class="pane-ctx-item" on:click={ctxChecksum}>
                <span class="pane-ctx-icon">⊛</span>
                <span class="pane-ctx-title">Checksum</span>
              </button>
            {/if}
            <button class="pane-ctx-item" on:click={ctxTerminal}>
              <span class="pane-ctx-icon">⬛</span>
              <span class="pane-ctx-title">Open Terminal Here</span>
              <span class="pane-ctx-sc">Ctrl+T</span>
            </button>

            {#if $userCommands.filter(c => c.show_in_context_menu).length > 0}
              <div class="pane-ctx-sep"></div>
              {#each $userCommands.filter(c => c.show_in_context_menu) as cmd}
                <button class="pane-ctx-item" on:click={() => ctxRunCmd(cmd)}>
                  <span class="pane-ctx-icon">⚡</span>
                  <span class="pane-ctx-title">{cmd.name}</span>
                </button>
              {/each}
            {/if}
          </div>
        {/if}

      {:else}
        <!-- Background click -->
        <div class="pane-ctx-label">This Folder</div>
        <div class="pane-ctx-sep"></div>
        <button class="pane-ctx-item" on:click={ctxNewFolder}>
          <span class="pane-ctx-icon">📁</span>
          <span class="pane-ctx-title">New Folder</span>
          <span class="pane-ctx-sc">F7</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxNewFile}>
          <span class="pane-ctx-icon">📄</span>
          <span class="pane-ctx-title">New File</span>
        </button>
        {#if $clipboard.paths.length > 0}
          <button class="pane-ctx-item" on:click={ctxPaste}>
            <span class="pane-ctx-icon">📋</span>
            <span class="pane-ctx-title">Paste</span>
            <span class="pane-ctx-sc">Ctrl+V</span>
          </button>
        {/if}
        <button class="pane-ctx-item" on:click={() => { navigator.clipboard.writeText(currentPath); notify("success","Path copied"); closeCtx(); }}>
          <span class="pane-ctx-icon">◈</span>
          <span class="pane-ctx-title">Copy Path</span>
        </button>
        <button class="pane-ctx-item" on:click={ctxTerminal}>
          <span class="pane-ctx-icon">⬛</span>
          <span class="pane-ctx-title">Open Terminal Here</span>
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
  .pane-file-area { flex: 1; overflow-y: auto; overflow-x: hidden; padding: 6px; position: relative; overflow-anchor: auto; }
  .pane-file-area.list-mode { padding: 0 0 6px 0; }
  .pane-state { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-dim); font-size: 12px; gap: 10px; flex-direction: column; }
  .pane-state.error { color: var(--danger); }
  .pane-spinner { width: 22px; height: 22px; border: 2px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: spin 0.7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* Selection rectangle */
  .sel-rect {
    position: absolute;
    pointer-events: none;
    background: rgba(130,100,255,0.15);
    border: 1px solid rgba(130,100,255,0.55);
    border-radius: 4px;
    z-index: 50;
  }

  /* Grid */
  .pane-grid { display: grid; gap: 8px; padding: 4px; }
  .pane-grid-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 6px 4px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
    text-align: center;
    user-select: none;
    background: transparent;
  }
  .pane-grid-item.selected .pane-grid-icon {
    outline: 2px solid var(--border-accent, #8264ff);
    outline-offset: 3px;
    background: rgba(130, 100, 255, 0.22);
    border-radius: 6px;
    box-shadow: 0 0 12px rgba(130, 100, 255, 0.4);
  }
  .pane-grid-item.selected .pane-grid-name {
    background: var(--accent, #8264ff);
    color: #fff;
    border-radius: 4px;
    padding: 1px 6px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.4);
  }
  .pane-grid-item:hover:not(.selected) .pane-grid-icon {
    background: var(--bg-hover, rgba(255,255,255,0.06));
    border-radius: 6px;
  }
  .pane-grid-item:hover:not(.selected) .pane-grid-name {
    background: var(--bg-hover, rgba(255,255,255,0.08));
    border-radius: 4px;
    padding: 1px 6px;
  }
  .pane-grid-icon {
    line-height: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    transition: all 0.15s ease;
  }
  .pane-grid-thumb {
    object-fit: cover;
    border-radius: 4px;
    display: block;
  }
  .pane-list-thumb {
    width: 20px;
    height: 20px;
    object-fit: cover;
    border-radius: 3px;
    flex-shrink: 0;
  }
  .pane-grid-name { font-size: 10px; color: var(--text-primary); width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pane-grid-size { font-size: 9px; color: var(--text-dim); }

  /* List */
  .pane-list { display: flex; flex-direction: column; width: 100%; }
  .pane-list-header {
    display: grid;
    grid-template-columns: 1fr 70px 120px;
    padding: 6px 8px;
    font-size: 10px;
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: rgba(18, 18, 28, 0.98);
    z-index: 25;
    box-sizing: border-box;
  }
  .pane-list-row { display: grid; grid-template-columns: 1fr 70px 120px; align-items: center; padding: 4px 6px; border-radius: var(--radius-sm); border: 1px solid transparent; cursor: pointer; margin-bottom: 2px; }
  .pane-list-row:hover { background: var(--bg-hover); }
  .pane-list-row.selected { background: var(--bg-selected); border-color: var(--border-accent); }
  .pane-list-row.drag-over,
  .pane-grid-item.drag-over {
    background: rgba(130, 100, 255, 0.28) !important;
    outline: 2px dashed var(--accent, #8264ff) !important;
    outline-offset: -2px;
    box-shadow: 0 0 16px rgba(130, 100, 255, 0.5) !important;
  }
  .pane-col-name { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: 11px; cursor: grab; }
  .pane-col-size, .pane-col-date { font-size: 10px; color: var(--text-secondary); }
  .pane-name-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* Context menu */
  .pane-ctx {
    position: fixed; z-index: 9999;
    min-width: 230px;
    border-radius: var(--radius);
    padding: 5px;
    box-shadow: 0 12px 36px rgba(0,0,0,0.55), 0 0 0 1px rgba(255,255,255,0.08);
    background: var(--bg-panel, rgba(20, 20, 28, 0.94));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    
    
    user-select: none;
    max-height: 90vh;
    overflow-y: auto;
  }
  .pane-ctx-sub {
    margin-left: 6px;
    border-left: 1px solid var(--border, rgba(255,255,255,0.1));
    padding-left: 4px;
    margin-top: 2px;
    margin-bottom: 2px;
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

  .virtual-batch-pill {
    position: sticky;
    bottom: 8px;
    margin: 8px auto;
    width: fit-content;
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 500;
    color: var(--accent, #6366f1);
    background: rgba(18, 18, 26, 0.95);
    border: 1px solid rgba(99, 102, 241, 0.35);
    border-radius: 9999px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    z-index: 10;
    pointer-events: none;
    animation: fadeIn 0.15s ease-out;
  }

</style>
