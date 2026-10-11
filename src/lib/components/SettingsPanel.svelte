<script>
  import { createEventDispatcher } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    theme, opacity, panelOpacity, blurAmount, accentColor, tabBarMode, autoReload, notify,
    previewMode, previewCustomTypes, thumbnailMode, thumbnailMinSize, gridZoom,
    adminMode, alwaysLaunchSudo, sessionSudoPassword, requestSudo,
    bookmarkIconsEnabled, videoHoverScrubbing, conflictFileAction, conflictDirAction,
    autoCrashReport
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
        notify("success", "Yönetici (Admin) Modu aktif edildi");
      } else {
        try {
          await requestSudo(
            "Yönetici (Admin) Modunu Etkinleştir",
            "Uygulamanın şifre sormadan doğrudan root yetkisiyle çalışması için sudo şifrenizi girin.",
            async (pwd) => {
              await invoke("verify_sudo_password", { password: pwd });
              sessionSudoPassword.set(pwd);
              adminMode.set(true);
              notify("success", "Yönetici (Admin) Modu aktif edildi");
            }
          );
        } catch (e) {
          // İptal edildi veya hata
        }
      }
    } else {
      adminMode.set(false);
      notify("info", "Yönetici (Admin) Modu kapatıldı");
    }
  }

  async function toggleAlwaysLaunchSudo(enable) {
    if (enable) {
      alwaysLaunchSudo.set(true);
      notify("success", "Her açılışta sudo ile başlatma aktif edildi");
      if (!$sessionSudoPassword) {
        toggleAdminMode(true);
      }
    } else {
      alwaysLaunchSudo.set(false);
      notify("info", "Her açılışta sudo ile başlatma kapatıldı");
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
      const res = await invoke("send_crash_report", {
        title: "User Test Diagnostics Report",
        body: `Arch Explorer Diagnostic Ping\nTime: ${new Date().toISOString()}\nPlatform: Arch Linux GTK WebKit`
      });
      notify("success", `Report generated: ${res}`);
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
      <button class="tab-btn" class:active={activeTab === "appearance"} on:click={() => activeTab = "appearance"}>{$t("settings") === "Settings" ? "Appearance" : "Görünüm"}</button>
      <button class="tab-btn" class:active={activeTab === "layout"} on:click={() => activeTab = "layout"}>{$t("settings") === "Settings" ? "Layout" : "Yerleşim"}</button>
      <button class="tab-btn" class:active={activeTab === "behavior"} on:click={() => activeTab = "behavior"}>{$t("settings") === "Settings" ? "Behavior" : "Davranış"}</button>
      <button class="tab-btn" class:active={activeTab === "admin"} on:click={() => activeTab = "admin"}>{$t("settings") === "Settings" ? "Admin" : "Yönetici"}</button>
      <button class="tab-btn" class:active={activeTab === "advanced"} on:click={() => activeTab = "advanced"}>{$t("settings") === "Settings" ? "Advanced" : "Gelişmiş"}</button>
      <button class="tab-btn" class:active={activeTab === "about"} on:click={() => activeTab = "about"}>{$t("settings") === "Settings" ? "About & Feedback" : "Hakkında & İletişim"}</button>
    </div>

    <div class="settings-body">

      <!-- ================= TAB: GÖRÜNÜM ================= -->
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
          <label class="group-label">Tema</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$theme === "dark"}  on:click={() => theme.set("dark")}>Dark</button>
            <button class="toggle-btn" class:active={$theme === "light"} on:click={() => theme.set("light")}>Light</button>
          </div>
        </section>

        <!-- Accent Color -->
        <section class="setting-group">
          <label class="group-label">Vurgu Rengi</label>
          <div class="color-row">
            <input type="color" bind:value={$accentColor} class="color-picker" />
            <span class="color-val">{$accentColor}</span>
            <div class="color-preview" style="background: {$accentColor}"></div>
          </div>
        </section>

        <!-- Opacity / Transparency -->
        <section class="setting-group">
          <label class="group-label">Arka Plan Opaklığı: {Math.round($opacity * 100)}%</label>
          <input
            type="range"
            min="0.3"
            max="0.99"
            step="0.01"
            bind:value={$opacity}
            class="slider"
          />
          <div class="opacity-labels">
            <span>Ghost (Şeffaf)</span>
            <span>Tam Opak</span>
          </div>
        </section>

        <!-- Blur -->
        <section class="setting-group">
          <label class="group-label">Arayüz Bulanıklığı (Blur): {$blurAmount}px</label>
          <input type="range" min="0" max="40" step="2" bind:value={$blurAmount} class="slider" />
          <div class="opacity-labels"><span>0 (Yok)</span><span>40px (Maksimum)</span></div>
          <p class="hint">Sağ tık menüleri, komut paneli, ayarlar ve açılır pencerelerin arkasındaki cisimleri blurlar.</p>
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
          <label class="group-label">Hazır Temalar (Presets)</label>
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

      <!-- ================= TAB: YERLEŞİM ================= -->
      {#if activeTab === "layout"}
        <!-- Yer İşaretleri İkonları -->
        <section class="setting-group">
          <label class="group-label">Yer İşaretleri (Bookmarks) İkonları</label>
          <div class="toggle-row">
            <button
              class="toggle-btn"
              class:active={!$bookmarkIconsEnabled}
              on:click={() => bookmarkIconsEnabled.set(false)}
              title="Klasik sade görünüm (◈ Sabitlenmiş / ⊹ Normal)"
            >Klasik (◈ / ⊹)</button>
            <button
              class="toggle-btn"
              class:active={$bookmarkIconsEnabled}
              on:click={() => bookmarkIconsEnabled.set(true)}
              title="Özel ikonlar (🏠 Home, 🗑️ Çöp Kutusu, 🖥️ Masaüstü vb.)"
            >Özel Emojiler</button>
          </div>
          <p class="hint">
            Sol paneldeki yer işaretlerinin başında klasik simgeler (◈ / ⊹) veya renkli emojiler gösterilsin.
          </p>
        </section>

        <!-- Tab Bar Mode -->
        <section class="setting-group">
          <label class="group-label">Sekme Çubuğu (Tab Bar)</label>
          <div class="toggle-row">
            <button
              class="toggle-btn"
              class:active={$tabBarMode === "always"}
              on:click={() => tabBarMode.set("always")}
              title="Her zaman görünür — tek tab bile olsa bar gösterilir"
            >Her Zaman Göster</button>
            <button
              class="toggle-btn"
              class:active={$tabBarMode === "lazy"}
              on:click={() => tabBarMode.set("lazy")}
              title="2+ tab açıkken belirir, tek tabta gizlenir"
            >Lazy (2+ Sekme)</button>
          </div>
          <p class="hint">
            <strong>Her Zaman:</strong> Sekme çubuğu her zaman görünür.<br>
            <strong>Lazy:</strong> Sadece 2 veya daha fazla sekme açıkken gösterilir.
          </p>
        </section>

        <!-- Preview Mode -->
        <section class="setting-group">
          <label class="group-label">Önizleme Paneli (Preview Panel)</label>
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
              <span class="mode-desc">Her zaman açık, boşken önceki/disk bilgisi</span>
            </button>
            <button class="mode-btn" class:active={$previewMode === "auto"}
              on:click={() => previewMode.set("auto")}>
              <span class="mode-icon">◐</span>
              <span class="mode-name">Otomatik</span>
              <span class="mode-desc">Seçim varsa açık, yoksa gizlenir</span>
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

        <!-- Grid & Thumbnails -->
        <section class="setting-group">
          <label class="group-label">Izgara ve Thumbnail Boyutu</label>
          <div class="preview-mode-grid">
            <button class="mode-btn" class:active={$thumbnailMode === "always"}
              on:click={() => thumbnailMode.set("always")}>
              <span class="mode-icon">🖼</span>
              <span class="mode-name">Her Zaman</span>
              <span class="mode-desc">Tüm resimler her zaman thumbnail gösterir</span>
            </button>
            <button class="mode-btn" class:active={$thumbnailMode === "auto"}
              on:click={() => thumbnailMode.set("auto")}>
              <span class="mode-icon">◐</span>
              <span class="mode-name">Otomatik</span>
              <span class="mode-desc">Grid {$thumbnailMinSize}px+ ise thumbnail, küçükse ikon</span>
            </button>
            <button class="mode-btn" class:active={$thumbnailMode === "custom"}
              on:click={() => thumbnailMode.set("custom")}
              style="grid-column: 1/-1">
              <span class="mode-icon">⚙</span>
              <span class="mode-name">Özel</span>
              <span class="mode-desc">Aşağıdan minimum boyutu kendin belirle</span>
            </button>
          </div>

          {#if $thumbnailMode === "auto" || $thumbnailMode === "custom"}
            <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
              <label class="group-label">
                Minimum Thumbnail Boyutu: {$thumbnailMinSize}px
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
            </div>
          {/if}

          <div style="margin-top:8px; display:flex; flex-direction:column; gap:6px">
            <label class="group-label">Grid Yakınlaştırma (Zoom): {$gridZoom}px
              <span style="color:var(--text-dim); font-weight:400; margin-left:4px">(Ctrl + Tekerlek)</span>
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

      <!-- ================= TAB: DAVRANIŞ ================= -->
      {#if activeTab === "behavior"}
        <!-- Auto Reload -->
        <section class="setting-group">
          <label class="group-label">Otomatik Yenileme (Auto Reload)</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$autoReload}  on:click={() => autoReload.set(true)}>Açık</button>
            <button class="toggle-btn" class:active={!$autoReload} on:click={() => autoReload.set(false)}>Kapalı</button>
          </div>
          <p class="hint">
            Klasör içindeki değişiklikleri inotify ile izler ve otomatik yeniler. Kapatıldığında F5 ile manuel yenilenir.
          </p>
        </section>

        <!-- Du Cache -->
        <section class="setting-group">
          <label class="group-label">Dizin Boyut Önbelleği (DU Cache)</label>
          <p class="hint">
            Klasör boyutları arka planda hesaplandıktan sonra önbelleğe kaydedilir. Önbelleği temizlediğinizde tüm boyutlar sıfırdan tekrar taranır.
          </p>
          <button class="btn" style="width:fit-content;margin-top:4px"
            on:click={async () => {
              const n = await invoke("clear_du_cache").catch(() => 0);
              window.dispatchEvent(new CustomEvent("clear-dir-size-cache"));
              notify("success", `${n} boyut önbellek kaydı silindi ve sıfırlandı`);
            }}
          >
            Boyut Önbelleğini Temizle
          </button>
        </section>

        <!-- Video Hover Scrubbing -->
        <section class="setting-group">
          <label class="group-label">Video Kare Kaydırma (Hover Scrubbing)</label>
          <div class="toggle-row">
            <button class="toggle-btn" class:active={$videoHoverScrubbing} on:click={() => videoHoverScrubbing.set(true)}>Açık</button>
            <button class="toggle-btn" class:active={!$videoHoverScrubbing} on:click={() => videoHoverScrubbing.set(false)}>Kapalı</button>
          </div>
          <p class="hint">
            Fareyi video küçük resminin (thumbnail) üzerinde sağa-sola kaydırdığınızda YouTube gibi videonun farklı anlarından kareleri anlık gösterir.
          </p>
        </section>

        <!-- Conflict Resolution: Files & Folders -->
        <section class="setting-group">
          <label class="group-label">Dosya ve Klasör Çakışma Yönetimi</label>
          
          <div style="display:flex; flex-direction:column; gap:12px; margin-top:6px">
            <div>
              <span style="font-size:12px; font-weight:600; color:var(--text-secondary)">Dosya Çakışmasında:</span>
              <div class="toggle-row" style="margin-top:4px">
                <button class="toggle-btn" class:active={$conflictFileAction === "ask"} on:click={() => conflictFileAction.set("ask")}>Her Zaman Sor</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "replace"} on:click={() => conflictFileAction.set("replace")}>Üzerine Yaz</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "rename"} on:click={() => conflictFileAction.set("rename")}>İkisini de Koru</button>
                <button class="toggle-btn" class:active={$conflictFileAction === "skip"} on:click={() => conflictFileAction.set("skip")}>Atla</button>
              </div>
            </div>

            <div>
              <span style="font-size:12px; font-weight:600; color:var(--text-secondary)">Klasör Çakışmasında:</span>
              <div class="toggle-row" style="margin-top:4px">
                <button class="toggle-btn" class:active={$conflictDirAction === "ask"} on:click={() => conflictDirAction.set("ask")}>Her Zaman Sor</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "replace"} on:click={() => conflictDirAction.set("replace")}>Üzerine Yaz / Birleştir</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "rename"} on:click={() => conflictDirAction.set("rename")}>İkisini de Koru</button>
                <button class="toggle-btn" class:active={$conflictDirAction === "skip"} on:click={() => conflictDirAction.set("skip")}>Atla</button>
              </div>
            </div>
          </div>
          <p class="hint" style="margin-top:8px">
            Üzerine yazılan tüm dosyalar güvenli yedek alanına alınır; Ctrl+Z yapıldığında eski dosyalarınız eksiksiz geri yüklenir!
          </p>
        </section>

        <!-- Thumbnail Cache & RAM Cleanup -->
        <section class="setting-group">
          <label class="group-label">Bellek & Önbellek Temizliği (RAM / Cache)</label>
          <p class="hint">
            Küçük resim önbelleğini ve bellekteki tüm görsel/dizin nesnelerini boşaltarak RAM kullanımını anında sıfırlar.
          </p>
          <div style="display:flex; gap:8px; margin-top:6px; flex-wrap:wrap">
            <button class="btn" style="flex:1"
              on:click={async () => {
                window.dispatchEvent(new CustomEvent('arch:free-ram'));
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", `⚡ RAM & Görsel Önbelleği temizlendi! (${n} önbellek silindi)`);
              }}
            >
              🧹 Belleği & RAM'i Temizle
            </button>
            <button class="btn" style="flex:1"
              on:click={async () => {
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", `${n} thumbnail önbellek kaydı silindi`);
              }}
            >
              Yalnızca Thumbnail'leri Sil
            </button>
          </div>
        </section>
      {/if}

      <!-- ================= TAB: YÖNETİCİ ================= -->
      {#if activeTab === "admin"}
        <!-- Admin Mode & Sudo Settings -->
        <section class="setting-group">
          <label class="group-label">Yönetici Yetkileri (Root / Admin)</label>
          
          <div style="display:flex; flex-direction:column; gap:16px;">
            <div>
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
                <span style="font-weight:600; font-size:13px">Admin Modu</span>
                <div class="toggle-row">
                  <button class="toggle-btn" class:active={$adminMode} on:click={() => toggleAdminMode(true)}>Açık</button>
                  <button class="toggle-btn" class:active={!$adminMode} on:click={() => toggleAdminMode(false)}>Kapalı</button>
                </div>
              </div>
              <p class="hint">
                Aktif olduğunda hiçbir klasöre girerken veya dosya taşırken şifre sormadan doğrudan root yetkisiyle çalışır.
              </p>
            </div>

            <div>
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
                <span style="font-weight:600; font-size:13px">Her Zaman Sudo ile Başlat</span>
                <div class="toggle-row">
                  <button class="toggle-btn" class:active={$alwaysLaunchSudo} on:click={() => toggleAlwaysLaunchSudo(true)}>Açık</button>
                  <button class="toggle-btn" class:active={!$alwaysLaunchSudo} on:click={() => toggleAlwaysLaunchSudo(false)}>Kapalı</button>
                </div>
              </div>
              <p class="hint">
                Uygulama her açılışında bir kere şifre sorar ve Admin Modu'nu baştan itibaren otomatik olarak aktif eder.
              </p>
            </div>
          </div>
        </section>
      {/if}

      <!-- ================= TAB: GELİŞMİŞ ================= -->
      {#if activeTab === "advanced"}
        <!-- RAM & Memory Optimization -->
        <section class="setting-group">
          <label class="group-label">Bellek ve RAM Optimizasyonu (Performance)</label>
          <p class="hint">
            On binlerce dosya içeren devasa klasörler açıldığında bellek otomatik olarak sanal pencerelenir (Virtual Chunking). RAM kullanımını anında sıfırlamak için önbelleği ve geçici nesneleri temizleyebilirsiniz.
          </p>
          <div style="display:flex; gap:8px; margin-top:8px">
            <button class="btn" style="flex:1"
              on:click={async () => {
                window.dispatchEvent(new CustomEvent('arch:free-ram'));
                const n = await invoke("clear_thumbnail_cache").catch(() => 0);
                notify("success", `⚡ RAM ve Görsel Önbelleği temizlendi! (${n} önbellek boşaltıldı)`);
              }}
            >
              🧹 Belleği & RAM'i Şimdi Temizle
            </button>
          </div>
        </section>

        <!-- Custom CSS -->
        <section class="setting-group">
          <label class="group-label">Özel CSS Değişkenleri (Custom CSS)</label>
          <p class="hint">
            Uygulamanın fontları, renkleri veya boşlukları için <code>src/app.css</code> dosyasındaki <code>--variable</code> tanımlarını düzenleyebilirsiniz.
          </p>
        </section>

        <section class="setting-group">
          <label class="group-label">System Information</label>
          <p class="hint">
            Linux Arch Linux GTK WebKit · Tauri 2.0 Native Shell Integration · Zero-Allocation Engine
          </p>
        </section>
      {/if}

      <!-- ================= TAB: HAKKINDA / ABOUT ================= -->
      {#if activeTab === "about"}
        <!-- Developer & Contact -->
        <section class="setting-group">
          <label class="group-label">Developer & Community</label>
          <div style="background:var(--bg-tertiary); border:1px solid var(--border); border-radius:var(--radius-md); padding:14px; display:flex; flex-direction:column; gap:10px;">
            <div style="display:flex; align-items:center; justify-content:space-between;">
              <div>
                <div style="font-weight:700; font-size:14px; color:var(--text-primary)">LordBlue / ALX</div>
                <div style="font-size:11px; color:var(--accent); font-family:var(--font-mono)">@basihamam</div>
              </div>
              <button class="btn btn-accent" on:click={() => openUrl("https://github.com/basihamam")}>
                ★ GitHub Profile
              </button>
            </div>
            <p class="hint">
              Arch Explorer is built with passion for the Arch Linux ecosystem. For questions, feature requests, or direct contact, check out the official GitHub page:
            </p>
            <div style="display:flex; gap:8px; flex-wrap:wrap">
              <button class="btn" style="flex:1" on:click={() => openUrl("https://github.com/basihamam/arch-explorer-tauri")}>
                📂 GitHub Repository
              </button>
              <button class="btn" style="flex:1" on:click={reportBug}>
                💬 Submit Feedback / Issues
              </button>
            </div>
          </div>
        </section>

        <!-- Crash & Error Reporting -->
        <section class="setting-group">
          <label class="group-label">Crash & Error Telemetry</label>
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px">
            <span style="font-weight:600; font-size:13px">Automatic Crash Reports</span>
            <div class="toggle-row">
              <button class="toggle-btn" class:active={$autoCrashReport} on:click={() => autoCrashReport.set(true)}>Enabled</button>
              <button class="toggle-btn" class:active={!$autoCrashReport} on:click={() => autoCrashReport.set(false)}>Disabled</button>
            </div>
          </div>
          <p class="hint">
            When an unexpected error or UI crash happens, Arch Explorer automatically generates a clean crash diagnostic report and opens an issue template so the developer (@basihamam) can investigate and fix it instantly.
          </p>
          <div style="margin-top:6px">
            <button class="btn" on:click={sendTestCrashReport}>
              ⚡ Send Test Diagnostic Report
            </button>
          </div>
        </section>

        <section class="setting-group">
          <label class="group-label">Version</label>
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
