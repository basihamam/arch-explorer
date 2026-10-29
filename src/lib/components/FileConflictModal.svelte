<script>
  import { conflictDialog } from "../stores/explorer.js";
  import { invoke } from "@tauri-apps/api/core";

  let applyToAll = false;
  let srcThumb = null;
  let dstThumb = null;

  $: currentConflict = $conflictDialog?.conflicts?.[$conflictDialog?.currentIndex || 0];
  $: totalConflicts = $conflictDialog?.conflicts?.length || 0;
  $: currentIndex = $conflictDialog?.currentIndex || 0;

  $: if (currentConflict) {
    srcThumb = null;
    dstThumb = null;
    const s = currentConflict.src;
    const d = currentConflict.dst;
    if (!currentConflict.is_dir) {
      invoke("get_thumbnail", { path: s, maxSize: 120 })
        .then(t => { if (currentConflict?.src === s) srcThumb = t; })
        .catch(() => {});
      invoke("get_thumbnail", { path: d, maxSize: 120 })
        .then(t => { if (currentConflict?.dst === d) dstThumb = t; })
        .catch(() => {});
    }
  }

  function getFileIcon(name, isDir) {
    if (isDir) return "📁";
    const ext = (name || "").split(".").pop().toLowerCase();
    const map = {
      png: "🖼", jpg: "🖼", jpeg: "🖼", gif: "🖼", webp: "🖼", svg: "🖼", bmp: "🖼",
      mp4: "🎬", mkv: "🎬", avi: "🎬", mov: "🎬", webm: "🎬", ts: "🎬",
      mp3: "🎵", wav: "🎵", flac: "🎵", ogg: "🎵", m4a: "🎵",
      pdf: "📄", doc: "📄", docx: "📄", txt: "📝", md: "📖",
      zip: "🗜", tar: "🗜", gz: "🗜", "7z": "🗜", rar: "🗜",
      rs: "🦀", py: "🐍", js: "⚡", ts: "⚡", sh: "💲",
      exe: "🪟", deb: "📦",
    };
    return map[ext] || "📄";
  }

  function formatBytes(bytes) {
    if (!bytes || bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }

  function chooseAction(action) {
    if (!$conflictDialog) return;
    $conflictDialog.onResolve(action, applyToAll);
  }

  function handleKeydown(e) {
    if (e.key === "Escape") {
      chooseAction("cancel");
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if $conflictDialog && currentConflict}
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div class="overlay" on:click|self={() => chooseAction("cancel")}>
    <div class="conflict-panel glass">
      <div class="conflict-header">
        <div class="title-wrap">
          <span class="warning-icon">⚠</span>
          <span class="title-text">
            {currentConflict.is_dir ? "Folder Already Exists" : "File Already Exists"}
          </span>
        </div>
        {#if totalConflicts > 1}
          <div class="counter-badge">
            {currentIndex + 1} / {totalConflicts}
          </div>
        {/if}
      </div>

      <div class="conflict-body">
        <p class="conflict-intro">
          A {currentConflict.is_dir ? "folder" : "file"} with the same name already exists in the destination:
        </p>

        <div class="conflict-name-box">
          <span class="item-kind-icon">{getFileIcon(currentConflict.name, currentConflict.is_dir)}</span>
          <span class="item-filename">{currentConflict.name}</span>
        </div>

        <!-- Karşılaştırma kartları (Gelen vs Hedefteki) -->
        <div class="compare-grid">
          <!-- Kaynak (gelen yeni dosya) -->
          <div class="compare-card source">
            <div class="card-label">INCOMING ({$conflictDialog.op === "move" ? "Moving" : "Copying"})</div>
            <div class="thumb-container">
              {#if srcThumb}
                <img src={srcThumb} alt="Incoming Thumbnail" class="file-thumb-img" />
              {:else}
                <div class="file-thumb-placeholder">{getFileIcon(currentConflict.name, currentConflict.is_dir)}</div>
              {/if}
            </div>
            <div class="card-prop">
              <span class="prop-k">Size:</span>
              <span class="prop-v">{formatBytes(currentConflict.src_size)}</span>
            </div>
            <div class="card-prop">
              <span class="prop-k">Date:</span>
              <span class="prop-v">{currentConflict.src_modified || "—"}</span>
            </div>
          </div>

          <!-- Hedef (mevcut olan dosya) -->
          <div class="compare-card dest">
            <div class="card-label">EXISTING (Destination)</div>
            <div class="thumb-container">
              {#if dstThumb}
                <img src={dstThumb} alt="Destination Thumbnail" class="file-thumb-img" />
              {:else}
                <div class="file-thumb-placeholder">{getFileIcon(currentConflict.name, currentConflict.is_dir)}</div>
              {/if}
            </div>
            <div class="card-prop">
              <span class="prop-k">Size:</span>
              <span class="prop-v">{formatBytes(currentConflict.dst_size)}</span>
            </div>
            <div class="card-prop">
              <span class="prop-k">Date:</span>
              <span class="prop-v">{currentConflict.dst_modified || "—"}</span>
            </div>
          </div>
        </div>

        <div class="backup-note">
          <span class="note-icon">🛡️</span>
          <span>
            <strong>Safe Overwrite:</strong> Overwritten items are safely kept in backup for Ctrl+Z undo!
          </span>
        </div>

        <label class="apply-all-label">
          <input type="checkbox" bind:checked={applyToAll} />
          <span>{totalConflicts > 1 ? `Apply to all remaining ${totalConflicts - currentIndex} conflicts` : "Apply to all conflicts"}</span>
        </label>
      </div>

      <!-- Eşit genişlikte, yan yana 4 buton -->
      <div class="conflict-actions">
        <button class="btn btn-cancel" on:click={() => chooseAction("cancel")}>
          Cancel
        </button>
        <button class="btn btn-skip" on:click={() => chooseAction("skip")}>
          Skip
        </button>
        <button class="btn btn-rename" on:click={() => chooseAction("rename")}>
          Keep Both
        </button>
        <button class="btn btn-replace" on:click={() => chooseAction("replace")}>
          Overwrite
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }

  .conflict-panel {
    width: 530px;
    max-width: 95vw;
    background: var(--bg-panel, rgba(22, 22, 32, 0.97));
    border: 1px solid var(--border-accent, rgba(130, 100, 255, 0.45));
    border-radius: var(--radius-lg, 14px);
    padding: 22px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.75), 0 0 0 1px rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    gap: 16px;
    animation: conflictPop 0.22s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes conflictPop {
    from {
      opacity: 0;
      transform: scale(0.92) translateY(10px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .conflict-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    padding-bottom: 12px;
  }

  .title-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .warning-icon {
    font-size: 20px;
    color: var(--warning, #ffd166);
  }

  .title-text {
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary, #fff);
  }

  .counter-badge {
    background: var(--accent-dim, rgba(130, 100, 255, 0.2));
    border: 1px solid var(--accent, #8264ff);
    color: var(--text-accent, #c2a8ff);
    padding: 3px 10px;
    border-radius: 20px;
    font-size: 11px;
    font-weight: 600;
  }

  .conflict-intro {
    font-size: 13px;
    color: var(--text-secondary, rgba(255, 255, 255, 0.75));
    margin-bottom: 10px;
  }

  .conflict-name-box {
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(0, 0, 0, 0.32);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
    border-radius: var(--radius-sm, 8px);
    padding: 9px 12px;
    margin-bottom: 14px;
  }

  .item-kind-icon {
    font-size: 20px;
  }

  .item-filename {
    font-family: var(--font-mono, monospace);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #fff);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compare-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-bottom: 14px;
  }

  .compare-card {
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    border-radius: var(--radius-sm, 8px);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .compare-card.source {
    border-color: rgba(0, 212, 255, 0.35);
    background: rgba(0, 212, 255, 0.03);
  }

  .compare-card.dest {
    border-color: rgba(255, 107, 157, 0.35);
    background: rgba(255, 107, 157, 0.03);
  }

  .card-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim, rgba(255, 255, 255, 0.5));
    margin-bottom: 4px;
  }

  .thumb-container {
    height: 72px;
    background: rgba(0, 0, 0, 0.35);
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    margin-bottom: 6px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .file-thumb-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: 4px;
  }

  .file-thumb-placeholder {
    font-size: 32px;
    opacity: 0.85;
  }

  .card-prop {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
  }

  .prop-k {
    color: var(--text-dim, rgba(255, 255, 255, 0.5));
  }

  .prop-v {
    color: var(--text-primary, #fff);
    font-family: var(--font-mono, monospace);
    font-weight: 500;
  }

  .backup-note {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(130, 100, 255, 0.09);
    border: 1px solid rgba(130, 100, 255, 0.22);
    border-radius: 8px;
    padding: 9px 12px;
    font-size: 11.5px;
    color: var(--text-secondary, rgba(255, 255, 255, 0.85));
    margin-bottom: 14px;
  }

  .note-icon {
    font-size: 15px;
  }

  .apply-all-label {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12px;
    color: var(--text-primary, #fff);
    cursor: pointer;
    user-select: none;
    margin-bottom: 4px;
  }

  .apply-all-label input[type="checkbox"] {
    cursor: pointer;
    accent-color: var(--accent, #8264ff);
    width: 15px;
    height: 15px;
  }

  .conflict-actions {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    border-top: 1px solid var(--border, rgba(255, 255, 255, 0.08));
    padding-top: 14px;
    width: 100%;
  }

  .btn {
    flex: 1;
    min-width: 0;
    padding: 10px 4px;
    font-size: 12px;
    font-weight: 600;
    border-radius: var(--radius-sm, 7px);
    cursor: pointer;
    transition: all 0.15s ease;
    border: 1px solid transparent;
    text-align: center;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .btn-cancel {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.08);
    color: var(--text-dim, rgba(255, 255, 255, 0.6));
  }
  .btn-cancel:hover {
    color: var(--text-primary, #fff);
    background: rgba(255, 255, 255, 0.1);
  }

  .btn-skip {
    background: rgba(255, 255, 255, 0.07);
    border-color: var(--border, rgba(255, 255, 255, 0.12));
    color: var(--text-secondary, rgba(255, 255, 255, 0.9));
  }
  .btn-skip:hover {
    background: rgba(255, 255, 255, 0.13);
    color: var(--text-primary, #fff);
  }

  .btn-rename {
    background: rgba(130, 100, 255, 0.15);
    border-color: rgba(130, 100, 255, 0.4);
    color: var(--text-accent, #d1bdff);
  }
  .btn-rename:hover {
    background: rgba(130, 100, 255, 0.28);
    border-color: var(--accent, #8264ff);
    color: #fff;
  }

  .btn-replace {
    background: var(--accent, #8264ff);
    color: #fff;
    border-color: var(--accent, #8264ff);
    font-weight: 700;
  }
  .btn-replace:hover {
    background: #967aff;
    box-shadow: 0 4px 14px rgba(130, 100, 255, 0.4);
  }
</style>
