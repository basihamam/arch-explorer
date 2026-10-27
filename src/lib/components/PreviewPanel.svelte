<script>
  import { onMount, onDestroy } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    previewEntry, showPreview, blockDevices,
    previewMode, previewCustomTypes,
    selectedDevice, currentPath, requestSudo,
    openEntry,
  } from "../stores/explorer.js";

  let preview   = null;
  let loading   = false;
  let error     = null;
  let imgSrc    = null;
  let mediaSrc  = null;
  let assetSrc  = null;
  let videoEl = null;
  let videoPoster = null;
  let videoError = false;
  let audioError = false;
  let videoWatchdog = null;
  let isPlayingVideo = false;
  let isVideoFullscreen = false;

  async function toggleVideoFullscreen() {
    isVideoFullscreen = !isVideoFullscreen;
    try {
      const appWin = getCurrentWindow();
      await appWin.setFullscreen(isVideoFullscreen);
    } catch (e) {
      console.warn("setFullscreen failed:", e);
    }
  }

  function handleVideoKeydown(e) {
    if (e.key === "Escape" && isVideoFullscreen) {
      e.stopPropagation();
      e.preventDefault();
      toggleVideoFullscreen();
    }
  }

  function onNativeFullscreenRequest() {
    if (document.fullscreenElement || document.webkitFullscreenElement) {
      if (document.exitFullscreen) document.exitFullscreen().catch(() => {});
      else if (document.webkitExitFullscreen) document.webkitExitFullscreen();
    }
    if (!isVideoFullscreen) {
      toggleVideoFullscreen();
    }
  }

  // Dir du cache
  let duLoading = false;
  let duResult  = null;
  let duPoll    = null;

  function clearDuPoll() {
    if (duPoll) {
      clearInterval(duPoll);
      duPoll = null;
    }
  }

  onDestroy(() => {
    clearDuPoll();
    clearTimeout(previewTimer);
    clearTimeout(duTimer);
    clearTimeout(duLoadingTimeout);
  });

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

  let previewTimer = null;
  let duTimer = null;
  let lastPreviewPath = null;
  let activeEntry = null;

  const IMAGE_EXTS = new Set(["jpg","jpeg","png","gif","webp","svg","bmp","ico","tiff","avif"]);
  function isImage(mime, entry) {
    if (entry && !entry.is_dir) {
      const ext = (entry.extension || entry.name?.split(".").pop() || "").toLowerCase();
      if (IMAGE_EXTS.has(ext)) return true;
    }
    return (mime && IMAGE_MIMES.some(m => mime.startsWith(m.split("/")[0] + "/") && mime === m))
      || (mime && mime.startsWith("image/"));
  }

  const VIDEO_EXTS = new Set(["mp4","mkv","webm","avi","mov","wmv","flv","m4v","ts","mpg","mpeg","3gp","ogv"]);
  function isVideo(entry) {
    if (!entry || entry.is_dir) return false;
    const ext = (entry.extension || entry.name?.split(".").pop() || "").toLowerCase();
    return VIDEO_EXTS.has(ext) || entry.mime_type?.startsWith("video/");
  }

  function loadVideoPreview(entry) {
    const currentPath = entry.path;
    loading = false;
    duLoading = false;
    error = null;
    videoError = false;
    audioError = false;
    isPlayingVideo = false;
    clearTimeout(videoWatchdog);

    if (currentPath.startsWith("adb://")) {
      preview = {
        kind: "video",
        mime_type: entry.mime_type || "video/mp4",
        size: entry.size,
        media_info: null,
      };
      return;
    }

    assetSrc = convertFileSrc(currentPath);
    mediaSrc = null;
    videoPoster = null;
    preview = {
      kind: "video",
      mime_type: entry.mime_type || "video/mp4",
      size: entry.size,
      media_info: null,
    };

    // Arka planda thumbnail poster çek (anında poster gösterimi için)
    invoke("get_thumbnail", { path: currentPath, maxSize: 384 })
      .then(poster => {
        if (poster && lastPreviewPath === currentPath) {
          videoPoster = poster;
        }
      })
      .catch(() => {});

    // Arka planda yerel media server url'sini al (HTTP Range akışı için)
    invoke("get_media_url", { path: currentPath })
      .then(url => {
        if (url && lastPreviewPath === currentPath) {
          mediaSrc = url;
        }
      })
      .catch((e) => {
        console.warn("get_media_url failed, falling back to assetSrc:", e);
        if (lastPreviewPath === currentPath) {
          mediaSrc = assetSrc;
        }
      });

    // Arka planda ffprobe detaylarını al (çözünürlük, süre vs.)
    invoke("get_file_preview", { path: currentPath })
      .then(res => {
        if (res && lastPreviewPath === currentPath && res.media_info) {
          preview = { ...(preview || {}), media_info: res.media_info };
        }
      })
      .catch(() => {});
  }

  // Svelte 5 uyumlu TEK ve doğrudan reaktif blok (aracı değişken zincirleme kopmalarını önler)
  $: {
    const entry = $previewEntry || ($selectedDevice ? null : ($currentPath ? {
      name: $currentPath.split("/").filter(Boolean).pop() || "/",
      path: $currentPath,
      is_dir: true,
      size: 0,
      modified: "",
      permissions: "drwxr-xr-x",
      mime_type: "inode/directory",
      extension: ""
    } : null));

    activeEntry = entry;

    if (entry) {
      if ($selectedDevice) selectedDevice.set(null);

      if (entry.path !== lastPreviewPath) {
        lastPreviewPath = entry.path;
        imgSrc   = null;
        mediaSrc = null;
        assetSrc = null;
        preview  = null;
        error    = null;
        duResult = null;
        videoPoster = null;
        videoError = false;
        audioError = false;

        clearDuPoll();
        clearTimeout(previewTimer);
        clearTimeout(duTimer);
        clearTimeout(duLoadingTimeout);

        if ($previewMode === "custom" && !shouldShowCustom(entry)) {
          loading = false;
          duLoading = false;
        } else if (isImage(entry.mime_type, entry)) {
          if (entry.path.startsWith("adb://")) {
            duLoading = false;
            previewTimer = setTimeout(() => {
              loadPreview(entry.path);
            }, 30);
          } else {
            imgSrc = convertFileSrc(entry.path);
            loading = false;
            duLoading = false;
          }
        } else if (isVideo(entry)) {
          loadVideoPreview(entry);
        } else if (entry.is_dir) {
          loadDirPreview(entry.path);
        } else {
          duLoading = false;
          previewTimer = setTimeout(() => {
            loadPreview(entry.path);
          }, 60);
        }
      }
    } else if (!$selectedDevice) {
      lastPreviewPath = null;
      clearDuPoll();
      clearTimeout(previewTimer);
      clearTimeout(duTimer);
      clearTimeout(duLoadingTimeout);
      preview  = null;
      imgSrc   = null;
      mediaSrc = null;
      assetSrc = null;
      videoPoster = null;
      error    = null;
      loading  = false;
      duLoading = false;
      duResult = null;
    }
  }

  async function loadPreview(path) {
    loading = true;
    error   = null;
    preview = null;
    mediaSrc = null;
    assetSrc = null;
    videoPoster = null;
    videoError = false;
    audioError = false;
    isPlayingVideo = false;
    clearTimeout(videoWatchdog);

    try {
      const res = await invoke("get_file_preview", { path });
      if (lastPreviewPath !== path) return;
      preview = res;
      if (preview && preview.image_b64) {
        imgSrc = `data:${preview.mime_type || "image/jpeg"};base64,${preview.image_b64}`;
      } else if (preview && (preview.kind === "video" || preview.kind === "audio")) {
        if (!path.startsWith("adb://")) {
          assetSrc = convertFileSrc(path);
          try {
            mediaSrc = await invoke("get_media_url", { path });
          } catch (e) {
            console.warn("get_media_url failed, falling back to asset protocol:", e);
            mediaSrc = assetSrc;
          }

          if (preview.kind === "video") {
            invoke("get_thumbnail", { path, maxSize: 384 })
              .then(poster => { if (poster && lastPreviewPath === path) videoPoster = poster; })
              .catch(() => {});
          }
        }
      }
    } catch (e) {
      if (lastPreviewPath === path) error = String(e);
    } finally {
      if (lastPreviewPath === path) loading = false;
    }
  }

  async function openInDefaultApp(path) {
    if (!path) return;
    openEntry({ path, is_dir: false });
  }

  // Client-side dir size cache: path -> duResult
  const dirSizeClientCache = new Map();

  onMount(() => {
    const onClearCache = () => {
      dirSizeClientCache.clear();
      duResult = null;
      if (activeEntry?.is_dir) {
        loadDirPreview(activeEntry.path);
      }
    };
    window.addEventListener("clear-dir-size-cache", onClearCache);
    return () => {
      window.removeEventListener("clear-dir-size-cache", onClearCache);
    };
  });

  async function loadDirPreview(rawPath) {
    const path = rawPath.replace(/\/+$/, "") || "/";
    loading = true;
    error = null;
    preview = null;

    // Eğer bu klasörün boyutu az önce hesaplandıysa veya hafızadaysa hemen göster, dönen çark çıkarma!
    if (dirSizeClientCache.has(path)) {
      duResult = dirSizeClientCache.get(path);
      duLoading = false;
    } else {
      duResult = null;
      duLoading = true;
      clearTimeout(duLoadingTimeout);
      duLoadingTimeout = setTimeout(() => {
        duLoading = false;
      }, 12000);
    }

    // 1. Önce anında dosya önizlemesini yükle (hızlı)
    try {
      const res = await invoke("get_file_preview", { path });
      if (lastPreviewPath !== path) return;
      preview = res;
    } catch (e) {
      if (lastPreviewPath === path) error = String(e);
    } finally {
      if (lastPreviewPath === path) loading = false;
    }

    // ADB dizinlerinde yerel du komutu çalıştırılamaz
    if (path.startsWith("adb://")) {
      duLoading = false;
      return;
    }

    // Eğer zaten kesin sonuca sahipsek arka plan taskini tetiklemeye gerek yok
    if (duResult && !duResult.is_calculating && !duResult.permission_denied) {
      duLoading = false;
      return;
    }

    // 2. Klasör boyutu hesaplamasını 80ms debounce ile arka planda yap
    duTimer = setTimeout(async () => {
      try {
        let du = await invoke("get_dir_size", { path });
        const curNorm = activeEntry?.path?.replace(/\/+$/, "") || "/";
        if (curNorm === path) {
          duResult = du;
          if (du && !du.is_calculating && !du.permission_denied) {
            dirSizeClientCache.set(path, du);
          }
          clearDuPoll();
          // Eğer hala hesaplanıyorsa, 2 saniyede bir kontrol et
          if (du && du.is_calculating) {
            let attempts = 0;
            duPoll = setInterval(async () => {
              attempts++;
              const checkNorm = activeEntry?.path?.replace(/\/+$/, "") || "/";
              if (checkNorm !== path || attempts > 30) {
                clearDuPoll();
                duLoading = false;
                return;
              }
              try {
                const nextDu = await invoke("get_dir_size", { path });
                const recheckNorm = activeEntry?.path?.replace(/\/+$/, "") || "/";
                if (recheckNorm === path) {
                  duResult = nextDu;
                  if (!nextDu.is_calculating) {
                    if (!nextDu.permission_denied) dirSizeClientCache.set(path, nextDu);
                    clearDuPoll();
                    duLoading = false;
                  }
                }
              } catch {
                clearDuPoll();
                duLoading = false;
              }
            }, 2000);
          } else {
            duLoading = false;
          }
        }
      } catch (e) {
        clearDuPoll();
        duLoading = false;
      }
    }, 80);
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

<svelte:window on:keydown={handleVideoKeydown} />

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
        <div class="meta-row"><span class="mk">{$t("device")}</span><span class="mv mono">{$selectedDevice.device}</span></div>
        <div class="meta-row"><span class="mk">{$t("size")}</span><span class="mv">{$selectedDevice.size}</span></div>
        {#if $selectedDevice.mount_point}
          <div class="meta-row"><span class="mk">Mount</span><span class="mv mono" style="font-size:9px">{$selectedDevice.mount_point}</span></div>
        {/if}
        {#if u}
          <div class="meta-row"><span class="mk">{$t("used")}</span><span class="mv">{formatSize(u.used)}</span></div>
          <div class="meta-row"><span class="mk">{$t("free")}</span><span class="mv" style="color:var(--accent)">{formatSize(u.avail)}</span></div>
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

    {:else if !activeEntry}
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
        <span class="pv-kind-icon">{kindIcon(activeEntry, preview?.kind)}</span>
        <div class="pv-title-group">
          <span class="pv-name" title={activeEntry.path}>{activeEntry.name}</span>
          <div class="pv-badges">
            <span class="badge">{activeEntry.extension || activeEntry.mime_type.split("/")[1] || "file"}</span>
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
          {#if activeEntry.is_dir}
            {#if duResult?.permission_denied}
              <span class="mv" style="color:var(--warning)">🔒 Yetki Gerekli</span>
            {:else if duResult && !duResult.is_calculating && duResult.size > 0}
              <span class="mv">{formatSize(duResult.size)}</span>
            {:else if duLoading || (duResult && duResult.is_calculating)}
              <span class="mv" style="color:var(--accent)">Hesaplanıyor…</span>
            {:else}
              <span class="mv">—</span>
            {/if}
          {:else}
            <span class="mv">{formatSize(activeEntry.size)}</span>
          {/if}
        </div>
        <div class="meta-row">
          <span class="mk">Modified</span>
          <span class="mv">{activeEntry.modified || "—"}</span>
        </div>
        <div class="meta-row">
          <span class="mk">Perms</span>
          <span class="mv mono">{activeEntry.permissions || "—"}</span>
        </div>
        {#if activeEntry.is_dir && duResult && !duResult.permission_denied}
          <div class="meta-row">
            <span class="mk">{$t("preview_title") === "Preview" ? "Content" : "İçerik"}</span>
            <span class="mv">{duResult.file_count} {$t("items_count")}</span>
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
          <!-- Image: convertFileSrc ile direkt yükle — başarısız olursa (ör. root dosyası) get_file_preview ile base64 dene -->
          <img class="pv-image" src={imgSrc} alt={activeEntry.name}
            on:error={async () => {
              imgSrc = null;
              await loadPreview(activeEntry.path);
            }} />

        {:else if preview}

          {#if preview.kind === "image" && preview.image_b64}
            <img class="pv-image" src={`data:${preview.mime_type};base64,${preview.image_b64}`} alt={activeEntry.name} />

          {:else if preview.kind === "text" || preview.kind === "binary"}
            {#if preview.is_truncated}
              <div class="pv-trunc">
                İlk {preview.kind === "binary" ? "512 byte" : "100 satır"} gösteriliyor
              </div>
            {/if}
            {#if preview.line_count}
              <div class="pv-trunc">{preview.line_count.toLocaleString()} satır</div>
            {/if}
            <pre class="pv-code">{preview.text_content}</pre>

          {:else if preview.kind === "video"}
            <div class="pv-media-player">
              {#if videoError}
                <div class="pv-video-fallback">
                  <div class="pv-video-fallback-icon">🎬</div>
                  <div class="pv-video-fallback-text">
                    {#if activeEntry.size > 100 * 1024 * 1024}
                      Gömülü WebKit oynatıcısı bu büyük videoyu ({formatSize(activeEntry.size)}) açamadı. Sistem oynatıcınızda (mpv / VLC) anında izleyebilirsiniz:
                    {:else}
                      Bu video formatı WebKit içinde oynatılamadı. Sistem oynatıcısında hemen açabilirsiniz:
                    {/if}
                  </div>
                  <button class="pv-btn-open-app" on:click={() => openInDefaultApp(activeEntry.path)}>
                    ▶ Sistem Oynatıcısında Aç (mpv / VLC)
                  </button>
                </div>
              {:else if mediaSrc}
                <div class="pv-video-wrapper" class:is-fullscreen={isVideoFullscreen}>
                  <div class="pv-video-top-bar">
                    <button
                      class="pv-video-app-btn"
                      on:click={() => openInDefaultApp(activeEntry.path)}
                      title="Varsayılan Oynatıcıda Aç (mpv / VLC)"
                    >
                      <span>▶</span> <span>mpv</span>
                    </button>
                    <button
                      class="pv-video-fs-corner-btn"
                      class:is-active={isVideoFullscreen}
                      on:click={toggleVideoFullscreen}
                      title={isVideoFullscreen ? "Tam Ekrandan Çık (Esc)" : "Tam Ekran (veya videoya çift tıkla)"}
                    >
                      {#if isVideoFullscreen}
                        <span>✕</span> <span>Çık</span>
                      {:else}
                        <span>⛶</span> <span>Tam Ekran</span>
                      {/if}
                    </button>
                  </div>

                  {#key mediaSrc}
                    <!-- svelte-ignore a11y-media-has-caption -->
                    <video
                      src={mediaSrc}
                      poster={videoPoster}
                      controls
                      preload="metadata"
                      playsinline
                      class="pv-video"
                      bind:this={videoEl}
                      on:dblclick={toggleVideoFullscreen}
                      on:webkitfullscreenchange={onNativeFullscreenRequest}
                      on:fullscreenchange={onNativeFullscreenRequest}
                      on:error={(e) => {
                        console.warn("Video element error:", e);
                        videoError = true;
                      }}
                    >
                      Tarayıcınız video oynatmayı desteklemiyor.
                    </video>
                  {/key}
                </div>
              {:else}
                <div class="pv-video-poster-card">
                  {#if videoPoster}
                    <img src={videoPoster} alt={activeEntry.name} class="pv-video-poster-img" />
                  {:else}
                    <div class="pv-video-poster-placeholder">🎬</div>
                  {/if}
                  <div class="pv-video-badge-overlay">
                    <span class="video-overlay-badge">{activeEntry.extension?.toUpperCase() || "VİDEO"}</span>
                  </div>
                </div>
              {/if}
            </div>
            {#if preview.media_info}
              <div class="pv-media-table">
                {#each getMediaSummary(preview.media_info) as row}
                  <div class="meta-row">
                    <span class="mk">{row.k}</span>
                    <span class="mv">{row.v}</span>
                  </div>
                {/each}
              </div>
            {/if}

          {:else if preview.kind === "audio"}
            <div class="pv-media-player pv-audio-wrap">
              <div class="pv-audio-icon">🎵</div>
              {#if audioError}
                <div class="pv-video-fallback">
                  <div class="pv-video-fallback-text">Ses formatı WebKit içinde oynatılamadı.</div>
                  <button class="pv-btn-open-app" on:click={() => openInDefaultApp(activeEntry.path)}>
                    ▶ Sistem Oynatıcısında Aç
                  </button>
                </div>
              {:else if mediaSrc || assetSrc}
                {#key mediaSrc || assetSrc}
                  <audio
                    src={mediaSrc || assetSrc}
                    controls
                    preload="auto"
                    class="pv-audio"
                    on:error={() => {
                      if (mediaSrc && assetSrc && mediaSrc !== assetSrc) {
                        mediaSrc = assetSrc;
                      } else {
                        audioError = true;
                      }
                    }}
                  >
                    Tarayıcınız ses oynatmayı desteklemiyor.
                  </audio>
                {/key}
              {:else}
                <div class="spinner"></div>
              {/if}
            </div>
            {#if preview.media_info}
              <div class="pv-media-table">
                {#each getMediaSummary(preview.media_info) as row}
                  <div class="meta-row">
                    <span class="mk">{row.k}</span>
                    <span class="mv">{row.v}</span>
                  </div>
                {/each}
              </div>
            {/if}

          {:else if preview.kind === "dir"}
            <div class="pv-dir">
              {#if duResult?.permission_denied}
                <span class="dir-count" style="font-size:24px; color:var(--warning)">🔒</span>
                <span class="dir-label" style="color:var(--warning)">Permission Denied</span>
                <button
                  class="btn btn-sm btn-accent"
                  style="margin-top:10px; font-size:11px; padding:4px 10px; border-radius:6px; cursor:pointer;"
                  on:click={calculateDirWithSudo}
                  disabled={duLoading}
                >
                  {duLoading ? "Calculating…" : "Calculate Size with Root"}
                </button>
              {:else if duResult && !duResult.is_calculating && duResult.size > 0}
                <span class="dir-count">{formatSize(duResult.size)}</span>
                <span class="dir-label">{duResult.file_count} {$t("items_count")}</span>
                {#if duResult.from_cache}
                  <span style="font-size:9px;color:var(--text-dim);font-style:italic">cached</span>
                {/if}
              {:else if duLoading || (duResult && duResult.is_calculating)}
                <span class="dir-count">{duResult?.file_count ?? preview.line_count ?? 0}</span>
                <span class="dir-label" style="display:flex;align-items:center;gap:6px;justify-content:center">
                  <div class="spinner" style="width:12px;height:12px;border-width:2px;display:inline-block"></div>
                  {$t("items_count")} ({$t("scanning")})
                </span>
              {:else}
                <span class="dir-count">{preview.line_count ?? 0}</span>
                <span class="dir-label">{$t("items_count")}</span>
              {/if}
            </div>

          {/if}
        {/if}

      </div>
    {/if}
  </aside>

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
    overflow-y: auto;
    overflow-x: hidden;
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
    white-space: pre-wrap;
    word-break: break-word;
    overflow-wrap: anywhere;
    background: var(--bg-tertiary, rgba(30,30,40,0.9));
  }

  /* Media */
  .pv-media-player {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.3);
    border-radius: 6px;
    overflow: hidden;
    margin-bottom: 8px;
    border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    flex-shrink: 0;
  }
  .pv-video-poster-card {
    width: 100%;
    min-height: 200px;
    max-height: min(650px, 65vh);
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #000;
    border-radius: 6px;
    overflow: hidden;
  }
  .pv-video-poster-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .pv-video-poster-placeholder {
    font-size: 48px;
    opacity: 0.6;
  }
  .pv-video-play-center {
    position: absolute;
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: rgba(130, 100, 255, 0.9);
    border: 2px solid rgba(255, 255, 255, 0.9);
    color: #fff;
    font-size: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding-left: 4px;
    cursor: pointer;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.6);
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }
  .pv-video-play-center:hover {
    transform: scale(1.12);
    background: var(--accent, #8264ff);
    box-shadow: 0 6px 25px rgba(130, 100, 255, 0.7);
  }
  .pv-video-badge-overlay {
    position: absolute;
    bottom: 8px;
    right: 8px;
  }
  .video-overlay-badge {
    background: rgba(0, 0, 0, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 10px;
    font-weight: 700;
    color: #ffd166;
    letter-spacing: 0.05em;
  }
  .pv-quick-primary {
    background: var(--accent, #8264ff) !important;
    border-color: var(--accent, #8264ff) !important;
    color: #fff !important;
    font-weight: 600 !important;
  }
  .pv-video-wrapper {
    width: 100%;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .pv-video-wrapper.is-fullscreen {
    position: fixed !important;
    inset: 0 !important;
    width: 100vw !important;
    height: 100vh !important;
    z-index: 99999 !important;
    background: #000000 !important;
    display: flex !important;
    align-items: center !important;
    justify-content: center !important;
  }
  .pv-video {
    width: 100%;
    height: auto;
    max-height: min(650px, 65vh);
    object-fit: contain;
    outline: none;
    background: #000;
  }
  .pv-video-wrapper.is-fullscreen .pv-video {
    width: 100% !important;
    height: 100% !important;
    max-height: 100vh !important;
    max-width: 100vw !important;
    object-fit: contain !important;
    background: transparent !important;
  }
  .pv-video:fullscreen,
  .pv-video:-webkit-full-screen {
    max-height: 100vh !important;
    width: 100vw !important;
    height: 100vh !important;
    object-fit: contain !important;
    background: transparent !important;
    background-color: transparent !important;
  }

  /* Video üst bar ve aksiyon butonları */
  .pv-video-top-bar {
    position: absolute;
    top: 8px;
    left: 8px;
    right: 8px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    z-index: 20;
    pointer-events: none;
  }
  .pv-video-app-btn,
  .pv-video-fs-corner-btn {
    pointer-events: auto;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    font-size: 11px;
    font-weight: 600;
    color: #ffffff;
    background: rgba(15, 15, 20, 0.75);
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 6px;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    opacity: 0.85;
  }
  .pv-video-wrapper:hover .pv-video-app-btn,
  .pv-video-wrapper:hover .pv-video-fs-corner-btn {
    opacity: 1;
  }
  .pv-video-app-btn:hover {
    background: #10b981;
    border-color: #10b981;
    transform: scale(1.04);
  }
  .pv-video-fs-corner-btn:hover {
    background: var(--accent, #8264ff);
    border-color: var(--accent, #8264ff);
    color: #fff;
    transform: translateY(-1px);
    box-shadow: 0 4px 14px rgba(130, 100, 255, 0.5);
  }
  .pv-video-fs-corner-btn.is-active {
    background: rgba(30, 30, 40, 0.85);
    border-color: rgba(255, 255, 255, 0.35);
  }
  .pv-video-fs-corner-btn.is-active:hover {
    background: var(--danger, #ff5f6d);
    border-color: var(--danger, #ff5f6d);
  }
  .pv-video-fallback {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px 16px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px dashed rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    text-align: center;
    width: 100%;
  }
  .pv-video-fallback-icon {
    font-size: 32px;
  }
  .pv-video-fallback-text {
    font-size: 11px;
    color: var(--text-dim, #888);
    max-width: 220px;
    line-height: 1.4;
  }
  .pv-btn-open-app {
    background: var(--accent, #8264ff);
    color: #fff;
    border: none;
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: opacity 0.2s ease;
  }
  .pv-btn-open-app:hover {
    opacity: 0.85;
  }
  .pv-audio-wrap {
    padding: 16px 12px;
    gap: 10px;
  }
  .pv-audio-icon {
    font-size: 28px;
  }
  .pv-audio {
    width: 100%;
    outline: none;
  }

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
