<script>
  import { invoke } from "@tauri-apps/api/core";
  import {
    bookmarks, blockDevices, drives,
    navigateTo, currentPath,
    addBookmark, removeBookmark, loadBookmarks, notify,
    addTab, loadBlockDevices, showDevices, selectedDevice,
  } from "../stores/explorer.js";

  // ---- Event: MountPanel'i aç ----
  // Sidebar'dan unmounted diske tıklanınca App.svelte'e haber ver
  import { createEventDispatcher } from "svelte";
  const dispatch = createEventDispatcher();

  // ---- Drag ----
  let dragOverPath = null;

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

  async function handleDropOnBookmark(e, bm) {
    e.preventDefault(); dragOverPath = null;
    const src = e.dataTransfer.getData("text/plain");
    if (!src || src === bm.path) return;
    const dst = bm.path + "/" + src.split("/").pop();
    try {
      await invoke("move_item", { src, dst });
      notify("success", `Moved → ${bm.name}`);
      navigateTo($currentPath);
    } catch (err) { notify("error", String(err)); }
  }

  async function handleDropOnDevice(e, mp) {
    e.preventDefault(); dragOverPath = null;
    const src = e.dataTransfer.getData("text/plain");
    if (!src || !mp) return;
    const dst = mp + "/" + src.split("/").pop();
    try {
      await invoke("move_item", { src, dst });
      notify("success", `Moved → ${mp}`);
      navigateTo($currentPath);
    } catch (err) { notify("error", String(err)); }
  }

  async function promptAddBookmark() {
    const name = prompt("Bookmark name:", $currentPath.split("/").filter(Boolean).pop() || "root");
    if (!name) return;
    await addBookmark(name, $currentPath);
  }

  // ---- Devices collapse (5+ ise) ----
  let devicesExpanded = false;
  $: visibleDevices = devicesExpanded || $blockDevices.length <= 5
    ? $blockDevices
    : $blockDevices.slice(0, 5);

  function handleDeviceClick(dev) {
    if (dev.is_mounted && dev.mount_point) {
      navigateTo(dev.mount_point);
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
  }

  // ---- Disk icon ----
  function devIcon(dev) {
    if (dev.is_windows)  return "🪟";
    if (dev.is_mounted)  return "◉";
    return "○";
  }

  function devLabel(dev) {
    // label varsa kullan, yoksa device adı
    if (dev.label && dev.label !== dev.name) return dev.label;
    return dev.name || dev.device;
  }
</script>

<svelte:window on:mousedown={onWindowMousedown} />

<aside class="sidebar glass-panel">

  <!-- ── Bookmarks ── -->
  <div class="section">
    <div class="section-header">
      <span>Bookmarks</span>
      <button class="add-btn" on:click={promptAddBookmark} title="Add current folder">+</button>
    </div>

    {#each $bookmarks as bm (bm.path)}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="sidebar-item"
        class:active={$currentPath === bm.path}
        class:drag-over={dragOverPath === bm.path}
        class:pinned={bm.pinned}
        on:click={() => navigateTo(bm.path)}
        on:contextmenu={e => openBmMenu(e, bm)}
        on:dragover|preventDefault={() => dragOverPath = bm.path}
        on:dragleave={() => dragOverPath = null}
        on:drop={e => handleDropOnBookmark(e, bm)}
        title={bm.path}
        role="button"
        tabindex="0"
        on:keydown={e => e.key === "Enter" && navigateTo(bm.path)}
      >
        <span class="item-icon">{bm.pinned ? "◈" : "⊹"}</span>
        <span class="item-label">{bm.name}</span>
      </div>
    {/each}

    {#if $bookmarks.length === 0}
      <div class="empty-hint">No bookmarks yet</div>
    {/if}
  </div>

  <!-- ── Devices (lsblk tabanlı — mount edilmemiş diskler de görünür) ── -->
  <div class="section">
    <div class="section-header">
      <span>Devices</span>
      <button class="add-btn" on:click={openDevicesView} title="Tüm Diskler (Devices View)">◉</button>
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
      <span class="item-label">Devices</span>
    </div>

    <div class="ctx-sep-small"></div>

    {#each visibleDevices as dev (dev.device)}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="sidebar-item dev-item"
        class:active={!$showDevices && dev.is_mounted && dev.mount_point && $currentPath.startsWith(dev.mount_point)}
        class:drag-over={dragOverPath === (dev.mount_point || dev.device)}
        class:unmounted={!dev.is_mounted}
        on:click={() => handleDeviceClick(dev)}
        on:dragover|preventDefault={() => dragOverPath = dev.mount_point || dev.device}
        on:dragleave={() => dragOverPath = null}
        on:drop={e => dev.is_mounted && dev.mount_point && handleDropOnDevice(e, dev.mount_point)}
        role="button" tabindex="0"
        on:keydown={e => e.key === "Enter" && handleDeviceClick(dev)}
        title="{dev.device} · {dev.fs_type} · {dev.size}{dev.is_mounted ? ' · ' + dev.mount_point : ' · unmounted'}"
      >
        <span class="item-icon dev-icon">{devIcon(dev)}</span>
        <div class="dev-text">
          <span class="item-label">{devLabel(dev)}</span>
          <span class="dev-sub">{dev.size} · {dev.fs_type}{!dev.is_mounted ? " · click to mount" : ""}</span>
        </div>
      </div>
    {/each}

    {#if $blockDevices.length > 5}
      <button class="expand-btn" on:click={() => devicesExpanded = !devicesExpanded}>
        {devicesExpanded ? "▲ Daha az" : `▼ +${$blockDevices.length - 5} daha`}
      </button>
    {/if}

    {#if $blockDevices.length === 0}
      <div class="empty-hint">No block devices</div>
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
    <button class="bm-action" on:click={() => { navigateTo(bmMenu.bm.path); closeBmMenu(); }}>
      <span>▶</span> Open
    </button>
    <button class="bm-action" on:click={() => { addTab(bmMenu.bm.path); closeBmMenu(); }}>
      <span>⊞</span> Open in New Tab
    </button>
    <button class="bm-action" on:click={() => renameBm(bmMenu.bm)}>
      <span>✏</span> Rename
    </button>
    <button class="bm-action" on:click={() => pinBm(bmMenu.bm)}>
      <span>{bmMenu.bm.pinned ? "○" : "◈"}</span>
      {bmMenu.bm.pinned ? "Unpin" : "Pin to top"}
    </button>
    <div class="bm-sep"></div>
    <button class="bm-action danger" on:click={() => removeBm(bmMenu.bm)}>
      <span>✕</span> Remove bookmark
    </button>
  </div>
{/if}

<style>
  .sidebar {
    width: var(--sidebar-width, 200px);
    min-width: 160px;
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
    gap: 8px;
    padding: 5px 8px;
    border-radius: var(--radius-sm, 5px);
    border: 1px solid transparent;
    background: none;
    color: var(--text-secondary, rgba(255,255,255,0.55));
    font-size: 12px;
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s, color 0.12s;
    width: 100%;
    user-select: none;
    min-width: 0;
  }
  .sidebar-item:hover {
    background: var(--bg-hover, rgba(255,255,255,0.06));
    color: var(--text-primary, rgba(255,255,255,0.9));
  }
  .sidebar-item.active {
    background: var(--accent-dim, rgba(130,100,255,0.15));
    border-color: var(--border-accent, rgba(130,100,255,0.35));
    color: var(--text-accent, #8264ff);
  }
  .sidebar-item.drag-over {
    background: var(--accent-dim, rgba(130,100,255,0.15));
    border-color: var(--accent, #8264ff);
    border-style: dashed;
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
    backdrop-filter: blur(16px);
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
  .bm-action.danger:hover { background: rgba(255,95,109,0.12); }
  .bm-action span:first-child { font-size: 11px; width: 14px; text-align: center; }
  .bm-sep { height: 1px; background: var(--border, rgba(255,255,255,0.08)); margin: 3px 0; }
</style>
