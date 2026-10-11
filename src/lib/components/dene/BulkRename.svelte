<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { selectedPaths, displayEntries, loadDirectory, currentPath, notify } from "../stores/explorer.js";
  import { get } from "svelte/store";

  const dispatch = createEventDispatcher();

  // Seçili dosyaları al (klasörler dahil)
  const allEntries  = get(displayEntries);
  const selPaths    = [...get(selectedPaths)];
  const files       = allEntries.filter(e => selPaths.includes(e.path));

  // Pattern state
  let pattern    = "{name}";      // rename pattern
  let startNum   = 1;             // {n} başlangıcı
  let padLen     = 0;             // 0 = pad yok, 2 = 01, 3 = 001 ...
  let preview    = [];
  let busy       = false;
  let errors     = [];

  // ---- Pattern tokens ----
  // {name}  → orijinal isim (uzantısız)
  // {ext}   → uzantı (nokta ile, boş ise "")
  // {n}     → artan sayı
  // {date}  → bugünün tarihi YYYY-MM-DD
  // {upper} → {name} büyük harf
  // {lower} → {name} küçük harf

  const TODAY = new Date().toISOString().slice(0, 10);

  function applyPattern(entry, index) {
    const dotIdx = entry.name.lastIndexOf(".");
    const hasExt = !entry.is_dir && dotIdx > 0;
    const stem   = hasExt ? entry.name.slice(0, dotIdx) : entry.name;
    const ext    = hasExt ? entry.name.slice(dotIdx) : "";
    const num    = startNum + index;
    const numStr = padLen > 0 ? String(num).padStart(padLen, "0") : String(num);

    return pattern
      .replace(/{name}/g, stem)
      .replace(/{ext}/g,  ext)
      .replace(/{n}/g,    numStr)
      .replace(/{date}/g, TODAY)
      .replace(/{upper}/g, stem.toUpperCase())
      .replace(/{lower}/g, stem.toLowerCase());
  }

  // Reactive preview
  $: preview = files.map((entry, i) => ({
    original: entry.name,
    renamed:  applyPattern(entry, i),
    path:     entry.path,
    dir:      entry.path.replace(/\/[^/]+$/, ""),
    conflict: false,
  }));

  // Çakışma tespiti — aynı klasörde aynı isim
  $: {
    const seen = new Map();
    for (const p of preview) {
      const key = p.dir + "/" + p.renamed;
      if (seen.has(key)) {
        p.conflict = true;
        seen.get(key).conflict = true;
      } else {
        seen.set(key, p);
      }
    }
  }

  $: hasConflict = preview.some(p => p.conflict);
  $: hasChange   = preview.some(p => p.original !== p.renamed);

  async function doRename() {
    if (hasConflict || !hasChange) return;
    busy   = true;
    errors = [];
    for (const p of preview) {
      if (p.original === p.renamed) continue;
      const newPath = p.dir + "/" + p.renamed;
      try {
        await invoke("rename_item", { oldPath: p.path, newPath });
      } catch (e) {
        errors.push(`${p.original}: ${e}`);
      }
    }
    busy = false;
    if (errors.length) {
      notify("error", `${errors.length} rename başarısız`);
    } else {
      notify("success", `${preview.filter(p => p.original !== p.renamed).length} dosya yeniden adlandırıldı`);
      await loadDirectory(get(currentPath));
      dispatch("close");
    }
  }

  // Hazır pattern şablonları
  const TEMPLATES = [
    { label: "Sayısal sıra",  value: "{n}{ext}" },
    { label: "Ad + sıra",     value: "{name}_{n}{ext}" },
    { label: "Tarih + sıra",  value: "{date}_{n}{ext}" },
    { label: "Büyük harf",    value: "{upper}{ext}" },
    { label: "Küçük harf",    value: "{lower}{ext}" },
    { label: "Tarih + ad",    value: "{date}_{name}{ext}" },
  ];
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">

    <div class="panel-header">
      <span>Toplu Yeniden Adlandır — {files.length} dosya</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <div class="panel-body">

      <!-- Controls -->
      <div class="controls">
        <div class="ctrl-row">
          <label class="ctrl-label">Pattern</label>
          <input class="ctrl-input mono" bind:value={pattern} placeholder="{name}{ext}" />
        </div>
        <div class="ctrl-row">
          <label class="ctrl-label">Başlangıç sayısı</label>
          <input class="ctrl-input" type="number" bind:value={startNum} min="0" style="width:80px" />
          <label class="ctrl-label" style="margin-left:12px">Pad uzunluğu</label>
          <input class="ctrl-input" type="number" bind:value={padLen} min="0" max="6" style="width:60px" title="0=kapalı, 2=01 02, 3=001 002"/>
        </div>

        <!-- Şablonlar -->
        <div class="templates">
          {#each TEMPLATES as tmpl}
            <button class="tmpl-btn" on:click={() => pattern = tmpl.value}>{tmpl.label}</button>
          {/each}
        </div>

        <!-- Token referans -->
        <div class="tokens">
          <span class="token">{"{name}"} isim</span>
          <span class="token">{"{ext}"} uzantı</span>
          <span class="token">{"{n}"} sayı</span>
          <span class="token">{"{date}"} tarih</span>
          <span class="token">{"{upper}"} BÜYÜK</span>
          <span class="token">{"{lower}"} küçük</span>
        </div>
      </div>

      <!-- Preview tablosu -->
      <div class="preview-wrap">
        <div class="preview-header">
          <span>Önce</span>
          <span>Sonra</span>
        </div>
        <div class="preview-list">
          {#each preview as p}
            <div class="preview-row" class:conflict={p.conflict} class:unchanged={p.original === p.renamed}>
              <span class="col-old" title={p.path}>{p.original}</span>
              <span class="arrow">→</span>
              <span class="col-new">{p.renamed}</span>
              {#if p.conflict}<span class="badge-conflict">çakışma</span>{/if}
            </div>
          {/each}
        </div>
      </div>

      {#if errors.length}
        <div class="error-list">
          {#each errors as err}<div class="err-row">⚠ {err}</div>{/each}
        </div>
      {/if}

    </div>

    <div class="panel-footer">
      <span class="footer-info">
        {preview.filter(p => p.original !== p.renamed).length} değişecek
        {#if hasConflict}<span class="warn">· çakışma var!</span>{/if}
      </span>
      <button class="btn" on:click={() => dispatch("close")}>İptal</button>
      <button
        class="btn btn-accent"
        on:click={doRename}
        disabled={busy || hasConflict || !hasChange}
      >
        {busy ? "Yeniden adlandırılıyor..." : "Uygula"}
      </button>
    </div>

  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.5);
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    display: flex; align-items: center; justify-content: center;
    z-index: 110;
  }
  .panel {
    width: 680px; max-height: 80vh;
    border-radius: var(--radius-lg);
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
    font-weight: 600; font-size: 13px;
    flex-shrink: 0;
  }
  .panel-header button { background: none; border: none; color: var(--text-dim); cursor: pointer; font-size: 14px; }

  .panel-body { flex: 1; overflow-y: auto; padding: 14px; display: flex; flex-direction: column; gap: 12px; }

  /* Controls */
  .controls { display: flex; flex-direction: column; gap: 8px; }
  .ctrl-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .ctrl-label { font-size: 11px; color: var(--text-dim); white-space: nowrap; }
  .ctrl-input {
    flex: 1; min-width: 80px;
    padding: 5px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    font-size: 12px;
  }
  .ctrl-input.mono { font-family: var(--font-mono); }
  .ctrl-input:focus { border-color: var(--border-accent); outline: none; }

  .templates { display: flex; flex-wrap: wrap; gap: 5px; }
  .tmpl-btn {
    padding: 3px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    font-size: 10px; cursor: pointer;
    transition: border-color 0.1s, color 0.1s;
  }
  .tmpl-btn:hover { border-color: var(--accent); color: var(--text-primary); }

  .tokens { display: flex; flex-wrap: wrap; gap: 6px; }
  .token {
    font-family: var(--font-mono);
    font-size: 9px;
    padding: 2px 6px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-accent);
  }

  /* Preview */
  .preview-wrap { flex: 1; display: flex; flex-direction: column; min-height: 0; }
  .preview-header {
    display: grid; grid-template-columns: 1fr 1fr;
    padding: 4px 10px;
    font-size: 10px; font-weight: 700; color: var(--text-dim);
    text-transform: uppercase; letter-spacing: 0.08em;
    border-bottom: 1px solid var(--border);
  }
  .preview-list {
    flex: 1; overflow-y: auto;
    max-height: 280px;
  }
  .preview-row {
    display: flex; align-items: center; gap: 6px;
    padding: 4px 10px;
    border-bottom: 1px solid transparent;
    font-size: 11px;
    transition: background 0.1s;
  }
  .preview-row:hover { background: var(--bg-hover); }
  .preview-row.conflict { background: rgba(255,95,109,0.08); }
  .preview-row.unchanged { opacity: 0.4; }

  .col-old {
    flex: 1; min-width: 0;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--text-secondary); font-family: var(--font-mono); font-size: 10px;
  }
  .arrow { color: var(--text-dim); flex-shrink: 0; }
  .col-new {
    flex: 1; min-width: 0;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--text-primary); font-family: var(--font-mono); font-size: 10px;
    font-weight: 500;
  }
  .badge-conflict {
    flex-shrink: 0;
    font-size: 9px; padding: 1px 5px;
    background: rgba(255,95,109,0.2); color: var(--danger);
    border: 1px solid rgba(255,95,109,0.3); border-radius: 4px;
  }

  .error-list {
    padding: 8px 10px;
    background: rgba(255,95,109,0.08);
    border: 1px solid rgba(255,95,109,0.2);
    border-radius: var(--radius-sm);
    font-size: 11px;
  }
  .err-row { color: var(--danger); padding: 2px 0; }

  /* Footer */
  .panel-footer {
    display: flex; align-items: center; gap: 8px;
    padding: 10px 14px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }
  .footer-info { flex: 1; font-size: 11px; color: var(--text-dim); }
  .warn { color: var(--danger); }
</style>
