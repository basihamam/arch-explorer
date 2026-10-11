<script>
  import { invoke } from "@tauri-apps/api/core";
  import { previewEntry, showPreview } from "../stores/explorer.js";

  let preview   = null;
  let loading   = false;
  let error     = null;

  // previewEntry değişince yeni preview yükle
  $: if ($previewEntry) {
    loadPreview($previewEntry.path);
  } else {
    preview = null;
    error   = null;
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

  function formatSize(bytes) {
    if (!bytes && bytes !== 0) return "—";
    const u = ["B","KB","MB","GB","TB"];
    let i = 0, n = bytes;
    while (n >= 1024 && i < u.length - 1) { n /= 1024; i++; }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  }

  // Media info'dan özet bilgi çıkar
  function getMediaSummary(info) {
    if (!info) return [];
    const rows = [];
    const fmt = info.format;
    if (fmt) {
      if (fmt.duration)   rows.push({ k: "Duration",  v: formatDuration(parseFloat(fmt.duration)) });
      if (fmt.size)       rows.push({ k: "Size",       v: formatSize(parseInt(fmt.size)) });
      if (fmt.bit_rate)   rows.push({ k: "Bitrate",    v: `${Math.round(parseInt(fmt.bit_rate) / 1000)} kbps` });
      if (fmt.format_long_name) rows.push({ k: "Format", v: fmt.format_long_name });
    }
    const streams = info.streams || [];
    for (const s of streams) {
      if (s.codec_type === "video") {
        rows.push({ k: "Video",    v: `${s.codec_name} ${s.width}x${s.height}` });
        if (s.r_frame_rate) rows.push({ k: "FPS", v: evalFraction(s.r_frame_rate) });
      }
      if (s.codec_type === "audio") {
        rows.push({ k: "Audio",    v: `${s.codec_name} ${s.channels}ch ${s.sample_rate}Hz` });
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
    if (!f) return "?";
    const [n, d] = f.split("/").map(Number);
    return d ? `${(n/d).toFixed(2)} fps` : `${n} fps`;
  }

  function kindIcon(kind) {
    return { image:"🖼", text:"📝", video:"🎬", audio:"🎵", dir:"📁", binary:"⬡" }[kind] || "📄";
  }

  function langLabel(lang) {
    return lang ? lang.toUpperCase() : "";
  }
</script>

{#if $showPreview}
  <aside class="preview-panel glass-panel">
    {#if !$previewEntry}
      <div class="preview-empty">
        <span class="empty-icon">◫</span>
        <span>Click a file to preview</span>
      </div>

    {:else if loading}
      <div class="preview-empty">
        <div class="spinner"></div>
        <span>Loading...</span>
      </div>

    {:else if error}
      <div class="preview-empty err">
        <span>⚠</span>
        <span>{error}</span>
      </div>

    {:else if preview}
      <!-- Header -->
      <div class="preview-header">
        <span class="pv-kind-icon">{kindIcon(preview.kind)}</span>
        <div class="pv-title-group">
          <span class="pv-name" title={$previewEntry.path}>{$previewEntry.name}</span>
          <div class="pv-badges">
            <span class="badge">{preview.kind}</span>
            {#if preview.language && preview.kind === "text"}
              <span class="badge lang">{langLabel(preview.language)}</span>
            {/if}
          </div>
        </div>
      </div>

      <!-- File meta -->
      <div class="pv-meta">
        {#if preview.kind !== "dir"}
          <div class="meta-row"><span class="mk">Size</span><span class="mv">{formatSize($previewEntry.size)}</span></div>
        {/if}
        {#if preview.kind === "dir"}
          <div class="meta-row"><span class="mk">Items</span><span class="mv">{preview.line_count ?? 0}</span></div>
        {/if}
        <div class="meta-row"><span class="mk">Modified</span><span class="mv">{$previewEntry.modified}</span></div>
        <div class="meta-row"><span class="mk">Permissions</span><span class="mv mono">{$previewEntry.permissions}</span></div>
        <div class="meta-row"><span class="mk">Type</span><span class="mv mono">{preview.mime_type}</span></div>
        {#if preview.line_count && preview.kind === "text"}
          <div class="meta-row"><span class="mk">Lines</span><span class="mv">{preview.line_count.toLocaleString()}</span></div>
        {/if}
      </div>

      <div class="pv-sep"></div>

      <!-- Content -->
      <div class="pv-content">

        {#if preview.kind === "image"}
          {#if preview.image_b64}
            <img
              class="pv-image"
              src="data:{preview.mime_type};base64,{preview.image_b64}"
              alt={$previewEntry.name}
            />
          {:else}
            <div class="pv-note">{preview.text_content}</div>
          {/if}

        {:else if preview.kind === "text" || preview.kind === "binary"}
          {#if preview.is_truncated}
            <div class="pv-trunc-note">
              Showing first {preview.kind === "binary" ? "512 bytes" : "100 lines"}
            </div>
          {/if}
          <pre class="pv-code">{preview.text_content}</pre>

        {:else if preview.kind === "video" || preview.kind === "audio"}
          {#if preview.media_info}
            <div class="pv-media-table">
              {#each getMediaSummary(preview.media_info) as row}
                <div class="media-row">
                  <span class="mk">{row.k}</span>
                  <span class="mv">{row.v}</span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="pv-note">
              ffprobe bulunamadı — media bilgisi için ffmpeg kur:<br>
              <code>sudo pacman -S ffmpeg</code>
            </div>
          {/if}

        {:else if preview.kind === "dir"}
          <div class="pv-dir-info">
            <span class="dir-count">{preview.line_count ?? 0}</span>
            <span class="dir-label">items in folder</span>
          </div>
        {/if}

      </div>
    {/if}
  </aside>
{/if}

<style>
  .preview-panel {
    width: 260px;
    min-width: 220px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-left: 1px solid var(--border, rgba(255,255,255,0.08));
    border-top: none; border-bottom: none; border-right: none;
    border-radius: 0;
    background: var(--bg-secondary, rgba(22,22,30,0.87));
  }

  .preview-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-dim, rgba(255,255,255,0.3));
    font-size: 12px;
    padding: 20px;
    text-align: center;
  }
  .preview-empty.err { color: var(--danger, #ff5f6d); }
  .empty-icon { font-size: 32px; opacity: 0.4; }

  /* Header */
  .preview-header {
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

  /* Meta table */
  .pv-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 12px;
    flex-shrink: 0;
  }
  .meta-row, .media-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    font-size: 10px;
  }
  .mk {
    color: var(--text-dim, rgba(255,255,255,0.35));
    flex-shrink: 0;
    min-width: 64px;
  }
  .mv {
    color: var(--text-secondary, rgba(255,255,255,0.55));
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 140px;
  }
  .mv.mono { font-family: var(--font-mono, monospace); font-size: 9px; }

  .pv-sep { height: 1px; background: var(--border, rgba(255,255,255,0.08)); flex-shrink: 0; }

  /* Content */
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

  /* Code / text / binary */
  .pv-trunc-note {
    font-size: 9px;
    color: var(--text-dim, rgba(255,255,255,0.3));
    padding: 4px 12px 2px;
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
    color: var(--text-secondary, rgba(255,255,255,0.55));
    overflow: auto;
    white-space: pre;
    background: var(--bg-tertiary, rgba(30,30,40,0.9));
    border-top: 1px solid var(--border, rgba(255,255,255,0.06));
  }

  /* Media */
  .pv-media-table {
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* Note */
  .pv-note {
    padding: 12px;
    font-size: 11px;
    color: var(--text-dim, rgba(255,255,255,0.35));
    line-height: 1.6;
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
  .pv-dir-info {
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

  .spinner {
    width: 22px; height: 22px;
    border: 2px solid var(--border, rgba(255,255,255,0.08));
    border-top-color: var(--accent, #8264ff);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .mono { font-family: var(--font-mono, monospace); }
</style>
