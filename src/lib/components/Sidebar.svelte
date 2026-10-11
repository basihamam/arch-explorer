<script>
  import { invoke } from "@tauri-apps/api/core";
  import {
    bookmarks, bookmarkIconsEnabled, blockDevices, drives,
    navigateTo, currentPath,
    addBookmark, removeBookmark, loadBookmarks, notify,
    addTab, loadBlockDevices, showDevices, selectedDevice,
    activePaneId, rightPaneNavRequest, safeBatchFileOp,
    hiddenDevices, deviceOrder, pinnedDevices,
  } from "../stores/explorer.js";
  import { t, currentLocale } from "../i18n.js";
  import { get } from "svelte/store";

  // ---- Event: MountPanel'i aç ----
  // Sidebar'dan unmounted diske tıklanınca App.svelte'e haber ver
  import { createEventDispatcher, onMount, onDestroy } from "svelte";
  const dispatch = createEventDispatcher();

  let devicePollTimer;
  onMount(() => {
    devicePollTimer = setInterval(() => {
      loadBlockDevices();
    }, 3000);
  });
  onDestroy(() => {
    if (devicePollTimer) clearInterval(devicePollTimer);
  });

  function goToPath(path) {
    showDevices.set(false);
    if (get(activePaneId) === "right") {
      rightPaneNavRequest.set({ type: "navigate", path });
    } else {
      navigateTo(path);
    }
  }

  // ---- Drag ----
  let dragOverPath = null;
  let draggedBm = null;
  let bmDropTarget = null; // { path: string, position: 'before' | 'after' }

  // Device Drag & Drop sıralama
  let draggedDev = null;
  let devDropTarget = null;

  // Mount değişiminde bookmarks'ı yenile
  // (yeni disk mount edilince /run/media altı bookmark'larda görünsün)
  let _prevDeviceStr = "";
  blockDevices.subscribe(devs => {
    const str = devs.map(d => d.device + (d.mount_point || "")).join("|");
    if (str !== _prevDeviceStr) {
      _prevDeviceStr = str;
      loadBookmarks();
    }
  });

  function handleBmDragStart(e, bm) {
    if (bm.pinned) return;
    draggedBm = bm;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("application/x-bookmark-path", bm.path);
  }

  function handleBmDragEnd() {
    draggedBm = null;
    bmDropTarget = null;
    dragOverPath = null;
  }

  function handleBmDragOver(e, bm) {
    e.preventDefault();
    if (draggedBm) {
      if (draggedBm.path === bm.path) {
        bmDropTarget = null;
        return;
      }
      const rect = e.currentTarget.getBoundingClientRect();
      const midY = rect.top + rect.height / 2;
      const position = e.clientY < midY ? "before" : "after";
      bmDropTarget = { path: bm.path, position };
      dragOverPath = null;
    } else {
      dragOverPath = bm.path;
      bmDropTarget = null;
    }
  }

  async function handleBmDrop(e, targetBm) {
    e.preventDefault();
    const isBmReorder = !!draggedBm || !!e.dataTransfer.getData("application/x-bookmark-path");
    const currentDragged = draggedBm;
    const currentDropTarget = bmDropTarget;
    draggedBm = null;
    bmDropTarget = null;
    dragOverPath = null;

    if (isBmReorder && currentDragged && currentDropTarget) {
      const list = [...$bookmarks];
      const fromIdx = list.findIndex(b => b.path === currentDragged.path);
      if (fromIdx === -1) return;
      const [item] = list.splice(fromIdx, 1);
      let toIdx = list.findIndex(b => b.path === currentDropTarget.path);
      if (toIdx === -1) {
        list.push(item);
      } else {
        if (currentDropTarget.position === "after") {
          toIdx++;
        }
        list.splice(toIdx, 0, item);
      }

      const orderedPaths = list.map(b => b.path);
      try {
        await invoke("reorder_bookmarks", { orderedPaths });
        await loadBookmarks();
        notify("success", $t("bookmarks") + " updated");
      } catch (err) {
        notify("error", String(err));
      }
      return;
    }

    await handleDropOnBookmark(e, targetBm);
  }

  async function handleDropOnBookmark(e, bm) {
    e.preventDefault(); dragOverPath = null;
    const src = e.dataTransfer.getData("text/plain");
    if (!src || src === bm.path) return;
    await safeBatchFileOp([src], bm.path, "move");
    goToPath($currentPath);
  }

  async function handleDropOnDevice(e, mp) {
    e.preventDefault(); dragOverPath = null;
    const src = e.dataTransfer.getData("text/plain");
    if (!src || !mp) return;
    await safeBatchFileOp([src], mp, "move");
    goToPath($currentPath);
  }

  async function promptAddBookmark() {
    const name = prompt("Bookmark name:", $currentPath.split("/").filter(Boolean).pop() || "root");
    if (!name) return;
    await addBookmark(name, $currentPath);
  }

  // ---- Disk Yönetimi & Sıralama ----
  let showHiddenDevs = false;
  let devMenu = null; // { dev, x, y }

  function handleDevDragStart(e, dev) {
    draggedDev = dev;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("application/x-device-id", dev.device);
  }

  function handleDevDragEnd() {
    draggedDev = null;
    devDropTarget = null;
    dragOverPath = null;
  }

  function handleDevDragOver(e, dev) {
    e.preventDefault();
    if (draggedDev) {
      if (draggedDev.device === dev.device) {
        devDropTarget = null;
        return;
      }
      const rect = e.currentTarget.getBoundingClientRect();
      const midY = rect.top + rect.height / 2;
      const position = e.clientY < midY ? "before" : "after";
      devDropTarget = { device: dev.device, position };
    }
  }

  function handleDevDrop(e, targetDev) {
    e.preventDefault();
    if (!draggedDev || !devDropTarget) {
      if (targetDev.is_mounted && targetDev.mount_point) {
        handleDropOnDevice(e, targetDev.mount_point);
      }
      return;
    }
    const currentOrder = processedDevices.map(d => d.device);
    const fromIdx = currentOrder.indexOf(draggedDev.device);
    if (fromIdx !== -1) {
      currentOrder.splice(fromIdx, 1);
      let toIdx = currentOrder.indexOf(devDropTarget.device);
      if (toIdx !== -1) {
        if (devDropTarget.position === "after") toIdx++;
        currentOrder.splice(toIdx, 0, draggedDev.device);
      } else {
        currentOrder.push(draggedDev.device);
      }
      deviceOrder.set(currentOrder);
      notify("success", $t("devices") + " order saved");
    }
    draggedDev = null;
    devDropTarget = null;
  }

  function openDevMenu(e, dev) {
    e.preventDefault();
    e.stopPropagation();
    devMenu = { dev, x: e.clientX, y: e.clientY };
  }

  function closeDevMenu() { devMenu = null; }

  function togglePinDevice(dev) {
    pinnedDevices.update(pins => {
      if (pins.includes(dev.device)) {
        return pins.filter(p => p !== dev.device);
      } else {
        return [...pins, dev.device];
      }
    });
    closeDevMenu();
  }

  function hideDevice(dev) {
    hiddenDevices.update(h => [...new Set([...h, dev.device])]);
    closeDevMenu();
    notify("info", `${devLabel(dev)} hidden`);
  }

  function unhideDevice(dev) {
    hiddenDevices.update(h => h.filter(id => id !== dev.device));
  }

  function unhideAllDevices() {
    hiddenDevices.set([]);
    notify("success", "All devices visible");
  }

  async function unmountDev(dev) {
    closeDevMenu();
    try {
      await invoke("unmount_device", { device: dev.device });
      notify("success", `${devLabel(dev)} unmounted`);
      loadBlockDevices();
    } catch (e) {
      notify("error", String(e));
    }
  }

  // Sıralanmış ve filtrelenmiş disk listesi
  $: processedDevices = (() => {
    let list = [...$blockDevices];
    if (!showHiddenDevs) {
      list = list.filter(d => !$hiddenDevices.includes(d.device));
    }
    // Sıralama mantığı: Pinned olanlar en üstte, sonra deviceOrder, sonra varsayılan
    const pins = $pinnedDevices;
    const order = $deviceOrder;
    list.sort((a, b) => {
      const aPin = pins.includes(a.device);
      const bPin = pins.includes(b.device);
      if (aPin && !bPin) return -1;
      if (!aPin && bPin) return 1;

      const aIdx = order.indexOf(a.device);
      const bIdx = order.indexOf(b.device);
      if (aIdx !== -1 && bIdx !== -1) return aIdx - bIdx;
      if (aIdx !== -1) return -1;
      if (bIdx !== -1) return 1;
      return 0;
    });
    return list;
  })();

  // ---- Devices collapse (5+ ise) ----
  let devicesExpanded = false;
  $: visibleDevices = devicesExpanded || processedDevices.length <= 5
    ? processedDevices
    : processedDevices.slice(0, 5);

  function handleDeviceClick(dev) {
    if (dev.is_mounted && dev.mount_point) {
      goToPath(dev.mount_point);
    } else {
      dispatch("open-mounts", { device: dev.device });
    }
  }

  function openDevicesView() {
    showDevices.set(true);
    selectedDevice.set(null);
  }

  // ---- Bookmark sağ tık dropdown ----
  let bmMenu = null;

  function openBmMenu(e, bm) {
    e.preventDefault();
    e.stopPropagation();
    bmMenu = { bm, x: e.clientX, y: e.clientY };
  }

  function closeBmMenu() { bmMenu = null; }

  async function renameBm(bm) {
    closeBmMenu();
    const newName = prompt("New name:", bm.name);
    if (!newName || newName === bm.name) return;
    try {
      await invoke("update_bookmark", { path: bm.path, name: newName, icon: null, pinned: null, color: null });
      await loadBookmarks();
      notify("success", `Renamed: ${newName}`);
    } catch (e) { notify("error", String(e)); }
  }

  async function pinBm(bm) {
    closeBmMenu();
    try {
      await invoke("update_bookmark", { path: bm.path, name: null, icon: null, pinned: !bm.pinned, color: null });
      await loadBookmarks();
    } catch (e) { notify("error", String(e)); }
  }

  async function removeBm(bm) {
    closeBmMenu();
    if (!confirm(`Remove "${bm.name}" from bookmarks?`)) return;
    await removeBookmark(bm.path);
  }

  function onWindowMousedown() {
    if (bmMenu) closeBmMenu();
    if (devMenu) closeDevMenu();
  }

  // ---- Bookmark icon ----
  function bmIcon(bm) {
    if (!$bookmarkIconsEnabled) {
      return bm.pinned ? "◈" : "⊹";
    }
    const name = (bm.name || "").toLowerCase();
    const icon = (bm.icon || "").toLowerCase();
    if (name === "trash" || icon === "trash" || bm.path?.includes(".local/share/Trash")) return "🗑️";
    if (name === "home" || icon === "home") return "🏠";
    if (name === "desktop" || icon === "desktop") return "🖥️";
    if (name === "downloads" || icon === "download") return "📥";
    if (name === "documents" || icon === "document") return "📄";
    if (name === "pictures" || icon === "image") return "🖼️";
    if (name === "music" || icon === "music") return "🎵";
    if (name === "videos" || icon === "video") return "🎬";
    if (name === "root" || icon === "server") return "⚡";
    if (name === "media" || icon === "hdd") return "💾";
    return bm.pinned ? "◈" : "⊹";
  }

  // ---- Disk icon ----
  function devIcon(dev) {
    if (dev.fs_type === "adb") return "📱";
    if ($pinnedDevices.includes(dev.device)) return "📌";
    if (dev.is_windows)  return "🪟";
    if (dev.is_mounted)  return "◉";
    return "○";
  }

  function devLabel(dev) {
    if (dev.label && dev.label !== dev.name) return dev.label;
    return dev.name || dev.device;
  }
</script>

<svelte:window on:mousedown={onWindowMousedown} />

<aside class="sidebar glass-panel">

  <!-- ── Bookmarks ── -->
  <div class="section">
    <div class="section-header">
      <span>{$t("bookmarks")}</span>
      <button class="add-btn" on:click={promptAddBookmark} title={$t("add_bookmark")}>+</button>
    </div>

    {#each $bookmarks as bm (bm.path)}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="sidebar-item"
        class:active={$currentPath === bm.path}
        class:drag-over={dragOverPath === bm.path}
        class:pinned={bm.pinned}
        class:is-dragging={draggedBm?.path === bm.path}
        class:drop-before={bmDropTarget?.path === bm.path && bmDropTarget?.position === 'before'}
        class:drop-after={bmDropTarget?.path === bm.path && bmDropTarget?.position === 'after'}
        draggable={!bm.pinned}
        on:dragstart={e => handleBmDragStart(e, bm)}
        on:dragend={handleBmDragEnd}
        on:click={() => goToPath(bm.path)}
        on:contextmenu={e => openBmMenu(e, bm)}
        on:dragover|preventDefault={e => handleBmDragOver(e, bm)}
        on:dragleave={() => { if (!draggedBm) dragOverPath = null; }}
        on:drop={e => handleBmDrop(e, bm)}
        title={bm.path + (!bm.pinned ? " (drag to reorder)" : " (pinned)")}
        role="button"
        tabindex="0"
        on:keydown={e => e.key === "Enter" && goToPath(bm.path)}
      >
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <span
          class="item-icon bm-icon-btn"
          style="font-size: 13px"
          title={bm.pinned ? $t("unpin") : $t("pin_to_top")}
          on:click|stopPropagation={() => pinBm(bm)}
        >
          {bmIcon(bm)}
        </span>
        <span class="item-label">{bm.name}</span>
        {#if !bm.pinned}
          <span class="drag-handle-hint" title="Drag to reorder">⋮⋮</span>
        {/if}
      </div>
    {/each}

    {#if $bookmarks.length === 0}
      <div class="empty-hint">{$t("no_bookmarks")}</div>
    {/if}
  </div>

  <!-- ── Devices (lsblk tabanlı — mount edilmemiş diskler de görünür) ── -->
  <div class="section">
    <div class="section-header">
      <span>{$t("devices")}</span>
      <div style="display:flex;gap:4px;align-items:center;">
        {#if $hiddenDevices.length > 0}
          <button
            class="add-btn hidden-badge-btn"
            on:click={() => showHiddenDevs = !showHiddenDevs}
            title={showHiddenDevs ? "Gizlileri Kapat" : `${$hiddenDevices.length} Gizli Disk (Görmek için tıkla)`}
          >
            {showHiddenDevs ? "👁" : "👁‍🗨"}
          </button>
        {/if}
        <button class="add-btn" on:click={openDevicesView} title={$t("all_devices")}>◉</button>
      </div>
    </div>

    <!-- Devices View girişi — Windows "Bu Bilgisayar" gibi -->
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div
      class="sidebar-item"
      class:active={$showDevices}
      on:click={openDevicesView}
      role="button" tabindex="0"
      on:keydown={e => e.key === "Enter" && openDevicesView()}
    >
      <span class="item-icon" style="font-size:13px">◉</span>
      <span class="item-label">{$t("all_devices")}</span>
    </div>

    <div class="ctx-sep-small"></div>

    {#each visibleDevices as dev (dev.device)}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="sidebar-item dev-item"
        class:active={!$showDevices && dev.is_mounted && dev.mount_point && $currentPath.startsWith(dev.mount_point)}
        class:drag-over={dragOverPath === (dev.mount_point || dev.device)}
        class:unmounted={!dev.is_mounted}
        class:pinned={$pinnedDevices.includes(dev.device)}
        class:is-hidden-dev={$hiddenDevices.includes(dev.device)}
        draggable="true"
        on:dragstart={e => handleDevDragStart(e, dev)}
        on:dragend={handleDevDragEnd}
        on:dragover={e => handleDevDragOver(e, dev)}
        on:drop={e => handleDevDrop(e, dev)}
        on:click={() => handleDeviceClick(dev)}
        on:contextmenu={e => openDevMenu(e, dev)}
        role="button" tabindex="0"
        on:keydown={e => e.key === "Enter" && handleDeviceClick(dev)}
        title="{dev.device} · {dev.fs_type} · {dev.size}{dev.is_mounted ? ' · ' + dev.mount_point : ' · unmounted'}"
      >
        <span class="item-icon dev-icon">{devIcon(dev)}</span>
        <div class="dev-text">
          <span class="item-label">
            {devLabel(dev)}
            {#if $hiddenDevices.includes(dev.device)}
              <span class="hidden-pill">gizli</span>
            {/if}
          </span>
          <span class="dev-sub">{dev.size} · {dev.fs_type}{!dev.is_mounted ? " · click to mount" : ""}</span>
        </div>
        <span class="drag-handle-hint dev-drag-hint" title="Sürükle sırala / Sağ tıkla">⋮⋮</span>
      </div>
    {/each}

    {#if processedDevices.length > 5}
      <button class="expand-btn" on:click={() => devicesExpanded = !devicesExpanded}>
        {devicesExpanded ? `▲ ${$t("less_devices")}` : `▼ +${processedDevices.length - 5} ${$t("more_devices")}`}
      </button>
    {/if}

    {#if showHiddenDevs && $hiddenDevices.length > 0}
      <button class="unhide-all-btn" on:click={unhideAllDevices}>
        ↺ {$t("show_hidden_devices")} ({$hiddenDevices.length})
      </button>
    {/if}

    {#if $blockDevices.length === 0}
      <div class="empty-hint">{$t("no_devices")}</div>
    {/if}
  </div>

</aside>

<!-- ── Bookmark dropdown ── -->
{#if bmMenu}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="bm-dropdown glass"
    style="left:{bmMenu.x}px; top:{bmMenu.y}px;"
    on:mousedown|stopPropagation
    on:click|stopPropagation
  >
    <button class="bm-action" on:click={() => { goToPath(bmMenu.bm.path); closeBmMenu(); }}>
      <span>▶</span> {$t("open_folder")}
    </button>
    <button class="bm-action" on:click={() => { addTab(bmMenu.bm.path); closeBmMenu(); }}>
      <span>⊞</span> {$t("open_in_new_tab")}
    </button>
    <button class="bm-action" on:click={() => renameBm(bmMenu.bm)}>
      <span>✏</span> {$t("rename")}
    </button>
    <button class="bm-action" on:click={() => pinBm(bmMenu.bm)}>
      <span>{bmMenu.bm.pinned ? "○" : "◈"}</span>
      {bmMenu.bm.pinned ? $t("unpin") : $t("pin_to_top")}
    </button>
    <div class="bm-sep"></div>
    <button class="bm-action danger" on:click={() => removeBm(bmMenu.bm)}>
      <span>✕</span> {$t("remove_bookmark")}
    </button>
  </div>
{/if}

<!-- ── Device (Disk) dropdown menu ── -->
{#if devMenu}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="bm-dropdown glass"
    style="left:{devMenu.x}px; top:{devMenu.y}px;"
    on:mousedown|stopPropagation
    on:click|stopPropagation
  >
    {#if devMenu.dev.is_mounted && devMenu.dev.mount_point}
      <button class="bm-action" on:click={() => { goToPath(devMenu.dev.mount_point); closeDevMenu(); }}>
        <span>▶</span> {$t("go_to_disk")}
      </button>
      <button class="bm-action" on:click={() => { addTab(devMenu.dev.mount_point); closeDevMenu(); }}>
        <span>⊞</span> {$t("open_in_new_tab")}
      </button>
    {:else}
      <button class="bm-action" on:click={() => { dispatch("open-mounts", { device: devMenu.dev.device }); closeDevMenu(); }}>
        <span>⚡</span> {$t("mount_disk")}
      </button>
    {/if}

    <button class="bm-action" on:click={() => togglePinDevice(devMenu.dev)}>
      <span>{$pinnedDevices.includes(devMenu.dev.device) ? "📌" : "📍"}</span>
      {$pinnedDevices.includes(devMenu.dev.device) ? $t("unpin") : $t("pin_to_top")}
    </button>

    {#if devMenu.dev.is_mounted && devMenu.dev.mount_point}
      <button class="bm-action" on:click={async () => {
        closeDevMenu();
        await addBookmark(devLabel(devMenu.dev), devMenu.dev.mount_point);
      }}>
        <span>★</span> {$t("bookmarks") + " (" + $t("add_bookmark") + ")"}
      </button>
    {/if}

    <div class="bm-sep"></div>

    {#if $hiddenDevices.includes(devMenu.dev.device)}
      <button class="bm-action" on:click={() => { unhideDevice(devMenu.dev); closeDevMenu(); }}>
        <span>👁</span> {$t("show_hidden_devices")}
      </button>
    {:else}
      <button class="bm-action" on:click={() => hideDevice(devMenu.dev)}>
        <span>👁‍🗨</span> {$t("hide_from_sidebar")}
      </button>
    {/if}

    {#if devMenu.dev.is_mounted}
      <button class="bm-action danger" on:click={() => unmountDev(devMenu.dev)}>
        <span>⏏</span> {$t("unmount_disk")}
      </button>
    {/if}
  </div>
{/if}

<style>
  .sidebar {
    width: var(--sidebar-width, 250px);
    min-width: 160px;
    max-width: 550px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    overflow-x: hidden;
    border-right: 1px solid var(--border, rgba(255,255,255,0.08));
    border-top: none; border-bottom: none; border-left: none;
    border-radius: 0;
    padding: 8px 6px;
    gap: 4px;
    background: var(--bg-secondary, rgba(22,22,30,0.87));
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-bottom: 8px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 8px;
    font-size: 10px;
    font-weight: 700;
    color: var(--text-dim, rgba(255,255,255,0.35));
    text-transform: uppercase;
    letter-spacing: 0.1em;
    user-select: none;
  }

  .add-btn {
    background: none; border: none;
    color: var(--text-dim, rgba(255,255,255,0.35));
    cursor: pointer; font-size: 16px; line-height: 1; padding: 0 2px;
    transition: color 0.15s;
  }
  .add-btn:hover { color: var(--accent, #8264ff); }

  .sidebar-item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 10px;
    border-radius: var(--radius-sm, 6px);
    border: 1px solid transparent;
    background: none;
    color: var(--text-secondary, rgba(255,255,255,0.55));
    font-size: 13px;
    cursor: pointer;
    transition: all 0.22s cubic-bezier(0.16, 1, 0.3, 1);
    width: 100%;
    user-select: none;
    min-width: 0;
  }
  .sidebar-item:hover {
    background: var(--bg-hover, rgba(255,255,255,0.06));
    color: var(--text-primary, rgba(255,255,255,0.9));
    transform: translateX(3px);
  }
  .sidebar-item.active {
    background: var(--accent-dim, rgba(130,100,255,0.15));
    border-color: var(--border-accent, rgba(130,100,255,0.35));
    color: var(--text-accent, #8264ff);
    transform: translateX(2px);
  }
  .sidebar-item.drag-over {
    background: var(--accent-dim, rgba(130,100,255,0.15));
    border-color: var(--accent, #8264ff);
    border-style: dashed;
  }
  .sidebar-item.is-dragging {
    opacity: 0.35;
    background: rgba(130,100,255,0.1);
  }
  .sidebar-item.drop-before {
    border-top: 2px solid var(--accent, #8264ff) !important;
    background: rgba(130,100,255,0.08);
  }
  .sidebar-item.drop-after {
    border-bottom: 2px solid var(--accent, #8264ff) !important;
    background: rgba(130,100,255,0.08);
  }
  .drag-handle-hint {
    margin-left: auto;
    font-size: 11px;
    letter-spacing: -1px;
    color: var(--text-dim, rgba(255,255,255,0.25));
    opacity: 0;
    transition: opacity 0.15s ease;
    cursor: grab;
    user-select: none;
  }
  .sidebar-item:hover .drag-handle-hint {
    opacity: 0.7;
  }
  .sidebar-item:active .drag-handle-hint {
    cursor: grabbing;
  }
  .bm-icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    padding: 2px 4px;
    margin: -2px -4px;
    transition: transform 0.15s ease, background 0.15s ease, color 0.15s ease;
  }
  .bm-icon-btn:hover {
    transform: scale(1.25);
    background: rgba(255, 255, 255, 0.1);
    color: var(--accent, #8264ff);
  }
  /* Unmounted disk — soluk göster */
  .sidebar-item.unmounted {
    opacity: 0.55;
  }
  .sidebar-item.unmounted:hover {
    opacity: 1;
    border-color: var(--border, rgba(255,255,255,0.08));
    border-style: dashed;
  }

  .item-icon { color: var(--accent, #8264ff); font-size: 10px; flex-shrink: 0; }
  .dev-icon  { font-size: 13px; }

  .item-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .dev-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .dev-sub {
    font-size: 9px;
    color: var(--text-dim, rgba(255,255,255,0.3));
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-hint {
    padding: 6px 8px;
    font-size: 10px;
    color: var(--text-dim, rgba(255,255,255,0.35));
    font-style: italic;
  }

  .ctx-sep-small { height: 1px; background: var(--border); margin: 4px 4px; opacity: 0.5; }

  .expand-btn {
    width: 100%; padding: 4px 8px;
    background: none; border: none;
    color: var(--text-dim); font-size: 10px;
    cursor: pointer; text-align: left;
    border-radius: var(--radius-sm);
    transition: background 0.1s, color 0.1s;
  }
  .expand-btn:hover { background: var(--bg-hover); color: var(--text-secondary); }

  /* ── Bookmark dropdown ── */
  .bm-dropdown {
    position: fixed;
    z-index: 9999;
    min-width: 170px;
    border-radius: var(--radius, 8px);
    padding: 4px;
    box-shadow: 0 8px 28px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.06);
    background: var(--bg-secondary, rgba(22,22,30,0.96));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    animation: fadeInBmDropdown 0.22s cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes fadeInBmDropdown {
    from { opacity: 0; transform: translateY(-4px) scale(0.97); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .bm-action {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--radius-sm, 5px);
    border: none;
    background: none;
    color: var(--text-primary, rgba(255,255,255,0.9));
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s;
  }
  .bm-action:hover { background: var(--bg-hover, rgba(255,255,255,0.06)); }
  .bm-action.danger { color: var(--danger, #ff5f6d); }
  .bm-sep { height: 1px; background: var(--border, rgba(255,255,255,0.08)); margin: 3px 0; }

  .hidden-badge-btn {
    font-size: 11px;
    opacity: 0.7;
    margin-right: 2px;
  }
  .hidden-badge-btn:hover {
    opacity: 1;
    color: var(--accent, #8264ff);
  }
  .hidden-pill {
    font-size: 8px;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(255, 95, 109, 0.2);
    color: #ff5f6d;
    margin-left: 5px;
    vertical-align: middle;
  }
  .unhide-all-btn {
    width: 100%;
    margin-top: 4px;
    padding: 4px 8px;
    background: rgba(130, 100, 255, 0.08);
    border: 1px dashed rgba(130, 100, 255, 0.3);
    border-radius: var(--radius-sm);
    color: var(--text-accent, #8264ff);
    font-size: 10px;
    cursor: pointer;
    text-align: center;
    transition: all 0.15s ease;
  }
  .unhide-all-btn:hover {
    background: rgba(130, 100, 255, 0.18);
  }
  .dev-item.pinned {
    border-left: 2px solid var(--accent, #8264ff);
  }
  .dev-item.is-hidden-dev {
    opacity: 0.45;
  }
</style>
