<script>
  import { blockDevices, navigateTo, notify, selectedDevice, showDevices } from "../stores/explorer.js";
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  // Boyut string'ini byte'a çevir: "500G" → 500*1024^3
  function parseSize(s) {
    if (!s) return 0;
    const m = s.match(/^([\d.]+)\s*([KMGTP]?)/i);
    if (!m) return 0;
    const n = parseFloat(m[1]);
    const u = m[2].toUpperCase();
    const map = { K: 1024, M: 1024**2, G: 1024**3, T: 1024**4, P: 1024**5 };
    return n * (map[u] || 1);
  }

  // Büyükten küçüğe sıralı disk listesi
  $: sortedDevices = [...$blockDevices].sort((a, b) =>
    parseSize(b.size) - parseSize(a.size)
  );

  let diskUsage   = {};
  let deviceMenu  = null; // { x, y, dev }
  let menuEl;

  async function loadDiskUsage() {
    try {
      const drives = await invoke("get_drives");
      diskUsage = {};
      for (const d of drives) {
        diskUsage[d.device] = {
          total: d.total, used: d.used, avail: d.available,
          pct: d.total > 0 ? Math.round((d.used / d.total) * 100) : 0,
        };
      }
    } catch {}
  }

  $: loadDiskUsage(), $blockDevices;

  function formatSize(bytes) {
    if (!bytes) return "—";
    const u = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  }

  function pctColor(pct) {
    if (pct >= 90) return "var(--danger)";
    if (pct >= 70) return "var(--warning)";
    return "var(--accent)";
  }

  function devIcon(dev) {
    if (dev.fs_type === "adb") return "📱";
    if (dev.is_windows) return "🪟";
    if (dev.is_mounted) return "◉";
    return "○";
  }

  // Tek tık → seç, çift tık → gezin
  function onCardClick(e, dev) {
    e.stopPropagation();
    selectedDevice.set(dev);
  }

  function onCardDblClick(dev) {
    if (dev.is_mounted && dev.mount_point) {
      navigateTo(dev.mount_point);
      showDevices.set(false);
      dispatch("navigate");
    } else {
      dispatch("open-mounts");
    }
  }

  function onCardContextMenu(e, dev) {
    e.preventDefault();
    e.stopPropagation();
    selectedDevice.set(dev);
    const vw = window.innerWidth, vh = window.innerHeight;
    const mw = 210, mh = 280;
    deviceMenu = {
      dev,
      x: e.clientX + mw > vw ? e.clientX - mw : e.clientX,
      y: e.clientY + mh > vh ? e.clientY - mh : e.clientY,
    };
  }

  function closeMenu() { deviceMenu = null; }

  // Boş alana tık → seçimi kaldır
  function onBgClick() {
    selectedDevice.set(null);
    deviceMenu = null;
  }

  // ── Disk actions (stub'lar — ilerleyen sprint'lerde implement) ──
  async function openDisk(dev) {
    if (!dev.is_mounted || !dev.mount_point) { dispatch("open-mounts"); return; }
    navigateTo(dev.mount_point);
    showDevices.set(false);
    dispatch("navigate");
    closeMenu();
  }

  async function mountDisk(dev) {
    dispatch("open-mounts");
    closeMenu();
  }

  async function unmountDisk(dev) {
    try {
      await invoke("unmount_device", { device: dev.device });
      notify("success", `${dev.label || dev.name} ayrıldı`);
      await invoke("list_block_devices").then(d => {}).catch(() => {});
    } catch (e) {
      notify("error", String(e));
    }
    closeMenu();
  }

  function stubAction(name) {
    notify("info", `${name} — yakında gelecek`);
    closeMenu();
  }
</script>

<svelte:window on:click={onBgClick} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="devices-view" on:click={onBgClick}>

  <div class="dv-header">
    <span class="dv-title">◉ Devices</span>
    <span class="dv-sub">Bir diske çift tıkla → içine gir · Sağ tıkla → disk işlemleri</span>
  </div>

  <div class="disk-grid">
    {#each sortedDevices as dev (dev.device)}
      {@const usage = diskUsage[dev.device]}
      {@const sel   = $selectedDevice?.device === dev.device}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="disk-card"
        class:selected={sel}
        class:unmounted={!dev.is_mounted}
        class:windows={dev.is_windows}
        on:click={e => onCardClick(e, dev)}
        on:dblclick={() => onCardDblClick(dev)}
        on:contextmenu={e => onCardContextMenu(e, dev)}
        title="{dev.device} · {dev.fs_type} · {dev.size}"
      >
        <div class="card-top">
          <span class="disk-icon">{devIcon(dev)}</span>
          <div class="disk-info">
            <span class="disk-name">{dev.label || dev.name}</span>
            <span class="disk-device">{dev.device}</span>
          </div>
          {#if dev.is_windows}
            <span class="win-badge">NTFS</span>
          {/if}
        </div>

        <div class="card-meta">
          <span class="fs-badge">{dev.fs_type}</span>
          <span class="disk-size">{dev.size}</span>
        </div>

        {#if dev.is_mounted && usage}
          <div class="usage-bar-track">
            <div class="usage-bar-fill"
              style="width:{usage.pct}%; background:{pctColor(usage.pct)}">
            </div>
          </div>
          <div class="usage-nums">
            <span style="color:{pctColor(usage.pct)};font-weight:600">{usage.pct}%</span>
            <span class="usage-detail">{formatSize(usage.used)} / {formatSize(usage.total)}</span>
          </div>
          <div class="usage-avail">
            <span class="avail-label">Boş</span>
            <span class="avail-val">{formatSize(usage.avail)}</span>
          </div>
        {:else if !dev.is_mounted}
          <div class="unmounted-hint">○ Unmounted — çift tıkla mount et</div>
        {/if}
      </div>
    {/each}

    {#if sortedDevices.length === 0}
      <div class="no-disk">◫ Disk bulunamadı</div>
    {/if}
  </div>
</div>

<!-- ── Device context menu ── -->
{#if deviceMenu}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="dev-ctx-menu"
    style="left:{deviceMenu.x}px; top:{deviceMenu.y}px"
    bind:this={menuEl}
    on:click|stopPropagation
    on:contextmenu|preventDefault|stopPropagation
  >
    <div class="ctx-label">{deviceMenu.dev.label || deviceMenu.dev.name}</div>
    <div class="ctx-sep"></div>

    {#if deviceMenu.dev.is_mounted}
      <button class="ctx-item" on:click={() => openDisk(deviceMenu.dev)}>
        <span class="ctx-icon">▶</span> Aç
      </button>
      <button class="ctx-item" on:click={() => unmountDisk(deviceMenu.dev)}>
        <span class="ctx-icon">⏏</span> Unmount
      </button>
    {:else}
      <button class="ctx-item" on:click={() => mountDisk(deviceMenu.dev)}>
        <span class="ctx-icon">⊕</span> Mount Et
      </button>
    {/if}
    <div class="ctx-sep"></div>

    <button class="ctx-item" on:click={() => stubAction("Biçimlendir")}>
      <span class="ctx-icon">◫</span> Biçimlendir
      <span class="ctx-badge">yakında</span>
    </button>
    <button class="ctx-item" on:click={() => stubAction("Yedekle")}>
      <span class="ctx-icon">◈</span> Yedekle (dd/rsync)
      <span class="ctx-badge">yakında</span>
    </button>
    <button class="ctx-item" on:click={() => stubAction("Sıkıştır")}>
      <span class="ctx-icon">🗜</span> Sıkıştır
      <span class="ctx-badge">yakında</span>
    </button>

    {#if deviceMenu.dev.fs_type === "btrfs"}
      <div class="ctx-sep"></div>
      <button class="ctx-item" on:click={() => stubAction("BTRFS sıkıştırma")}>
        <span class="ctx-icon">⚙</span> BTRFS Sıkıştırma Formatı
        <span class="ctx-badge">yakında</span>
      </button>
    {/if}

    <div class="ctx-sep"></div>
    <button class="ctx-item" on:click={() => stubAction("Sıfırla")}>
      <span class="ctx-icon danger-icon">⚠</span>
      <span style="color:var(--danger)">Sıfırla (wipe)</span>
      <span class="ctx-badge">yakında</span>
    </button>
  </div>
{/if}

<style>
  .devices-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 20px;
    gap: 16px;
    background: var(--bg-primary);
  }

  .dv-header { display: flex; flex-direction: column; gap: 4px; }
  .dv-title { font-size: 16px; font-weight: 700; color: var(--accent); }
  .dv-sub { font-size: 11px; color: var(--text-dim); }

  .disk-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
  }

  .disk-card {
    display: flex; flex-direction: column; gap: 8px;
    padding: 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: border-color 0.12s, transform 0.1s, box-shadow 0.12s;
    background: var(--bg-secondary, rgba(22,22,30,0.95));
    user-select: none;
  }
  .disk-card:hover { border-color: var(--border-accent); transform: translateY(-1px); }
  .disk-card.selected {
    border-color: var(--accent);
    background: var(--bg-selected);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .disk-card.unmounted { opacity: 0.6; border-style: dashed; }
  .disk-card.unmounted:hover { opacity: 1; }
  .disk-card.windows { border-color: rgba(0,120,212,0.3); }
  .disk-card.windows.selected { border-color: #60aaff; box-shadow: 0 0 0 1px #60aaff; }

  .card-top { display: flex; align-items: center; gap: 10px; }
  .disk-icon { font-size: 22px; flex-shrink: 0; }
  .disk-info { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .disk-name { font-size: 13px; font-weight: 600; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .disk-device { font-size: 10px; color: var(--text-dim); font-family: var(--font-mono); }
  .win-badge { font-size: 9px; padding: 1px 5px; background: rgba(0,120,212,0.2); color: #60aaff; border: 1px solid rgba(0,120,212,0.3); border-radius: 4px; flex-shrink: 0; }

  .card-meta { display: flex; align-items: center; gap: 6px; }
  .fs-badge { font-size: 9px; padding: 1px 5px; background: var(--bg-tertiary); border: 1px solid var(--border); border-radius: 4px; color: var(--text-dim); text-transform: uppercase; font-family: var(--font-mono); }
  .disk-size { font-size: 11px; color: var(--text-secondary); margin-left: auto; font-family: var(--font-mono); }

  .usage-bar-track { height: 4px; background: var(--bg-tertiary); border-radius: 2px; overflow: hidden; }
  .usage-bar-fill { height: 100%; border-radius: 2px; transition: width 0.4s ease; }
  .usage-nums { display: flex; justify-content: space-between; font-size: 10px; }
  .usage-detail { color: var(--text-dim); }
  .usage-avail { display: flex; justify-content: space-between; font-size: 10px; border-top: 1px solid var(--border); padding-top: 4px; }
  .avail-label { color: var(--text-dim); }
  .avail-val { color: var(--text-secondary); font-family: var(--font-mono); }
  .unmounted-hint { font-size: 10px; color: var(--text-dim); font-style: italic; text-align: center; }
  .no-disk { grid-column: 1/-1; text-align: center; color: var(--text-dim); padding: 40px; }

  .dev-ctx-menu {
    position: fixed; z-index: 9999;
    min-width: 210px;
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 8px 32px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05);
    background: var(--bg-secondary, rgba(22,22,30,0.98));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
  }
  .ctx-label { padding: 4px 10px; font-size: 10px; color: var(--text-dim); font-weight: 600; text-transform: uppercase; letter-spacing: 0.08em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ctx-sep { height: 1px; background: var(--border); margin: 3px 0; }
  .ctx-item {
    display: flex; align-items: center; gap: 8px;
    width: 100%; padding: 6px 10px;
    border-radius: var(--radius-sm); border: none; background: none;
    color: var(--text-primary); font-size: 12px; cursor: pointer;
    text-align: left; transition: var(--transition-fast); user-select: none;
  }
  .ctx-item:hover { background: var(--bg-hover); }
  .ctx-icon { font-size: 12px; width: 16px; text-align: center; flex-shrink: 0; }
  .danger-icon { color: var(--danger); }
  .ctx-badge {
    margin-left: auto;
    font-size: 9px; padding: 1px 5px;
    background: var(--bg-tertiary); border: 1px solid var(--border);
    border-radius: 4px; color: var(--text-dim); font-style: italic;
  }
</style>
