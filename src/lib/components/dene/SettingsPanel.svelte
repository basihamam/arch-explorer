<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { theme, opacity, panelOpacity, blurAmount, accentColor, tabBarMode, autoReload, notify, previewMode, previewCustomTypes, thumbnailMode, thumbnailMinSize, gridZoom } from "../stores/explorer.js";

  const dispatch = createEventDispatcher();

  const PRESETS = [
    { name: "Violet Dark",  accent: "#8264ff", theme: "dark",  opacity: 0.82 },
    { name: "Cyan Night",   accent: "#00d4ff", theme: "dark",  opacity: 0.78 },
    { name: "Rose Dark",    accent: "#ff6b9d", theme: "dark",  opacity: 0.80 },
    { name: "Amber Dark",   accent: "#ffd166", theme: "dark",  opacity: 0.80 },
    { name: "Mint Light",   accent: "#00c896", theme: "light", opacity: 0.90 },
    { name: "Sakura Light", accent: "#e8748a", theme: "light", opacity: 0.88 },
    { name: "Full Dark",    accent: "#8264ff", theme: "dark",  opacity: 0.96 },
    { name: "Ghost",        accent: "#8264ff", theme: "dark",  opacity: 0.55 },
  ];

  function applyPreset(p) {
    theme.set(p.theme);
    opacity.set(p.opacity);
    accentColor.set(p.accent);
  }
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">
    <div class="panel-header">
      <span>Appearance Settings</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <div class="settings-body">

      <!-- Theme -->
      <section class="setting-group">
        <label class="group-label">Theme</label>
        <div class="toggle-row">
          <button class="toggle-btn" class:active={$theme === "dark"}  on:click={() => theme.set("dark")}>Dark</button>
          <button class="toggle-btn" class:active={$theme === "light"} on:click={() => theme.set("light")}>Light</button>
        </div>
      </section>

      <!-- Accent Color -->
      <section class="setting-group">
        <label class="group-label">Accent Color</label>
        <div class="color-row">
          <input type="color" bind:value={$accentColor} class="color-picker" />
          <span class="color-val">{$accentColor}</span>
          <div class="color-preview" style="background: {$accentColor}"></div>
        </div>
      </section>

      <!-- Opacity / Transparency -->
      <section class="setting-group">
        <label class="group-label">Background Opacity: {Math.round($opacity * 100)}%</label>
        <input
          type="range"
          min="0.3"
          max="0.99"
          step="0.01"
          bind:value={$opacity}
          class="slider"
        />
        <div class="opacity-labels">
          <span>Ghost</span>
          <span>Full</span>
        </div>
      </section>

      <!-- Blur -->
      <section class="setting-group">
        <label class="group-label">Arayüz Bulanıklığı (Blur): {$blurAmount}px</label>
        <input type="range" min="0" max="40" step="2" bind:value={$blurAmount} class="slider" />
        <div class="opacity-labels"><span>0 (Yok)</span><span>40px (Maksimum)</span></div>
        <p class="hint">Sağ tık menüleri, komut paneli, ayarlar ve açılır pencerelerin arkasındaki cisimleri blurlar. Dosya yöneticisi arka planını etkilemez.</p>
      </section>

      <!-- Panel / Overlay Opacity -->
      <section class="setting-group">
        <label class="group-label">Panel Opaklığı (Modal): {`${Math.round($panelOpacity * 100)}%`}</label>
        <input type="range" min="0.55" max="0.99" step="0.01" bind:value={$panelOpacity} class="slider" />
        <div class="opacity-labels">
          <span>Çok şeffaf</span>
          <span>Opak</span>
        </div>
        <p class="hint">Mount panel, ayarlar, komut paneli gibi ikinci katman pencerelerin opaklığı.</p>
      </section>

      <!-- Presets -->
      <section class="setting-group">
        <label class="group-label">Presets</label>
        <div class="presets">
          {#each PRESETS as p}
            <button
              class="preset-btn"
              style="--pc: {p.accent}"
              on:click={() => applyPreset(p)}
              title={p.name}
            >
              <span class="preset-dot"></span>
              <span class="preset-name">{p.name}</span>
            </button>
          {/each}
        </div>
      </section>

      <!-- Preview Mode -->
      <section class="setting-group">
        <label class="group-label">Preview Panel Modu</label>
        <div class="preview-mode-grid">
          <button class="mode-btn" class:active={$previewMode === "manual"}
            on:click={() => previewMode.set("manual")}>
            <span class="mode-icon">⊡</span>
            <span class="mode-name">Manuel</span>
            <span class="mode-desc">Kapalı — sağ tık → Preview ile açılır</span>
          </button>
          <button class="mode-btn" class:active={$previewMode === "always"}
            on:click={() => previewMode.set("always")}>
            <span class="mode-icon">◫</span>
            <span class="mode-name">Her Zaman</span>
            <span class="mode-desc">Her zaman açık, boşken disk bilgisi</span>
          </button>
          <button class="mode-btn" class:active={$previewMode === "auto"}
            on:click={() => previewMode.set("auto")}>
            <span class="mode-icon">◐</span>
            <span class="mode-name">Otomatik</span>
            <span class="mode-desc">Seçim varsa açık, yoksa boş (disk göstermez)</span>
          </button>
          <button class="mode-btn" class:active={$previewMode === "custom"}
            on:click={() => previewMode.set("custom")}>
            <span class="mode-icon">⚙</span>
            <span class="mode-name">Özel</span>
            <span class="mode-desc">Sadece seçili dosya tipleri için</span>
          </button>
        </div>

        {#if $previewMode === "custom"}
          <div class="custom-types-section">
            <label class="group-label" style="margin-top:8px">Gösterilecek Uzantılar</label>
            <p class="hint">Virgülle ayır: png, jpg, rs, md, pdf</p>
            <input
              class="ctrl-input mono"
              value={$previewCustomTypes.join(", ")}
              on:change={e => {
                const types = e.target.value.split(",").map(s => s.trim().toLowerCase().replace(/^\./, "")).filter(Boolean);
                previewCustomTypes.set(types);
              }}
              placeholder="png, jpg, mp4, pdf, rs, md"
            />
          </div>
        {/if}
      </section>

      <!-- Tab Bar Mode -->
      <section class="setting-group">
        <label class="group-label">Tab Bar</label>
        <div class="toggle-row">
          <button
            class="toggle-btn"
            class:active={$tabBarMode === "always"}
            on:click={() => tabBarMode.set("always")}
            title="Her zaman görünür — tek tab bile olsa bar gösterilir"
          >Always visible</button>
          <button
            class="toggle-btn"
            class:active={$tabBarMode === "lazy"}
            on:click={() => tabBarMode.set("lazy")}
            title="2+ tab açıkken belirir, tek tabta gizlenir"
          >Lazy (2+ tabs)</button>
        </div>
        <p class="hint">
          <strong>Always:</strong> tab bar her zaman görünür, tek tabta bile.<br>
          <strong>Lazy:</strong> sadece 2+ tab açıkken belirir — tek tabta ekstra yer kaplamaz.
        </p>
      </section>

      <!-- Thumbnail Modu -->
      <section class="setting-group">
        <label class="group-label">Thumbnail Modu</label>
        <div class="preview-mode-grid">
          <button class="mode-btn" class:active={$thumbnailMode === "always"}
            on:click={() => thumbnailMode.set("always")}>
            <span class="mode-icon">🖼</span>
            <span class="mode-name">Her Zaman</span>
            <span class="mode-desc">Tüm resim dosyaları her zaman thumbnail gösterir, zoom ne olursa olsun</span>
          </button>
          <button class="mode-btn" class:active={$thumbnailMode === "auto"}
            on:click={() => thumbnailMode.set("auto")}>
            <span class="mode-icon">◐</span>
            <span class="mode-name">Otomatik</span>
            <span class="mode-desc">Grid yeterince büyükse ({$thumbnailMinSize}px+) thumbnail çıkar, küçükse emoji</span>
          </button>
          <button class="mode-btn" class:active={$thumbnailMode === "custom"}
            on:click={() => thumbnailMode.set("custom")}
            style="grid-column: 1/-1">
            <span class="mode-icon">⚙</span>
            <span class="mode-name">Özel</span>
            <span class="mode-desc">Aşağıdan minimum grid boyutunu kendin belirle</span>
          </button>
        </div>

        {#if $thumbnailMode === "auto" || $thumbnailMode === "custom"}
          <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
            <label class="group-label">
              Minimum Grid Boyutu: {$thumbnailMinSize}px
              <span style="color:var(--text-dim); font-weight:400; margin-left:4px">
                (şu an: {$gridZoom}px → thumbnail {$gridZoom >= $thumbnailMinSize ? "✓ görünür" : "✗ gizli"})
              </span>
            </label>
            <input
              type="range" min="60" max="220" step="10"
              bind:value={$thumbnailMinSize}
              on:input={e => thumbnailMinSize.set(parseInt(e.target.value))}
              class="slider"
            />
            <div style="display:flex; justify-content:space-between; font-size:10px; color:var(--text-dim)">
              <span>60px (küçük)</span>
              <span>220px (büyük)</span>
            </div>
          </div>
        {/if}

        <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
          <label class="group-label">Grid Yakınlaştırma: {$gridZoom}px
            <span style="color:var(--text-dim); font-weight:400; margin-left:4px">(Ctrl+Tekerlek ile de ayarlanır)</span>
          </label>
          <input
            type="range" min="60" max="220" step="10"
            bind:value={$gridZoom}
            on:input={e => gridZoom.set(parseInt(e.target.value))}
            class="slider"
          />
          <div style="display:flex; justify-content:space-between; font-size:10px; color:var(--text-dim)">
            <span>60px</span>
            <span class="dim">Varsayılan: 90px</span>
            <span>220px</span>
          </div>
        </div>

        <button class="btn" style="width:fit-content; margin-top:4px"
          on:click={async () => {
            const n = await invoke("clear_thumbnail_cache").catch(() => 0);
            notify("success", `${n} thumbnail cache silindi`);
          }}
        >Thumbnail Cache Temizle</button>
      </section>

      <!-- Auto Reload -->
      <section class="setting-group">
        <label class="group-label">Auto Reload</label>
        <div class="toggle-row">
          <button class="toggle-btn" class:active={$autoReload}  on:click={() => autoReload.set(true)}>Enabled</button>
          <button class="toggle-btn" class:active={!$autoReload} on:click={() => autoReload.set(false)}>Disabled</button>
        </div>
        <p class="hint">
          Klasör içindeki değişiklikleri inotify ile izler, otomatik yeniler.
          Kapatınca sadece F5 ile yenilersin.
        </p>
      </section>

      <!-- Du Cache -->
      <section class="setting-group">
        <label class="group-label">Boyut Önbelleği</label>
        <p class="hint">
          Klasör boyutları hesaplandıktan sonra önbelleğe alınır.
          Normal klasör: 5dk · Sığ dizin: 15dk · Büyük dosyalar: 1sa.
        </p>
        <button class="btn" style="width:fit-content;margin-top:4px"
          on:click={async () => {
            await invoke("clear_du_cache");
            notify("success", "Boyut önbelleği temizlendi");
          }}
        >
          Önbelleği Temizle
        </button>
      </section>

      <!-- Custom CSS -->
      <section class="setting-group">
        <label class="group-label">Custom CSS Variables</label>
        <p class="hint">
          Edit <code>src/app.css</code> to change any <code>--variable</code>.
          Full control, no limits.
        </p>
      </section>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    backdrop-filter: var(--blur-ui) !important;
    -webkit-backdrop-filter: var(--blur-ui) !important;
    display: flex; align-items: center; justify-content: center;
    z-index: 100;
  }
  .panel {
    width: 420px;
    max-height: 80vh;
    border-radius: var(--radius-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
    font-weight: 600;
  }
  .panel-header button { background: none; border: none; color: var(--text-dim); cursor: pointer; font-size: 14px; }
  .settings-body { flex: 1; overflow-y: auto; padding: 16px; display: flex; flex-direction: column; gap: 20px; }

  .setting-group { display: flex; flex-direction: column; gap: 8px; }
  .group-label { font-size: 11px; font-weight: 600; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.08em; }

  .toggle-row { display: flex; gap: 6px; }
  .toggle-btn {
    padding: 5px 16px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 13px;
    transition: var(--transition-fast);
  }
  .toggle-btn.active {
    background: var(--accent-dim);
    border-color: var(--border-accent);
    color: var(--accent);
  }

  .color-row { display: flex; align-items: center; gap: 10px; }
  .color-picker {
    width: 40px; height: 32px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    padding: 2px;
    background: var(--bg-tertiary);
    cursor: pointer;
  }
  .color-val { font-family: var(--font-mono); font-size: 12px; color: var(--text-secondary); }
  .color-preview { width: 24px; height: 24px; border-radius: 50%; }

  .slider {
    width: 100%;
    accent-color: var(--accent);
    cursor: pointer;
  }
  .opacity-labels { display: flex; justify-content: space-between; font-size: 10px; color: var(--text-dim); }

  .presets { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .preset-btn {
    display: flex; align-items: center; gap: 8px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    cursor: pointer;
    transition: var(--transition-fast);
    color: var(--text-primary);
    font-size: 12px;
  }
  .preset-btn:hover { border-color: var(--pc); background: var(--bg-hover); }
  .preset-dot { width: 10px; height: 10px; border-radius: 50%; background: var(--pc); flex-shrink: 0; }
  .preset-name { color: var(--text-secondary); }

  .hint { font-size: 11px; color: var(--text-dim); line-height: 1.6; }
  .hint code { font-family: var(--font-mono); color: var(--text-accent); background: var(--bg-tertiary); padding: 1px 4px; border-radius: 3px; }
  .hint strong { color: var(--text-secondary); font-weight: 600; }

  .preview-mode-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; }
  .mode-btn {
    display: flex; flex-direction: column; gap: 3px; align-items: flex-start;
    padding: 8px 10px;
    border-radius: var(--radius-sm); border: 1px solid var(--border);
    background: var(--bg-tertiary); cursor: pointer;
    text-align: left; transition: border-color 0.1s, background 0.1s;
  }
  .mode-btn:hover { border-color: var(--border-accent); background: var(--bg-hover); }
  .mode-btn.active { border-color: var(--accent); background: var(--accent-dim); }
  .mode-icon { font-size: 14px; color: var(--accent); }
  .mode-name { font-size: 11px; font-weight: 600; color: var(--text-primary); }
  .mode-desc { font-size: 9px; color: var(--text-dim); line-height: 1.4; }

  .custom-types-section { display: flex; flex-direction: column; gap: 4px; margin-top: 4px; }
  .ctrl-input {
    padding: 5px 8px;
    background: var(--bg-tertiary); border: 1px solid var(--border);
    border-radius: var(--radius-sm); color: var(--text-primary); font-size: 12px;
  }
  .ctrl-input.mono { font-family: var(--font-mono); }
  .ctrl-input:focus { border-color: var(--border-accent); outline: none; }
</style>
