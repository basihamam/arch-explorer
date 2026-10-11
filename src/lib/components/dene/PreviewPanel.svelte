<script>
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { invoke } from "@tauri-apps/api/core";
  import {
    previewEntry, showPreview, blockDevices,
    previewMode, previewCustomTypes,
    selectedDevice, currentPath, requestSudo,
  } from "../stores/explorer.js";

  let preview   = null;
  let loading   = false;
  let error     = null;
  let imgSrc    = null;

  // Dir du cache
  let duLoading = false;
  let duResult  = null;

  // Disk overview
  let diskUsage   = {};

  // Büyük dosyalar (selectedDevice için)
  let bigFiles   = [];
  let bigLoading = false;

  // ── previewMode → showPreview senkronizasyonu ──
  // Orijinal {#if $showPreview} korunuyor, sadece store'u yönetiyoruz
  $: if ($previewMode === "always")  showPreview.set(true);
  $: if ($previewMode === "manual")  showPreview.set(false);
  // auto: seçim varsa aç, yoksa kapat
  $: if ($previewMode === "auto") showPreview.set(!!$previewEntry || !!$selectedDevice);

  function shouldShowCustom(entry) {
    if (!entry) return false;
    const ext = (entry.extension || "").toLowerCase();
    return ($previewCustomTypes || []).includes(ext);
  }

  function pctColor(pct) {
    if (pct >= 90) return "var(--danger)";
    if (pct >= 70) return "var(--warning)";
    return "var(--accent)";
  }

  function getCurrentDisk() {
    const path = $currentPath;
    let best = null, bestLen = 0;
    for (const dev of ($blockDevices || [])) {
      if (!dev.is_mounted || !dev.mount_point) continue;
      if (path.startsWith(dev.mount_point) && dev.mount_point.length > bestLen) {
        best = dev; bestLen = dev.mount_point.length;
      }
    }
    return best;
  }

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

  let duLoadingTimeout = null;

  async function loadBigFiles(mountPoint) {
    if (!mountPoint) {
      bigLoading = false;
      bigFiles = [];
      return;
    }
    bigLoading = true; bigFiles = [];
    try {
      const res = await Promise.race([
        invoke("get_big_files", { mountPoint }),
        new Promise((_, reject) => setTimeout(() => reject(new Error("timeout")), 3000))
      ]);
      bigFiles = res || [];
    } catch {
      bigFiles = [];
    } finally {
      bigLoading = false;
    }
  }

  // selectedDevice değişince disk bilgilerini yükle
  $: if ($selectedDevice) {
    loadDiskUsage();
    loadBigFiles($selectedDevice.mount_point || "");
  }

  // Boşken disk bilgisi
  $: if (!$previewEntry && !$selectedDevice && $previewMode === "always") {
    loadDiskUsage();
  }

  const IMAGE_MIMES = ["image/jpeg","image/jpg","image/png","image/gif",
                       "image/webp","image/svg+xml","image/bmp","image/ico",
                       "image/tiff","image/avif"];

  function isImage(mime) {
    return (mime && IMAGE_MIMES.some(m => mime.startsWith(m.split("/")[0] + "/") && mime === m))
      || (mime && mime.startsWith("image/"));
  }

  let previewTimer = null;
  let duTimer = null;
  let lastPreviewPath = null;

  $: if ($previewEntry) {
    if ($selectedDevice) {
      selectedDevice.set(null);
    }

    if ($previewEntry.path !== lastPreviewPath) {
      lastPreviewPath = $previewEntry.path;
      imgSrc   = null;
      preview  = null;
      error    = null;
      duResult = null;

      clearTimeout(previewTimer);
      clearTimeout(duTimer);
      clearTimeout(duLoadingTimeout);

      if ($previewMode === "custom" && !shouldShowCustom($previewEntry)) {
        loading = false;
        duLoading = false;
      } else if (isImage($previewEntry.mime_type)) {
        imgSrc = convertFileSrc($previewEntry.path);
        loading = false;
        duLoading = false;
      } else if ($previewEntry.is_dir) {
        loadDirPreview($previewEntry.path);
      } else {
        duLoading = false;
        previewTimer = setTimeout(() => {
          loadPreview($previewEntry.path);
        }, 80);
      }
    }
  } else {
    lastPreviewPath = null;
    clearTimeout(previewTimer);
    clearTimeout(duTimer);
    clearTimeout(duLoadingTimeout);
    preview  = null;
    imgSrc   = null;
    error    = null;
    loading  = false;
    duLoading = false;
    duResult = null;
  }

  async function loadPreview(path) {
    loading = true;
    error   = null;
    preview = null;
    try {
      preview = await invoke("get_file_preview", { path });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadDirPreview(path) {
    loading = true;
    duLoading = true;
    error = null;
    preview = null;
    duResult = null;

    clearTimeout(duLoadingTimeout);
    // Güvenlik: 1.5 saniye sonra ne olursa olsun dönen çarkı durdur
    duLoadingTimeout = setTimeout(() => {
      duLoading = false;
    }, 1500);

    // 1. Önce anında dosya önizlemesini yükle (hızlı)
    try {
      preview = await invoke("get_file_preview", { path });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }

    // 2. Klasör boyutu hesaplamasını 100ms debounce ile arka planda yap
    duTimer = setTimeout(async () => {
      try {
        const du = await invoke("get_dir_size", { path });
        if ($previewEntry?.path === path) {
          duResult = du;
        }
      } catch (e) {
        // du hatası dosya önizlemesini bozmaz
      } finally {
        clearTimeout(duLoadingTimeout);
        duLoading = false;
      }
    }, 100);
  }

  async function calculateDirWithSudo() {
    if (!$previewEntry) return;
    const path = $previewEntry.path;
    try {
      await requestSudo(
        "Klasör Taraması İçin Root Yetkisi",
        `'${path}' dizinini okumak ve boyutunu hesaplamak için yönetici şifresi gereklidir.`,
        async (password) => {
          duLoading = true;
          const du = await invoke("sudo_get_dir_size", { path, password });
          if ($previewEntry?.path === path) {
            duResult = du;
          }
        }
      );
    } catch (e) {
      // İptal veya hata
    } finally {
      duLoading = false;
    }
  }

  function formatSize(bytes) {
    if (!bytes && bytes !== 0) return "—";
    const u = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  }

  function getMediaSummary(info) {
    if (!info) return [];
    const rows = [];
    const fmt = info.format;
    if (fmt) {
      if (fmt.duration)        rows.push({ k: "Duration", v: formatDuration(parseFloat(fmt.duration)) });
      if (fmt.size)            rows.push({ k: "Size",     v: formatSize(parseInt(fmt.size)) });
      if (fmt.bit_rate)        rows.push({ k: "Bitrate",  v: `${Math.round(parseInt(fmt.bit_rate)/1000)} kbps` });
      if (fmt.format_long_name) rows.push({ k: "Format",  v: fmt.format_long_name });
    }
    for (const s of (info.streams || [])) {
      if (s.codec_type === "video") {
        rows.push({ k: "Video", v: `${s.codec_name} ${s.width}x${s.height}` });
        if (s.r_frame_rate) rows.push({ k: "FPS", v: evalFraction(s.r_frame_rate) });
      }
      if (s.codec_type === "audio") {
        rows.push({ k: "Audio", v: `${s.codec_name} ${s.channels}ch ${s.sample_rate}Hz` });
      }
    }
    return rows;
  }

  function formatDuration(secs) {
    if (isNaN(secs)) return "?";
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    return h > 0
      ? `${h}:${String(m).padStart(2,"0")}:${String(s).padStart(2,"0")}`
      : `${m}:${String(s).padStart(2,"0")}`;
  }

  function evalFraction(f) {
    const [n, d] = f.split("/").map(Number);
    return d ? `${(n/d).toFixed(2)} fps` : `${n} fps`;
  }

  function kindIcon(entry, pvKind) {
    if (!entry) return "◫";
    if (entry.is_dir) return "📁";
    if (isImage(entry?.mime_type)) return "🖼";
    return { text:"📝", video:"🎬", audio:"🎵", dir:"📁", binary:"⬡" }[pvKind] || "📄";
  }
</script>

{#if $showPreview}
  <aside class="preview-panel glass-panel">

    {#if $selectedDevice}
      <!-- ── Seçili Disk Detayı ── -->
      {@const u = diskUsage[$selectedDevice.device]}
      <div class="pv-header">
        <span class="pv-kind-icon">{$selectedDevice.is_windows ? "🪟" : "◉"}</span>
        <div class="pv-title-group">
          <span class="pv-name">{$selectedDevice.label || $selectedDevice.name}</span>
          <div class="pv-badges">
            <span class="badge">{$selectedDevice.fs_type}</span>
            {#if $selectedDevice.is_windows}<span class="badge lang">NTFS</span>{/if}
          </div>
        </div>
      </div>
      <div class="pv-meta">
        <div class="meta-row"><span class="mk">Aygıt</span><span class="mv mono">{$selectedDevice.device}</span></div>
        <div class="meta-row"><span class="mk">Boyut</span><span class="mv">{$selectedDevice.size}</span></div>
        {#if $selectedDevice.mount_point}
          <div class="meta-row"><span class="mk">Mount</span><span class="mv mono" style="font-size:9px">{$selectedDevice.mount_point}</span></div>
        {/if}
        {#if u}
          <div class="meta-row"><span class="mk">Kullanılan</span><span class="mv">{formatSize(u.used)}</span></div>
          <div class="meta-row"><span class="mk">Boş</span><span class="mv" style="color:var(--accent)">{formatSize(u.avail)}</span></div>
        {/if}
      </div>
      {#if u}
        <div style="padding:0 12px 8px">
          <div class="pv-usage-track">
            <div class="pv-usage-fill" style="width:{u.pct}%; background:{pctColor(u.pct)}"></div>
          </div>
          <div class="pv-usage-nums">
            <span style="color:{pctColor(u.pct)};font-weight:700">{u.pct}%</span>
            <span style="color:var(--text-dim);font-size:9px">{formatSize(u.used)} / {formatSize(u.total)}</span>
          </div>
        </div>
      {/if}
      <div class="pv-sep"></div>
      <div class="pv-content">
        {#if bigLoading}
          <div class="pv-empty"><div class="spinner"></div><span>Taranıyor...</span></div>
        {:else if bigFiles.length > 0}
          <div class="pv-section-title">En Büyük Dosyalar</div>
          <div class="pv-big-list">
            {#each bigFiles as f}
              <div class="pv-big-row" title={f.path}>
                <span class="pv-big-name">{f.path.split("/").pop()}</span>
                <span class="pv-big-size">{formatSize(f.size)}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="pv-empty"><span>{$selectedDevice.is_mounted ? "Büyük dosya bulunamadı" : "Mount edilmemiş"}</span></div>
        {/if}
      </div>

    {:else if !$previewEntry}
      <!-- ── Boşken: bulunulan disk özeti ── -->
      {@const cd = getCurrentDisk()}
      {#if cd}
        {@const u = diskUsage[cd.device]}
        <div class="pv-disk-summary">
          <div class="pv-disk-title">◉ Mevcut Disk</div>
          <div class="pv-disk-card">
            <div class="pv-disk-top">
              <span style="font-size:16px">{cd.is_windows ? "🪟" : "◉"}</span>
              <div style="flex:1;min-width:0">
                <div class="pv-disk-name">{cd.label || cd.name}</div>
                <div class="pv-disk-dev">{cd.device} · {cd.fs_type}</div>
              </div>
              <span class="pv-disk-sz">{cd.size}</span>
            </div>
            {#if u}
              <div class="pv-usage-track">
                <div class="pv-usage-fill" style="width:{u.pct}%; background:{pctColor(u.pct)}"></div>
              </div>
              <div class="pv-usage-nums">
                <span style="color:{pctColor(u.pct)};font-weight:600">{u.pct}%</span>
                <span style="color:var(--text-dim);font-size:9px">{formatSize(u.avail)} boş</span>
              </div>
            {/if}
          </div>
          <div class="pv-disk-hint">Bir dosyaya tıkla</div>
        </div>
      {:else}
        <div class="pv-empty">
          <span class="empty-icon">◫</span>
          <span>Click a file to preview</span>
        </div>
      {/if}

    {:else}
      <!-- ── Header ── -->
      <div class="pv-header">
        <span class="pv-kind-icon">{kindIcon($previewEntry, preview?.kind)}</span>
        <div class="pv-title-group">
          <span class="pv-name" title={$previewEntry.path}>{$previewEntry.name}</span>
          <div class="pv-badges">
            <span class="badge">{$previewEntry.extension || $previewEntry.mime_type.split("/")[1] || "file"}</span>
            {#if preview?.language && preview.kind === "text"}
              <span class="badge lang">{preview.language.toUpperCase()}</span>
            {/if}
          </div>
        </div>
      </div>

      <!-- ── Meta ── -->
      <div class="pv-meta">
        <div class="meta-row">
          <span class="mk">Size</span>
          {#if $previewEntry.is_dir}
            {#if duResult?.permission_denied}
              <span class="mv" style="color:var(--warning)">🔒 Yetki Gerekli</span>
            {:else}
              <span class="mv">{duResult ? formatSize(duResult.size) : (duLoading ? "..." : "—")}</span>
            {/if}
          {:else}
            <span class="mv">{formatSize($previewEntry.size)}</span>
          {/if}
        </div>
        <div class="meta-row">
          <span class="mk">Modified</span>
          <span class="mv">{$previewEntry.modified}</span>
        </div>
        <div class="meta-row">
          <span class="mk">Perms</span>
          <span class="mv mono">{$previewEntry.permissions}</span>
        </div>
        {#if $previewEntry.is_dir && duResult && !duResult.permission_denied}
          <div class="meta-row">
            <span class="mk">İçerik</span>
            <span class="mv">{duResult.file_count} öğe</span>
          </div>
        {/if}
      </div>

      <div class="pv-sep"></div>

      <!-- ── Content ── -->
      <div class="pv-content">

        {#if loading}
          <div class="pv-empty"><div class="spinner"></div></div>

        {:else if error}
          <div class="pv-empty err"><span>⚠</span><span>{error}</span></div>

        {:else if imgSrc}
          <!-- Image: convertFileSrc ile direkt yükle — base64 yok, her boyut çalışır -->
          <img class="pv-image" src={imgSrc} alt={$previewEntry.name}
            on:error={() => error = "Resim yüklenemedi"} />

        {:else if preview}

          {#if preview.kind === "text" || preview.kind === "binary"}
            {#if preview.is_truncated}
              <div class="pv-trunc">
                İlk {preview.kind === "binary" ? "512 byte" : "100 satır"} gösteriliyor
              </div>
            {/if}
            {#if preview.line_count}
              <div class="pv-trunc">{preview.line_count.toLocaleString()} satır</div>
            {/if}
            <pre class="pv-code">{preview.text_content}</pre>

          {:else if preview.kind === "video" || preview.kind === "audio"}
            {#if preview.media_info}
              <div class="pv-media-table">
                {#each getMediaSummary(preview.media_info) as row}
                  <div class="meta-row">
                    <span class="mk">{row.k}</span>
                    <span class="mv">{row.v}</span>
                  </div>
                {/each}
              </div>
            {:else}
              <div class="pv-note">
                ffprobe bulunamadı.<br>
                <code>sudo pacman -S ffmpeg</code>
              </div>
            {/if}

          {:else if preview.kind === "dir"}
            <div class="pv-dir">
              {#if duResult?.permission_denied}
                <span class="dir-count" style="font-size:24px; color:var(--warning)">🔒</span>
                <span class="dir-label" style="color:var(--warning)">Okuma Yetkisi Yok</span>
                <button
                  class="btn btn-sm btn-accent"
                  style="margin-top:10px; font-size:11px; padding:4px 10px; border-radius:6px; cursor:pointer;"
                  on:click={calculateDirWithSudo}
                  disabled={duLoading}
                >
                  {duLoading ? "Hesaplanıyor…" : "Root İle Boyutu Hesapla"}
                </button>
              {:else if duResult}
                <span class="dir-count">{formatSize(duResult.size)}</span>
                <span class="dir-label">{duResult.file_count} öğe</span>
                {#if duResult.from_cache}
                  <span style="font-size:9px;color:var(--text-dim);font-style:italic">önbellekten</span>
                {/if}
              {:else if duLoading}
                <span class="dir-count">{preview.line_count ?? 0}</span>
                <span class="dir-label" style="display:flex;align-items:center;gap:6px;justify-content:center">
                  <div class="spinner" style="width:12px;height:12px;border-width:2px;display:inline-block"></div>
                  öğe (boyut taranıyor…)
                </span>
              {:else}
                <span class="dir-count">{preview.line_count ?? 0}</span>
                <span class="dir-label">öğe</span>
              {/if}
            </div>

          {/if}
        {/if}

      </div>
    {/if}
  </aside>
{/if}

<style>
  .preview-panel {
    width: 100%;
    min-width: 0;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-left: 1px solid var(--border, rgba(255,255,255,0.08));
    border-top: none; border-bottom: none; border-right: none;
    border-radius: 0;
    background: var(--bg-secondary, rgba(22,22,30,0.87));
  }

  .pv-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-dim, rgba(255,255,255,0.3));
    font-size: 12px;
    text-align: center;
    padding: 20px;
  }
  .pv-empty.err { color: var(--danger, #ff5f6d); }
  .empty-icon { font-size: 32px; opacity: 0.4; }

  .pv-header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 12px 8px;
    border-bottom: 1px solid var(--border, rgba(255,255,255,0.08));
    flex-shrink: 0;
  }
  .pv-kind-icon { font-size: 22px; flex-shrink: 0; margin-top: 2px; }
  .pv-title-group { min-width: 0; flex: 1; }
  .pv-name {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary, rgba(255,255,255,0.9));
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-bottom: 4px;
  }
  .pv-badges { display: flex; gap: 4px; flex-wrap: wrap; }
  .badge {
    font-size: 9px;
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--bg-tertiary, rgba(30,30,40,0.9));
    border: 1px solid var(--border, rgba(255,255,255,0.08));
    color: var(--text-dim, rgba(255,255,255,0.35));
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .badge.lang {
    background: var(--accent-dim, rgba(130,100,255,0.15));
    border-color: var(--border-accent, rgba(130,100,255,0.35));
    color: var(--text-accent, #8264ff);
  }

  .pv-meta {
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex-shrink: 0;
  }
  .meta-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 10px;
  }
  .mk { color: var(--text-dim, rgba(255,255,255,0.35)); flex-shrink: 0; min-width: 52px; }
  .mv {
    color: var(--text-secondary, rgba(255,255,255,0.55));
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mv.mono { font-family: var(--font-mono, monospace); font-size: 9px; }

  .pv-sep { height: 1px; background: var(--border, rgba(255,255,255,0.08)); flex-shrink: 0; }

  .pv-content {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* Image */
  .pv-image {
    width: 100%;
    height: 100%;
    object-fit: contain;
    padding: 8px;
    box-sizing: border-box;
  }

  /* Text / binary */
  .pv-trunc {
    font-size: 9px;
    color: var(--text-dim, rgba(255,255,255,0.3));
    padding: 3px 12px;
    font-style: italic;
    flex-shrink: 0;
  }
  .pv-code {
    flex: 1;
    margin: 0;
    padding: 8px 12px;
    font-family: var(--font-mono, monospace);
    font-size: 9.5px;
    line-height: 1.55;
    color: var(--text-secondary, rgba(255,255,255,0.6));
    overflow: auto;
    white-space: pre;
    background: var(--bg-tertiary, rgba(30,30,40,0.9));
  }

  /* Media */
  .pv-media-table {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pv-note {
    padding: 12px;
    font-size: 11px;
    color: var(--text-dim, rgba(255,255,255,0.35));
    line-height: 1.7;
  }
  .pv-note code {
    font-family: var(--font-mono, monospace);
    font-size: 10px;
    color: var(--text-accent, #8264ff);
    background: var(--bg-tertiary, rgba(30,30,40,0.9));
    padding: 2px 5px;
    border-radius: 3px;
  }

  /* Dir */
  .pv-dir {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 6px;
  }
  .dir-count {
    font-size: 36px;
    font-weight: 700;
    color: var(--accent, #8264ff);
    line-height: 1;
  }
  .dir-label {
    font-size: 11px;
    color: var(--text-dim, rgba(255,255,255,0.35));
  }

  /* ── Disk summary (no selection) ── */
  .pv-disk-summary { display: flex; flex-direction: column; gap: 8px; padding: 10px; }
  .pv-disk-title { font-size: 10px; font-weight: 700; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }
  .pv-disk-card { display: flex; flex-direction: column; gap: 6px; padding: 10px; background: var(--bg-tertiary, rgba(30,30,40,0.9)); border: 1px solid var(--border, rgba(255,255,255,0.08)); border-radius: var(--radius-sm); }
  .pv-disk-top { display: flex; align-items: center; gap: 8px; }
  .pv-disk-name { font-size: 12px; font-weight: 600; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pv-disk-dev { font-size: 9px; color: var(--text-dim); font-family: var(--font-mono, monospace); }
  .pv-disk-sz { font-size: 10px; color: var(--text-secondary); font-family: var(--font-mono, monospace); flex-shrink: 0; }
  .pv-disk-hint { font-size: 10px; color: var(--text-dim); text-align: center; font-style: italic; }

  /* Usage bar */
  .pv-usage-track { height: 4px; background: var(--bg-primary, rgba(15,15,20,0.95)); border-radius: 2px; overflow: hidden; }
  .pv-usage-fill { height: 100%; border-radius: 2px; transition: width 0.4s ease; }
  .pv-usage-nums { display: flex; justify-content: space-between; font-size: 9px; margin-top: 3px; }

  /* Big files */
  .pv-section-title { font-size: 10px; font-weight: 700; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; padding: 8px 12px 4px; }
  .pv-big-list { display: flex; flex-direction: column; overflow-y: auto; flex: 1; }
  .pv-big-row { display: flex; align-items: center; justify-content: space-between; padding: 4px 12px; gap: 8px; font-size: 10px; }
  .pv-big-row:hover { background: var(--bg-hover); }
  .pv-big-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-secondary); }
  .pv-big-size { flex-shrink: 0; color: var(--text-dim); font-family: var(--font-mono, monospace); font-size: 9px; }

  .spinner {
    width: 22px; height: 22px;
    border: 2px solid var(--border, rgba(255,255,255,0.08));
    border-top-color: var(--accent, #8264ff);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
