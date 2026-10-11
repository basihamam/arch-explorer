<script>
  import { blockDevices, navigateTo, notify } from "../stores/explorer.js";
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  const dispatch = createEventDispatcher();

  // df çıktısından mounted disk kullanımı
  let diskUsage = {}; // device → { total, used, avail, pct }

  async function loadDiskUsage() {
    try {
      const drives = await invoke("get_drives");
      diskUsage = {};
      for (const d of drives) {
        diskUsage[d.device] = {
          total: d.total,
          used:  d.used,
          avail: d.available,
          pct:   d.total > 0 ? Math.round((d.used / d.total) * 100) : 0,
        };
      }
    } catch {}
  }

  $: loadDiskUsage(), $blockDevices; // blockDevices değişince yenile

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

  function handleClick(dev) {
    if (dev.is_mounted && dev.mount_point) {
      navigateTo(dev.mount_point);
      dispatch("navigate");
    } else {
      dispatch("open-mounts");
    }
  }
</script>

<div class="home-screen">
  <div class="home-header">
    <span class="home-title">◈ Arch Explorer</span>
    <span class="home-sub">Bir diske tıkla veya sol panelden gezinmeye başla</span>
  </div>

  <div class="disk-grid">
    {#each $blockDevices as dev (dev.device)}
      {@const usage = diskUsage[dev.device]}
      <!-- svelte-ignore a11y-no-static-element-interactions -->
      <div
        class="disk-card glass"
        class:unmounted={!dev.is_mounted}
        class:windows={dev.is_windows}
        on:click={() => handleClick(dev)}
        title={dev.is_mounted ? dev.mount_point : "Unmounted — tıkla mount et"}
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
          <div class="usage-section">
            <div class="usage-bar-track">
              <div
                class="usage-bar-fill"
                style="width:{usage.pct}%; background:{pctColor(usage.pct)}"
              ></div>
            </div>
            <div class="usage-nums">
              <span style="color:{pctColor(usage.pct)}">{usage.pct}%</span>
              <span class="usage-detail">
                {formatSize(usage.used)} / {formatSize(usage.total)}
              </span>
            </div>
          </div>
          <div class="usage-avail">
            <span class="avail-label">Boş</span>
            <span class="avail-val">{formatSize(usage.avail)}</span>
          </div>
        {:else if !dev.is_mounted}
          <div class="unmounted-hint">
            <span>○ Mount edilmemiş · tıkla</span>
          </div>
        {:else}
          <div class="unmounted-hint"><span>Kullanım bilgisi alınamadı</span></div>
        {/if}
      </div>
    {/each}

    {#if $blockDevices.length === 0}
      <div class="no-disk">
        <span>◫ Disk bulunamadı</span>
      </div>
    {/if}
  </div>
</div>

<style>
  .home-screen {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    padding: 24px 20px;
    gap: 20px;
    background: var(--bg-primary);
  }

  .home-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .home-title {
    font-size: 18px;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.02em;
  }
  .home-sub {
    font-size: 11px;
    color: var(--text-dim);
  }

  .disk-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }

  .disk-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, transform 0.1s;
    background: var(--bg-secondary);
  }
  .disk-card:hover {
    border-color: var(--border-accent);
    background: var(--bg-tertiary);
    transform: translateY(-1px);
  }
  .disk-card.unmounted {
    opacity: 0.6;
    border-style: dashed;
  }
  .disk-card.unmounted:hover { opacity: 1; }
  .disk-card.windows { border-color: rgba(0,120,212,0.3); }
  .disk-card.windows:hover { border-color: rgba(0,120,212,0.6); }

  .card-top {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .disk-icon { font-size: 22px; flex-shrink: 0; }
  .disk-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .disk-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .disk-device {
    font-size: 10px;
    color: var(--text-dim);
    font-family: var(--font-mono);
  }
  .win-badge {
    font-size: 9px;
    padding: 1px 5px;
    background: rgba(0,120,212,0.2);
    color: #60aaff;
    border: 1px solid rgba(0,120,212,0.3);
    border-radius: 4px;
    flex-shrink: 0;
  }

  .card-meta {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .fs-badge {
    font-size: 9px;
    padding: 1px 5px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-dim);
    text-transform: uppercase;
    font-family: var(--font-mono);
  }
  .disk-size {
    font-size: 11px;
    color: var(--text-secondary);
    margin-left: auto;
    font-family: var(--font-mono);
  }

  .usage-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .usage-bar-track {
    height: 4px;
    background: var(--bg-tertiary);
    border-radius: 2px;
    overflow: hidden;
  }
  .usage-bar-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.4s ease;
  }
  .usage-nums {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 10px;
    font-weight: 600;
  }
  .usage-detail { color: var(--text-dim); font-weight: 400; }

  .usage-avail {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    padding-top: 2px;
    border-top: 1px solid var(--border);
  }
  .avail-label { color: var(--text-dim); }
  .avail-val { color: var(--text-secondary); font-family: var(--font-mono); }

  .unmounted-hint {
    font-size: 10px;
    color: var(--text-dim);
    font-style: italic;
    text-align: center;
    padding: 4px 0;
  }

  .no-disk {
    grid-column: 1 / -1;
    text-align: center;
    color: var(--text-dim);
    font-size: 13px;
    padding: 40px;
  }
</style>
