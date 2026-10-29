<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    theme, opacity, panelOpacity, blurAmount, accentColor, tabBarMode, autoReload, notify,
    previewMode, previewCustomTypes, thumbnailMode, thumbnailMinSize, gridZoom,
    adminMode, alwaysLaunchSudo, sessionSudoPassword, requestSudo,
    bookmarkIconsEnabled, videoHoverScrubbing, conflictFileAction, conflictDirAction,
    autoCrashReport, showPreview, folderPreviewEnabled
  } from "../stores/explorer.js";
  import { t, currentLocale, setLocale } from "../i18n.js";

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

  async function toggleAdminMode(enable) {
    if (enable) {
      if ($sessionSudoPassword) {
        adminMode.set(true);
        notify("success", $currentLocale === "tr" ? "Yönetici (Admin) Modu aktif edildi" : "Admin Mode enabled");
      } else {
        try {
          await requestSudo(
            $currentLocale === "tr" ? "Yönetici (Admin) Modunu Etkinleştir" : "Enable Admin Mode",
            $currentLocale === "tr" ? "Uygulamanın şifre sormadan doğrudan root yetkisiyle çalışması için sudo şifrenizi girin." : "Enter sudo password to run with root privileges without prompting.",
            async (pwd) => {
              await invoke("verify_sudo_password", { password: pwd });
              sessionSudoPassword.set(pwd);
              adminMode.set(true);
              notify("success", $currentLocale === "tr" ? "Yönetici (Admin) Modu aktif edildi" : "Admin Mode enabled");
            }
          );
        } catch (e) {
          // cancelled or failed
        }
      }
    } else {
      adminMode.set(false);
      notify("info", $currentLocale === "tr" ? "Yönetici (Admin) Modu kapatıldı" : "Admin Mode disabled");
    }
  }

  async function toggleAlwaysLaunchSudo(enable) {
    if (enable) {
      alwaysLaunchSudo.set(true);
      notify("success", $currentLocale === "tr" ? "Her açılışta sudo ile başlatma aktif edildi" : "Always launch with sudo enabled");
      if (!$sessionSudoPassword) {
        toggleAdminMode(true);
      }
    } else {
      alwaysLaunchSudo.set(false);
      notify("info", $currentLocale === "tr" ? "Her açılışta sudo ile başlatma kapatıldı" : "Always launch with sudo disabled");
    }
  }

  async function openUrl(url) {
    try {
      await invoke("open_external_url", { url });
    } catch (e) {
      window.open(url, "_blank");
    }
  }

  async function reportBug() {
    openUrl("https://github.com/basihamam/arch-explorer-tauri/issues/new?title=Bug%20Report&body=Describe%20the%20issue%20here...");
  }

  async function sendTestCrashReport() {
    try {
      const title = "User Test Diagnostics Report";
      const body = `### Arch Explorer Diagnostic Ping\n- **Time**: ${new Date().toISOString()}\n- **Platform**: Arch Linux GTK WebKit\n- **Version**: 0.1v\n\nDiagnostic test ping generated from Settings -> About.`;
      const res = await invoke("send_crash_report", {
        title,
        body
      });
      // Also open pre-filled GitHub issue in the default browser so it directly reaches @basihamam
      const issueUrl = `https://github.com/basihamam/arch-explorer-tauri/issues/new?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}`;
      openUrl(issueUrl);
      notify("success", $currentLocale === "tr" ? `Rapor kaydedildi ve GitHub Issue açıldı! (${res})` : `Report saved and GitHub Issue opened! (${res})`);
    } catch (e) {
      notify("error", String(e));
    }
  }

  let activeTab = "appearance"; // "appearance" | "layout" | "behavior" | "admin" | "advanced" | "about"
</script>

<div class="overlay" on:click|self={() => dispatch("close")}>
  <div class="panel glass">
    <div class="panel-header">
      <span>{$t("settings")}</span>
      <button on:click={() => dispatch("close")}>✕</button>
    </div>

    <!-- Tab navigation -->
    <div class="settings-tabs">
      <button class="tab-btn" class:active={activeTab === "appearance"} on:click={() => activeTab = "appearance"}>{$t("tab_appearance")}</button>
      <button class="tab-btn" class:active={activeTab === "layout"} on:click={() => activeTab = "layout"}>{$t("tab_layout")}</button>
      <button class="tab-btn" class:active={activeTab === "behavior"} on:click={() => activeTab = "behavior"}>{$t("tab_behavior")}</button>
      <button class="tab-btn" class:active={activeTab === "admin"} on:click={() => activeTab = "admin"}>{$t("tab_admin")}</button>
      <button class="tab-btn" class:active={activeTab === "advanced"} on:click={() => activeTab = "advanced"}>{$t("tab_advanced")}</button>
      <button class="tab-btn" class:active={activeTab === "about"} on:click={() => activeTab = "about"}>{$t("tab_about")}</button>
    </div>

    <div class="settings-body">

      <!-- ================= TAB: GÖRÜNÜM / APPEARANCE ================= -->
      {#if activeTab === "appearance"}
        <!-- Dil / Language -->
        <section class="setting-group">
          <label class="group-label">{$t("language")} (Language)</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$currentLocale === "tr"} on:click={() => setLocale("tr")}>
              🇹🇷 Türkçe
            </button>
            <button class="toggle-btn" class:active={$currentLocale === "en"} on:click={() => setLocale("en")}>
              🇬🇧 English
            </button>
          </div>
        </section>

        <!-- Theme -->
        <section class="setting-group">
          <label class="group-label">{$t("theme")}</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$theme === "dark"}  on:click={() => theme.set("dark")}>Dark</button>
            <button class="toggle-btn" class:active={$theme === "light"} on:click={() => theme.set("light")}>Light</button>
          </div>
        </section>

        <!-- Accent Color -->
        <section class="setting-group">
          <label class="group-label">{$t("accent_color")}</label>
          <div class="color-row">
            <input type="color" bind:value={$accentColor} class="color-picker" />
            <span class="color-val">{$accentColor}</span>
            <div class="color-preview" style="background: {$accentColor}"></div>
          </div>
        </section>

        <!-- Opacity / Transparency -->
        <section class="setting-group">
          <label class="group-label">{$t("bg_opacity")}: {Math.round($opacity * 100)}%</label>
          <input
            type="range"
            min="0.3"
            max="0.99"
            step="0.01"
            bind:value={$opacity}
            class="slider"
          />
          <div class="opacity-labels">
            <span>{$t("ghost_transparent")}</span>
            <span>{$t("full_opaque")}</span>
          </div>
        </section>

        <!-- Blur -->
        <section class="setting-group">
          <label class="group-label">{$t("ui_blur")}: {$blurAmount}px</label>
          <input type="range" min="0" max="40" step="2" bind:value={$blurAmount} class="slider" />
          <div class="opacity-labels"><span>0</span><span>40px</span></div>
          <p class="hint">{$t("blur_hint")}</p>
        </section>

        <!-- Panel / Overlay Opacity -->
        <section class="setting-group">
          <label class="group-label">{$t("panel_opacity")}: {`${Math.round($panelOpacity * 100)}%`}</label>
          <input type="range" min="0.55" max="0.99" step="0.01" bind:value={$panelOpacity} class="slider" />
          <div class="opacity-labels">
            <span>{$t("very_transparent")}</span>
            <span>{$t("opaque")}</span>
          </div>
          <p class="hint">{$t("panel_opacity_hint")}</p>
        </section>

        <!-- Presets -->
        <section class="setting-group">
          <label class="group-label">{$t("preset_themes")}</label>
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
      {/if}

      <!-- ================= TAB: YERLEŞİM / LAYOUT ================= -->
      {#if activeTab === "layout"}
        <!-- Yer İşaretleri İkonları -->
        <section class="setting-group">
          <label class="group-label">{$t("bookmark_icons")}</label>
          <div class="toggle-row">
            <button
              class="toggle-btn"
              class:active={!$bookmarkIconsEnabled}
              on:click={() => bookmarkIconsEnabled.set(false)}
            >{$t("bookmark_icons_classic")}</button>
            <button
              class="toggle-btn"
              class:active={$bookmarkIconsEnabled}
              on:click={() => bookmarkIconsEnabled.set(true)}
            >{$t("bookmark_icons_custom")}</button>
          </div>
          <p class="hint">{$t("bookmark_icons_hint")}</p>
        </section>

        <!-- Tab Bar Mode -->
        <section class="setting-group">
          <label class="group-label">{$t("tab_bar")}</label>
          <div class="toggle-row">
            <button
              class="toggle-btn"
              class:active={$tabBarMode === "always"}
              on:click={() => tabBarMode.set("always")}
            >{$t("tab_bar_always")}</button>
            <button
              class="toggle-btn"
              class:active={$tabBarMode === "lazy"}
              on:click={() => tabBarMode.set("lazy")}
            >{$t("tab_bar_lazy")}</button>
          </div>
          <p class="hint">
            <strong>{$t("tab_bar_always")}:</strong> {$t("tab_bar_hint_always")}<br>
            <strong>{$t("tab_bar_lazy")}:</strong> {$t("tab_bar_hint_lazy")}
          </p>
        </section>

        <!-- Preview Mode -->
        <section class="setting-group">
          <label class="group-label">{$t("preview_mode_title")}</label>
          <div class="preview-mode-grid">
            <button class="mode-btn" class:active={$previewMode === "manual"}
              on:click={() => { previewMode.set("manual"); showPreview.set(false); }}>
              <span class="mode-icon">⊡</span>
              <span class="mode-name">{$t("mode_manual")}</span>
              <span class="mode-desc">{$t("mode_manual_desc")}</span>
            </button>
            <button class="mode-btn" class:active={$previewMode === "always"}
              on:click={() => { previewMode.set("always"); showPreview.set(true); }}>
              <span class="mode-icon">◫</span>
              <span class="mode-name">{$t("mode_always")}</span>
              <span class="mode-desc">{$t("mode_always_desc")}</span>
            </button>
            <button class="mode-btn" class:active={$previewMode === "auto"}
              on:click={() => { previewMode.set("auto"); showPreview.set(true); }}>
              <span class="mode-icon">◐</span>
              <span class="mode-name">{$t("mode_auto")}</span>
              <span class="mode-desc">{$t("mode_auto_desc")}</span>
            </button>
            <button class="mode-btn" class:active={$previewMode === "custom"}
              on:click={() => { previewMode.set("custom"); }}>
              <span class="mode-icon">⚙</span>
              <span class="mode-name">{$t("mode_custom")}</span>
              <span class="mode-desc">{$t("mode_custom_desc")}</span>
            </button>
          </div>

          {#if $previewMode === "custom"}
            <div class="custom-types-section">
              <label class="group-label" style="margin-top:8px">{$t("preview_extensions")}</label>
              <p class="hint">{$t("preview_extensions_hint")}</p>
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

        <!-- Klasör Önizleme Ayarı -->
        <section class="setting-group">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
            <label class="group-label" style="margin-bottom:0">{$t("folder_preview_title")}</label>
            <div class="toggle-row">
              <button
                class="toggle-btn"
                class:active={$folderPreviewEnabled}
                on:click={() => folderPreviewEnabled.set(true)}
              >{$t("on")}</button>
              <button
                class="toggle-btn"
                class:active={!$folderPreviewEnabled}
                on:click={() => folderPreviewEnabled.set(false)}
              >{$t("off")}</button>
            </div>
          </div>
          <p class="hint">
            <strong>{$t("on")}:</strong> {$t("folder_preview_hint_on")}<br>
            <strong>{$t("off")}:</strong> {$t("folder_preview_hint_off")}
          </p>
        </section>

        <!-- Grid & Thumbnails -->
        <section class="setting-group">
          <label class="group-label">{$t("grid_thumbnail_size")}</label>
          <div class="preview-mode-grid">
            <button class="mode-btn" class:active={$thumbnailMode === "always"}
              on:click={() => thumbnailMode.set("always")}>
              <span class="mode-icon">🖼</span>
              <span class="mode-name">{$t("mode_always")}</span>
              <span class="mode-desc">{$t("grid_thumb_always_desc")}</span>
            </button>
            <button class="mode-btn" class:active={$thumbnailMode === "auto"}
              on:click={() => thumbnailMode.set("auto")}>
              <span class="mode-icon">◐</span>
              <span class="mode-name">{$t("mode_auto")}</span>
              <span class="mode-desc">{$t("grid_thumb_auto_desc")}</span>
            </button>
            <button class="mode-btn" class:active={$thumbnailMode === "custom"}
              on:click={() => thumbnailMode.set("custom")}
              style="grid-column: 1/-1">
              <span class="mode-icon">⚙</span>
              <span class="mode-name">{$t("mode_custom")}</span>
              <span class="mode-desc">{$t("grid_thumb_custom_desc")}</span>
            </button>
          </div>

          {#if $thumbnailMode === "auto" || $thumbnailMode === "custom"}
            <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
              <label class="group-label">
                {$t("min_thumb_size")}: {$thumbnailMinSize}px
                <span style="color:var(--text-dim); font-weight:400; margin-left:4px">
                  ({$gridZoom}px → thumbnail {$gridZoom >= $thumbnailMinSize ? "✓" : "✗"})
                </span>
              </label>
              <input
                type="range" min="60" max="220" step="10"
                bind:value={$thumbnailMinSize}
                on:input={e => thumbnailMinSize.set(parseInt(e.target.value))}
                class="slider"
              />
            </div>
          {/if}

          <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
            <label class="group-label">{$t("grid_zoom")}: {$gridZoom}px
              <span style="color:var(--text-dim); font-weight:400; margin-left:4px">(Ctrl + Wheel)</span>
            </label>
            <input
              type="range" min="60" max="220" step="10"
              bind:value={$gridZoom}
              on:input={e => gridZoom.set(parseInt(e.target.value))}
              class="slider"
            />
          </div>
        </section>
      {/if}

      <!-- ================= TAB: DAVRANIŞ / BEHAVIOR ================= -->
      {#if activeTab === "behavior"}
        <!-- Auto Reload -->
        <section class="setting-group">
          <label class="group-label">{$t("auto_reload_title")}</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$autoReload}  on:click={() => autoReload.set(true)}>{$t("on")}</button>
            <button class="toggle-btn" class:active={!$autoReload} on:click={() => autoReload.set(false)}>{$t("off")}</button>
          </div>
          <p class="hint">{$t("auto_reload_hint")}</p>
        </section>

        <!-- Du Cache -->
        <section class="setting-group">
          <label class="group-label">{$t("du_cache_title")}</label>
          <p class="hint">{$t("du_cache_hint")}</p>
          <button class="btn" style="width:fit-content;margin-top:4px"
            on:click={async () => {
              const n = await invoke("clear_du_cache").catch(() => 0);
              window.dispatchEvent(new CustomEvent("clear-dir-size-cache"));
              notify("success", $currentLocale === "tr" ? `${n} boyut önbellek kaydı silindi ve sıfırlandı` : `${n} dir size cache entries cleared`);
            }}
          >
            {$t("du_cache_clear")}
          </button>
        </section>

        <!-- Video Hover Scrubbing -->
        <section class="setting-group">
          <label class="group-label">{$t("hover_scrub_title")}</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$videoHoverScrubbing} on:click={() => videoHoverScrubbing.set(true)}>{$t("on")}</button>
            <button class="toggle-btn" class:active={!$videoHoverScrubbing} on:click={() => videoHoverScrubbing.set(false)}>{$t("off")}</button>
          </div>
          <p class="hint">{$t("hover_scrub_hint")}</p>
        </section>

        <!-- Conflict Resolution: Files & Folders -->
        <section class="setting-group">
          <label class="group-label">{$t("conflict_management_title")}</label>
          
          <div style="display:flex; flex-direction:column; gap:12px; margin-top:6px">
            <div>
              <span style="font-size:12px; font-weight:600; color:var(--text-secondary)">{$t("conflict_on_files")}</span>
              <div class="toggle-row" style="margin-top:4px">
                <button class="toggle-btn" class:active={$conflictFileAction === "ask"} on:click={() => conflictFileAction.set("ask")}>{$t("conflict_always_ask")}</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "replace"} on:click={() => conflictFileAction.set("replace")}>{$t("conflict_replace")}</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "rename"} on:click={() => conflictFileAction.set("rename")}>{$t("conflict_keep_both")}</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "skip"} on:click={() => conflictFileAction.set("skip")}>{$t("conflict_skip")}</button>
              </div>
            </div>

            <div>
              <span style="font-size:12px; font-weight:600; color:var(--text-secondary)">{$t("conflict_on_dirs")}</span>
              <div class="toggle-row" style="margin-top:4px">
                <button class="toggle-btn" class:active={$conflictDirAction === "ask"} on:click={() => conflictDirAction.set("ask")}>{$t("conflict_always_ask")}</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "replace"} on:click={() => conflictDirAction.set("replace")}>{$t("conflict_replace_merge")}</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "rename"} on:click={() => conflictDirAction.set("rename")}>{$t("conflict_keep_both")}</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "skip"} on:click={() => conflictDirAction.set("skip")}>{$t("conflict_skip")}</button>
              </div>
            </div>
          </div>
          <p class="hint" style="margin-top:8px">{$t("conflict_backup_hint")}</p>
        </section>

        <!-- Thumbnail Cache & RAM Cleanup -->
        <section class="setting-group">
          <label class="group-label">{$t("ram_cache_cleanup_title")}</label>
          <p class="hint">{$t("ram_cache_cleanup_hint")}</p>
          <div style="display:flex; gap:8px; margin-top:6px; flex-wrap:wrap">
            <button class="btn" style="flex:1"
              on:click={async () => {
                window.dispatchEvent(new CustomEvent('arch:free-ram'));
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", $currentLocale === "tr" ? `⚡ RAM & Görsel Önbelleği temizlendi! (${n} önbellek silindi)` : `⚡ RAM & Image Cache cleared! (${n} entries freed)`);
              }}
            >
              {$t("clear_ram_now")}
            </button>
            <button class="btn" style="flex:1"
              on:click={async () => {
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", $currentLocale === "tr" ? `${n} thumbnail önbellek kaydı silindi` : `${n} thumbnail cache items deleted`);
              }}
            >
              {$t("clear_thumbs_only")}
            </button>
          </div>
        </section>
      {/if}

      <!-- ================= TAB: YÖNETİCİ / ADMIN ================= -->
      {#if activeTab === "admin"}
        <!-- Admin Mode & Sudo Settings -->
        <section class="setting-group">
          <label class="group-label">{$t("admin_privileges_title")}</label>
          
          <div style="display:flex; flex-direction:column; gap:16px;">
            <div>
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
                <span style="font-weight:600; font-size:13px">{$t("admin_mode_label")}</span>
                <div class="toggle-row">
                  <button class="toggle-btn" class:active={$adminMode} on:click={() => toggleAdminMode(true)}>{$t("on")}</button>
                  <button class="toggle-btn" class:active={!$adminMode} on:click={() => toggleAdminMode(false)}>{$t("off")}</button>
                </div>
              </div>
              <p class="hint">{$t("admin_mode_hint")}</p>
            </div>

            <div>
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
                <span style="font-weight:600; font-size:13px">{$t("always_sudo_label")}</span>
                <div class="toggle-row">
                  <button class="toggle-btn" class:active={$alwaysLaunchSudo} on:click={() => toggleAlwaysLaunchSudo(true)}>{$t("on")}</button>
                  <button class="toggle-btn" class:active={!$alwaysLaunchSudo} on:click={() => toggleAlwaysLaunchSudo(false)}>{$t("off")}</button>
                </div>
              </div>
              <p class="hint">{$t("always_sudo_hint")}</p>
            </div>
          </div>
        </section>
      {/if}

      <!-- ================= TAB: GELİŞMİŞ / ADVANCED ================= -->
      {#if activeTab === "advanced"}
        <!-- RAM & Memory Optimization -->
        <section class="setting-group">
          <label class="group-label">{$t("perf_optimization_title")}</label>
          <p class="hint">{$t("perf_optimization_hint")}</p>
          <div style="display:flex; gap:8px; margin-top:8px">
            <button class="btn" style="flex:1"
              on:click={async () => {
                window.dispatchEvent(new CustomEvent('arch:free-ram'));
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", $currentLocale === "tr" ? `⚡ RAM ve Görsel Önbelleği temizlendi! (${n} önbellek boşaltıldı)` : `⚡ RAM & Image cache cleaned! (${n} entries freed)`);
              }}
            >
              {$t("clear_ram_advanced")}
            </button>
          </div>
        </section>

        <!-- Custom CSS -->
        <section class="setting-group">
          <label class="group-label">{$t("custom_css_title")}</label>
          <p class="hint">{$t("custom_css_hint")}</p>
        </section>

        <section class="setting-group">
          <label class="group-label">{$t("sys_info_title")}</label>
          <p class="hint">
            Linux Arch Linux GTK WebKit · Tauri 2.0 Native Shell Integration · Zero-Allocation Engine
          </p>
        </section>
      {/if}

      <!-- ================= TAB: HAKKINDA / ABOUT ================= -->
      {#if activeTab === "about"}
        <!-- Developer & Contact -->
        <section class="setting-group">
          <label class="group-label">{$t("dev_community_title")}</label>
          <div style="background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-md); padding:14px; display:flex; flex-direction:column; gap:10px;">
            <div style="display:flex; align-items:center; justify-content:space-between;">
              <div>
                <div style="font-weight:700; font-size:14px; color:var(--text-primary)">LordBlue / ALX</div>
                <div style="font-size:11px; color:var(--accent); font-family:var(--font-mono)">@basihamam</div>
              </div>
              <button class="btn btn-accent" on:click={() => openUrl("https://github.com/basihamam")}>
                {$t("github_profile")}
              </button>
            </div>
            <p class="hint">{$t("dev_community_hint")}</p>
            <div style="display:flex; gap:8px; flex-wrap:wrap">
              <button class="btn" style="flex:1" on:click={() => openUrl("https://github.com/basihamam/arch-explorer-tauri")}>
                {$t("github_repo")}
              </button>
              <button class="btn" style="flex:1" on:click={reportBug}>
                {$t("submit_feedback")}
              </button>
            </div>
          </div>
        </section>

        <!-- Crash & Error Reporting -->
        <section class="setting-group">
          <label class="group-label">{$t("crash_telemetry_title")}</label>
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
            <span style="font-weight:600; font-size:13px">{$t("auto_crash_reports")}</span>
            <div class="toggle-row">
              <button class="toggle-btn" class:active={$autoCrashReport} on:click={() => autoCrashReport.set(true)}>{$t("on")}</button>
              <button class="toggle-btn" class:active={!$autoCrashReport} on:click={() => autoCrashReport.set(false)}>{$t("off")}</button>
            </div>
          </div>
          <p class="hint">{$t("auto_crash_hint")}</p>
          <div style="margin-top:6px">
            <button class="btn" on:click={sendTestCrashReport}>
              {$t("send_test_crash_report")}
            </button>
          </div>
        </section>

        <section class="setting-group">
          <label class="group-label">{$t("version_title")}</label>
          <p class="hint mono">
            Arch Explorer v0.1.0 (x86_64 Linux) · Open Source Community Edition
          </p>
        </section>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    display: flex; align-items: center; justify-content: center;
    z-index: 100;
  }
  .panel {
    width: 480px;
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
  
  .settings-tabs {
    display: flex;
    overflow-x: auto;
    background: rgba(0, 0, 0, 0.12);
    border-bottom: 1px solid var(--border);
    padding: 4px 8px 0;
    gap: 4px;
    scrollbar-width: none;
  }
  .settings-tabs::-webkit-scrollbar {
    display: none;
  }
  .tab-btn {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-dim);
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }
  .tab-btn:hover {
    color: var(--text-primary);
  }
  .tab-btn.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 600;
  }

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
