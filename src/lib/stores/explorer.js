import { writable, derived, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";

// ---- Settings: tab bar mode + auto reload ----
export const tabBarMode = writable(localStorage.getItem("tabBarMode") || "always"); // "always" | "lazy"
export const autoReload = writable(localStorage.getItem("autoReload") !== "false");
tabBarMode.subscribe(v => localStorage.setItem("tabBarMode", v));
autoReload.subscribe(v => localStorage.setItem("autoReload", String(v)));

// ---- TABS ----
// Her tab: { id, path, label, history, historyIndex }
let _tabId = 0;
function newTab(path) {
  return {
    id: ++_tabId,
    path: path || "/home",
    label: (path || "/home").split("/").filter(Boolean).pop() || "/",
    history: [path || "/home"],
    historyIndex: 0,
    scrollTop: 0,
    cachedEntries: null,
    cachedSelectedPaths: null,
  };
}

// Sekme persistance — uygulama kapanınca localStorage'a kaydet, açılınca geri yükle
function _loadPersistedTabs() {
  try {
    const raw = localStorage.getItem("persistedTabs");
    if (!raw) return null;
    const saved = JSON.parse(raw);
    if (!Array.isArray(saved.tabs) || saved.tabs.length === 0) return null;
    // id'leri yeniden üret, çakışma olmasın
    const restored = saved.tabs.map(t => {
      const tab = newTab(t.path);
      tab.history      = Array.isArray(t.history) ? t.history : [t.path];
      tab.historyIndex = typeof t.historyIndex === "number" ? t.historyIndex : 0;
      return tab;
    });
    return { tabs: restored, activeIndex: saved.activeIndex || 0 };
  } catch { return null; }
}

function _persistTabs(tabList, activeId) {
  const activeIndex = tabList.findIndex(t => t.id === activeId);
  localStorage.setItem("persistedTabs", JSON.stringify({
    tabs: tabList.map(t => ({ path: t.path, history: t.history, historyIndex: t.historyIndex })),
    activeIndex: Math.max(0, activeIndex),
  }));
}

const _persisted = _loadPersistedTabs();
const _initialTabs = _persisted
  ? _persisted.tabs
  : [newTab(localStorage.getItem("lastPath") || "/home")];

export const tabs        = writable(_initialTabs);
export const activeTabId = writable(_initialTabs[_persisted?.activeIndex || 0]?.id || 1);

// Değişince otomatik persist
tabs.subscribe(ts => {
  const id = get(activeTabId);
  _persistTabs(ts, id);
});
activeTabId.subscribe(id => {
  const ts = get(tabs);
  _persistTabs(ts, id);
});

// currentPath — derived from active tab, backward compat için
export const currentPath = derived(
  [tabs, activeTabId],
  ([$tabs, $activeTabId]) => {
    const tab = $tabs.find(t => t.id === $activeTabId);
    return tab ? tab.path : "/home";
  }
);

// pathHistory / historyIndex — active tab'dan türet (Toolbar uyumu)
export const pathHistory = derived(
  [tabs, activeTabId],
  ([$tabs, $activeTabId]) => {
    const tab = $tabs.find(t => t.id === $activeTabId);
    return tab ? tab.history : [];
  }
);
export const historyIndex = derived(
  [tabs, activeTabId],
  ([$tabs, $activeTabId]) => {
    const tab = $tabs.find(t => t.id === $activeTabId);
    return tab ? tab.historyIndex : 0;
  }
);

// ---- Tab actions ----
export function addTab(path) {
  const t = newTab(path || get(currentPath));
  tabs.update(ts => [...ts, t]);
  activeTabId.set(t.id);
  loadDirectory(t.path);
}

export function closeTab(id) {
  const ts = get(tabs);
  if (ts.length === 1) return; // son tab kapatılamaz
  const idx = ts.findIndex(t => t.id === id);
  tabs.update(ts => ts.filter(t => t.id !== id));
  // kapatılan aktifse komşuya geç
  if (get(activeTabId) === id) {
    const next = get(tabs);
    const newActive = next[Math.max(0, idx - 1)];
    activeTabId.set(newActive.id);
    loadDirectory(newActive.path);
  }
}

export const tabScrollPositions = {};

export function saveTabScroll(tabId, scrollTop) {
  if (tabId) tabScrollPositions[tabId] = scrollTop;
}

export function restoreTabScroll(tabId, fallback = 0) {
  const target = tabScrollPositions[tabId] ?? fallback;
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("arch:restore-tab-scroll", {
      detail: { tabId, scrollTop: target }
    }));
  }
}

export async function refreshDirectorySilently(path, expectedTabId) {
  try {
    const listing = await fetchDirectoryListing(path, get(showHidden));
    if (get(activeTabId) === expectedTabId && get(currentPath) === path) {
      entries.set(listing.entries);
    }
  } catch {}
}

export function switchTab(id) {
  const oldId = get(activeTabId);
  if (oldId === id) return;

  // 1. Eski aktif tab'ın verilerini ve scroll pozisyonunu sakla
  const currentEntries = get(entries);
  const currentSel = get(selectedPaths);
  tabs.update(ts => ts.map(t => {
    if (t.id === oldId) {
      return {
        ...t,
        scrollTop: tabScrollPositions[oldId] || 0,
        cachedEntries: currentEntries,
        cachedSelectedPaths: currentSel,
      };
    }
    return t;
  }));

  // 2. Yeni tab'ı aktif yap
  activeTabId.set(id);
  const tab = get(tabs).find(t => t.id === id);
  if (tab) {
    if (tab.cachedEntries && tab.cachedEntries.length > 0) {
      // DOM'u ve scroll yüksekliğini sıfırlamadan ANINDA göster!
      entries.set(tab.cachedEntries);
      selectedPaths.set(tab.cachedSelectedPaths || new Set());
      restoreTabScroll(id, tab.scrollTop || 0);
      refreshDirectorySilently(tab.path, id);
    } else {
      loadDirectory(tab.path).then(() => {
        restoreTabScroll(id, tab.scrollTop || 0);
      });
    }
  }
}

// active tab'ı güncelle (internal)
function _updateActiveTab(updater) {
  const id = get(activeTabId);
  tabs.update(ts => ts.map(t => t.id === id ? updater(t) : t));
}

// ---- Directory contents ----
export const entries   = writable([]);
export const isLoading = writable(false);
export const loadError = writable(null);

// ---- View settings ----
export const viewMode   = writable(localStorage.getItem("viewMode") || "grid");
export const showHidden = writable(localStorage.getItem("showHidden") === "true");
export const sortBy     = writable("name");
export const sortAsc    = writable(true);

// Klasöre göre görünüm tercihi (Per-folder view mode memory)
// örn: { "/home/claude/Downloads": "list", "/home/claude/Pictures": "grid" }
export const dirViewModes = writable(JSON.parse(localStorage.getItem("dirViewModes") || "{}"));
dirViewModes.subscribe(v => localStorage.setItem("dirViewModes", JSON.stringify(v)));

export function setFolderViewMode(path, mode) {
  if (!path) return;
  const cleanPath = path.replace(/\/+$/, "") || "/";
  dirViewModes.update(m => ({ ...m, [cleanPath]: mode }));
  viewMode.set(mode);
}

export function applyViewModeForDirectory(path, entryCount) {
  const cleanPath = (path || "").replace(/\/+$/, "") || "/";
  const saved = get(dirViewModes);
  if (saved && saved[cleanPath]) {
    // Kullanıcı bu klasör için önceden bir ayar seçmiş -> onu hatırla
    viewMode.set(saved[cleanPath]);
  } else {
    // Akıllı varsayılan: Az dosya (<= 25) ise Grid, çok dosya (> 25) ise List view!
    const autoMode = (entryCount <= 25) ? "grid" : "list";
    viewMode.set(autoMode);
  }
}

// Klasöre göre sıralama tercihi (Per-folder sort settings memory)
// örn: { "/home/claude/Downloads": { sortBy: "modified", sortAsc: false } }
export const dirSortSettings = writable(JSON.parse(localStorage.getItem("dirSortSettings") || "{}"));
dirSortSettings.subscribe(v => localStorage.setItem("dirSortSettings", JSON.stringify(v)));

export function setFolderSort(path, field, asc) {
  if (!path) return;
  const cleanPath = path.replace(/\/+$/, "") || "/";
  dirSortSettings.update(m => ({ ...m, [cleanPath]: { sortBy: field, sortAsc: asc } }));
  sortBy.set(field);
  sortAsc.set(asc);
}

export function toggleSort(field) {
  const currentField = get(sortBy);
  const currentAsc   = get(sortAsc);
  const isRight      = get(activePaneId) === "right";
  const path         = isRight ? get(rightPanePath) : get(currentPath);

  let newAsc = true;
  if (currentField === field) {
    newAsc = !currentAsc;
  } else {
    newAsc = (field === "name") ? true : false;
  }
  setFolderSort(path, field, newAsc);
}

export function applySortForDirectory(path) {
  const cleanPath = (path || "").replace(/\/+$/, "") || "/";
  const saved = get(dirSortSettings);
  if (saved && saved[cleanPath]) {
    sortBy.set(saved[cleanPath].sortBy || "name");
    sortAsc.set(saved[cleanPath].sortAsc !== false);
  } else {
    // Varsayılan: İsim (A-Z)
    sortBy.set("name");
    sortAsc.set(true);
  }
}

// ---- Selection ----
export const selectedPaths = writable(new Set());

// ---- Clipboard ----
export const clipboard = writable({ paths: [], mode: null });

// ---- Search ----
export const searchQuery   = writable("");
export const searchResults = writable([]);
export const isSearching   = writable(false);

// ---- Drives & devices ----
export const drives        = writable([]);
export const blockDevices  = writable([]);
// Sidebar'da gizlenen diskler (örn: loop cihazları veya kullanıcının gizlediği diskler)
export const hiddenDevices = writable(JSON.parse(localStorage.getItem("hiddenDevices") || "[]"));
hiddenDevices.subscribe(v => localStorage.setItem("hiddenDevices", JSON.stringify(v)));

// Disklerin sıralama düzeni ve sabitlenmiş (pinned) olanlar
export const deviceOrder   = writable(JSON.parse(localStorage.getItem("deviceOrder") || "[]"));
deviceOrder.subscribe(v => localStorage.setItem("deviceOrder", JSON.stringify(v)));

export const pinnedDevices = writable(JSON.parse(localStorage.getItem("pinnedDevices") || "[]"));
pinnedDevices.subscribe(v => localStorage.setItem("pinnedDevices", JSON.stringify(v)));

// ---- Bookmarks ----
export const bookmarks = writable([]);
// Bookmark ikon modu: false = Klasik (◈ / ⊹), true = Özel Renkli Emojiler (🏠, 🗑️ vb.)
export const bookmarkIconsEnabled = writable(localStorage.getItem("bookmarkIconsEnabled") === "true");
bookmarkIconsEnabled.subscribe(v => localStorage.setItem("bookmarkIconsEnabled", String(v)));

// ---- Commands ----
export const userCommands = writable([]);

// ---- Context menu ----
export const contextMenu = writable(null);

// ---- Preview mode ----
// 'manual'  → kapalı, sadece sağ tık Preview ile açılır
// 'always'  → her zaman açık, boşken disk bilgisi gösterir
// 'auto'    → seçim varsa açık, yoksa panel tamamen boş (disk göstermez)
// 'custom'  → kullanıcının belirlediği uzantılar için açık
export const previewMode        = writable(localStorage.getItem("previewMode") || "always");
export const previewCustomTypes = writable(JSON.parse(localStorage.getItem("previewCustomTypes") || '["png","jpg","mp4","pdf","rs","js","ts","svelte","md"]'));
previewMode.subscribe(v => localStorage.setItem("previewMode", v));
previewCustomTypes.subscribe(v => localStorage.setItem("previewCustomTypes", JSON.stringify(v)));

// showPreview artık previewMode'dan türetilir — backward compat için writable kalır
// ama previewMode=manual ise false'a zorlanır
export const showPreview = writable(
  localStorage.getItem("previewMode") !== "manual"
    && localStorage.getItem("showPreview") !== "false"
);
showPreview.subscribe(v => localStorage.setItem("showPreview", String(v)));

// ---- Dual pane ----
export const showDualPane  = writable(false); // Ctrl+P ile açılır, başlangıçta kapalı
export const activePaneId  = writable("left");
export const showDevices    = writable(false);
export const selectedDevice = writable(null); // BlockDevice | null

// ---- Right pane stores (dual pane Firefox stili) ----
export const rightPanePath      = writable(localStorage.getItem("rightPanePath") || "/home");
export const rightPaneTabs      = writable([]);
export const rightActiveTabId   = writable(null);
export const rightPaneCanBack   = writable(false);
export const rightPaneCanForward = writable(false);
// Toolbar → PaneColumn iletişim: { type, path?, tabId? }
export const rightPaneNavRequest = writable(null);

rightPanePath.subscribe(v => localStorage.setItem("rightPanePath", v));

export const rightViewMode      = writable("grid");
export const rightShowHidden    = writable(false);
export const rightDisplayCount  = writable(0);
export const rightTotalSize     = writable(0);
export const rightSelectedCount = writable(0);
export const rightSearchQuery   = writable("");

// ── Aktif pane'in değerlerini türet — Toolbar/StatusBar bunları okur ──
export const activeViewMode = derived(
  [viewMode, rightViewMode, activePaneId],
  ([$v, $rv, $id]) => $id === "right" ? $rv : $v
);
export const activeShowHidden = derived(
  [showHidden, rightShowHidden, activePaneId],
  ([$v, $rv, $id]) => $id === "right" ? $rv : $v
);
// Toolbar her zaman aktif pane'in yolunu gösterir
// (activePaneId ve rightPanePath yukarıda tanımlandıktan sonra derived)
export const activeToolbarPath = derived(
  [currentPath, rightPanePath, activePaneId],
  ([$cp, $rp, $id]) => $id === "right" ? $rp : $cp
);

// ---- Preview entry ----
export const previewEntry = writable(null);

selectedPaths.subscribe(paths => {
  if (paths.size === 1) {
    const path = [...paths][0];
    const entry = get(entries).find(e => e.path === path);
    if (entry) previewEntry.set(entry);
  } else if (paths.size === 0) {
    previewEntry.set(null);
  }
});

// ---- Grid zoom (sol pane & sağ pane bağımsız) + Thumbnail modu ----
export const leftGridZoom     = writable(parseInt(localStorage.getItem("leftGridZoom") || localStorage.getItem("gridZoom") || "90"));
export const rightGridZoom    = writable(parseInt(localStorage.getItem("rightGridZoom") || "90"));
export const gridZoom         = leftGridZoom; // Geriye dönük uyumluluk

export const thumbnailMode    = writable(localStorage.getItem("thumbnailMode") || "auto");
// auto/custom için minimum px eşiği
export const thumbnailMinSize = writable(parseInt(localStorage.getItem("thumbnailMinSize") || "100"));

leftGridZoom.subscribe(v     => { localStorage.setItem("leftGridZoom", String(v)); localStorage.setItem("gridZoom", String(v)); });
rightGridZoom.subscribe(v    => localStorage.setItem("rightGridZoom", String(v)));
thumbnailMode.subscribe(v    => localStorage.setItem("thumbnailMode",    v));
thumbnailMinSize.subscribe(v => localStorage.setItem("thumbnailMinSize", String(v)));

// ---- Global Sudo Password Dialog Store ----
// { show: bool, title: string, description: string, action: async (password) => void }
export const sudoDialog = writable(null);

// ---- Open With Dialog Store ----
// { path: string, name: string } | null
export const openWithDialog = writable(null);

export async function openEntry(entry) {
  if (!entry) return;
  const isRight = get(activePaneId) === "right" && get(showDualPane);
  if (entry.is_dir) {
    if (isRight) {
      rightPaneNavRequest.set({ type: "navigate", path: entry.path });
    } else {
      navigateTo(entry.path);
    }
    return;
  }
  try {
    const opened = await invoke("open_file_default", { path: entry.path });
    if (!opened) {
      // Varsayılan uygulama yoksa direkt OpenWith panelini aç!
      openWithDialog.set({ path: entry.path, name: entry.name });
    }
  } catch (e) {
    console.warn("open_file_default failed, opening OpenWith dialog:", e);
    openWithDialog.set({ path: entry.path, name: entry.name });
  }
}

export function requestSudo(title, description, action) {
  // Eğer oturum şifresi hafızadaysa, kullanıcıyı bir daha yormadan direkt çalıştır
  const cachedPwd = get(sessionSudoPassword);
  if (cachedPwd) {
    return Promise.resolve().then(() => action(cachedPwd)).catch(async (err) => {
      // Eğer eski şifre geçersiz kaldıysa hafızayı temizle ve kullanıcıdan yeni şifre iste
      if (String(err).includes("WRONG_PASSWORD")) {
        sessionSudoPassword.set(null);
        return requestSudo(title, description, action);
      }
      throw err;
    });
  }

  return new Promise((resolve, reject) => {
    sudoDialog.set({
      title: title || "Yetki Gerekli (Sudo)",
      description: description || "Bu işlemi gerçekleştirmek için yönetici şifresi gereklidir.",
      onSubmit: async (password) => {
        try {
          const res = await action(password);
          sessionSudoPassword.set(password); // Şifre doğruysa oturum için kaydet
          sudoDialog.set(null);
          resolve(res);
        } catch (err) {
          throw err;
        }
      },
      onCancel: () => {
        sudoDialog.set(null);
        reject(new Error("İptal edildi"));
      }
    });
  });
}

// ── Root Access & Admin Mode Stores ──
// adminMode: Oturum boyunca tüm işlemleri doğrudan root/sudo yetkisiyle yapar, şifre sormaz
export const adminMode = writable(false);
// alwaysLaunchSudo: Uygulama her açıldığında otomatik olarak admin modunu başlatmak için 1 kere şifre ister
export const alwaysLaunchSudo = writable(localStorage.getItem("alwaysLaunchSudo") === "true");
alwaysLaunchSudo.subscribe(v => localStorage.setItem("alwaysLaunchSudo", String(v)));

// Hafızada tutulan oturum şifresi (asla localStorage'a yazılmaz!)
export const sessionSudoPassword = writable(null);
sessionSudoPassword.subscribe(pwd => {
  invoke("set_session_sudo_password", { password: pwd }).catch(() => {});
});

// Kilidi açılmış kök dizinler (ör: ["/root"])
// Bu dizinin altındaki tüm klasörler için bir daha şifre sorulmaz!
export const unlockedRoots = writable([]);

export function isPathUnlocked(targetPath) {
  if (get(adminMode) && get(sessionSudoPassword)) return true;
  const p = (targetPath || "").trim();
  const roots = get(unlockedRoots);
  for (const root of roots) {
    if (p === root || p.startsWith(root.endsWith("/") ? root : root + "/")) {
      return true;
    }
  }
  return false;
}

// ── Video Hover Scrubbing (Fare Gezdirme ile Önizleme) ──
export const videoHoverScrubbing = writable(localStorage.getItem("videoHoverScrubbing") === "true");
videoHoverScrubbing.subscribe(v => localStorage.setItem("videoHoverScrubbing", String(v)));

// ── Çakışma Yönetimi Ayarları (Conflict Resolution Preferences) ──
// Dosyalar için: "ask" | "replace" | "rename" | "skip"
export const conflictFileAction = writable(localStorage.getItem("conflictFileAction") || "ask");
conflictFileAction.subscribe(v => localStorage.setItem("conflictFileAction", v));

// Klasörler için: "ask" | "replace" | "rename" | "skip"
export const conflictDirAction = writable(localStorage.getItem("conflictDirAction") || "ask");
conflictDirAction.subscribe(v => localStorage.setItem("conflictDirAction", v));

// Global Çakışma İletişim Kutusu Store'u
export const conflictDialog = writable(null);


export function unlockPathWithPassword(targetPath, password) {
  sessionSudoPassword.set(password);
  unlockedRoots.update(roots => {
    if (!roots.includes(targetPath)) {
      return [...roots, targetPath];
    }
    return roots;
  });
}

export async function promptAndUnlockPath(targetPath) {
  const normPath = targetPath.replace(/\/+$/, "") || "/";
  return await requestSudo(
    "Erişim İzni Gerekli",
    `'${normPath}' klasörüne ve alt dizinlerine erişmek için yönetici (sudo) şifrenizi girin.`,
    async (password) => {
      // Önce bu şifreyle listelemeyi dene
      const listing = await invoke("sudo_list_directory", {
        path: targetPath,
        password,
        showHidden: get(showHidden)
      });
      unlockPathWithPassword(normPath, password);
      return listing;
    }
  );
}

export async function fetchDirectoryListing(path, showHiddenVal) {
  const sessPwd = get(sessionSudoPassword);
  const isAdmin = get(adminMode);

  // 1. Zaten admin modu aktifse veya bu path açılmış bir kökün altındaysa direkt sudo ile oku
  if (sessPwd && (isAdmin || isPathUnlocked(path))) {
    try {
      return await invoke("sudo_list_directory", { path, password: sessPwd, showHidden: showHiddenVal });
    } catch (e) {
      // Eğer sudo_list_directory hata verirse ve şifre bozulduysa normale düş
      const errStr = String(e).toLowerCase();
      if (!errStr.includes("wrong_password")) {
        throw e;
      }
    }
  }

  // 2. Normal okumayı dene
  try {
    return await invoke("list_directory", { path, showHidden: showHiddenVal });
  } catch (err) {
    const errStr = String(err).toLowerCase();
    const isPermissionDenied =
      errStr.includes("permission denied") ||
      errStr.includes("yetki") ||
      errStr.includes("access denied") ||
      errStr.includes("os error 13");

    if (isPermissionDenied) {
      // Eğer hafızada zaten şifre varsa (ör. admin mode yeni açılmış ama listing henüz gitmemişse)
      if (sessPwd) {
        try {
          const res = await invoke("sudo_list_directory", { path, password: sessPwd, showHidden: showHiddenVal });
          unlockPathWithPassword(path.replace(/\/+$/, "") || "/", sessPwd);
          return res;
        } catch (sudoErr) {
          if (!String(sudoErr).toLowerCase().includes("wrong_password")) {
            throw sudoErr;
          }
        }
      }

      // Kullanıcıdan şifre iste! Başarılı olursa bu klasör ve tüm alt klasörleri hafızaya aç
      const listing = await promptAndUnlockPath(path);
      return listing;
    }

    throw err;
  }
}

// ---- Git status ----
export const gitStatus    = writable({ is_repo: false, branch: "", files: {}, counts: {} });
export const renamingPath = writable(null); // inline rename için aktif path

async function loadGitStatus(path) {
  try {
    const status = await invoke("get_git_status", { path });
    gitStatus.set(status);
  } catch { gitStatus.set({ is_repo: false, branch: "", files: {}, counts: {} }); }
}

// ---- Notifications ----
export const notifications = writable([]);

// ---- Theme ----
export const theme        = writable(localStorage.getItem("theme")        || "dark");
export const opacity      = writable(parseFloat(localStorage.getItem("opacity")      || "0.82"));
export const panelOpacity = writable(parseFloat(localStorage.getItem("panelOpacity") || "0.92"));
export const blurAmount   = writable(parseInt(localStorage.getItem("blurAmount") || "20"));
export const accentColor  = writable(localStorage.getItem("accent")       || "#8264ff");

// ---- Persist settings ----
viewMode.subscribe(v   => localStorage.setItem("viewMode", v));
showHidden.subscribe(v => localStorage.setItem("showHidden", String(v)));
theme.subscribe(v => {
  localStorage.setItem("theme", v);
  document.documentElement.setAttribute("data-theme", v);
});
opacity.subscribe(v => {
  localStorage.setItem("opacity", String(v));
  const o = parseFloat(v);
  document.documentElement.style.setProperty("--bg-primary",   `rgba(15,15,20,${o})`);
  document.documentElement.style.setProperty("--bg-secondary", `rgba(22,22,30,${Math.min(o+0.05,1).toFixed(2)})`);
  document.documentElement.style.setProperty("--bg-tertiary",  `rgba(30,30,40,${Math.min(o+0.08,1).toFixed(2)})`);
});
blurAmount.subscribe(v => {
  localStorage.setItem("blurAmount", String(v));
  const b = parseInt(v, 10);
  const blurVal = (isNaN(b) || b <= 0) ? "none" : `blur(${b}px)`;
  document.documentElement.style.setProperty("--blur-ui",   blurVal);
  document.documentElement.style.setProperty("--blur-panel", blurVal);
  document.documentElement.style.setProperty("--blur-popup", blurVal);
  document.documentElement.style.setProperty("--blur-amount", `${isNaN(b) ? 0 : b}px`);
});

panelOpacity.subscribe(v => {
  localStorage.setItem("panelOpacity", String(v));
  const o = parseFloat(v);
  document.documentElement.style.setProperty("--bg-panel",   `rgba(15,15,20,${o})`);
  document.documentElement.style.setProperty("--bg-panel-2", `rgba(22,22,30,${Math.min(o+0.04,1).toFixed(2)})`);
  document.documentElement.style.setProperty("--bg-panel-3", `rgba(30,30,40,${Math.min(o+0.08,1).toFixed(2)})`);
});

accentColor.subscribe(v => {
  localStorage.setItem("accent", v);
  document.documentElement.style.setProperty("--accent", v);
  const hex = v.replace("#","");
  const r = parseInt(hex.slice(0,2),16);
  const g = parseInt(hex.slice(2,4),16);
  const b = parseInt(hex.slice(4,6),16);
  document.documentElement.style.setProperty("--accent-soft",   `rgba(${r},${g},${b},0.4)`);
  document.documentElement.style.setProperty("--accent-dim",    `rgba(${r},${g},${b},0.15)`);
  document.documentElement.style.setProperty("--border-accent", `rgba(${r},${g},${b},0.35)`);
  document.documentElement.style.setProperty("--text-accent",   v);
  document.documentElement.style.setProperty("--bg-selected",   `rgba(${r},${g},${b},0.25)`);
});

// ---- Navigation ----
export async function loadDirectory(path, preserveScroll = false) {
  const currentTabId = get(activeTabId);
  const prevScroll = currentTabId ? tabScrollPositions[currentTabId] : 0;
  isLoading.set(true);
  loadError.set(null);
  searchQuery.set("");
  previewEntry.set(null);
  try {
    const listing = await fetchDirectoryListing(path, get(showHidden));
    entries.set(listing.entries);
    // Klasörün kayıtlı sıralama ayarlarını uygula
    applySortForDirectory(listing.path || path);
    // Klasörün kayıtlı veya akıllı (az/çok dosya) görünüm modunu uygula
    applyViewModeForDirectory(listing.path || path, (listing.entries || []).length);
    // active tab'ı güncelle
    _updateActiveTab(t => ({
      ...t,
      path: listing.path || path,
      label: (listing.path || path).split("/").filter(Boolean).pop() || "/",
    }));
    localStorage.setItem("lastPath", listing.path || path);
    selectedPaths.set(new Set());
    invoke("start_watching", { path: listing.path || path }).catch(() => {});
    loadGitStatus(listing.path || path);
    if (preserveScroll && currentTabId) {
      setTimeout(() => restoreTabScroll(currentTabId, prevScroll), 20);
    }
  } catch (e) {
    loadError.set(String(e));
    notify("error", `Cannot open: ${e}`);
  } finally {
    isLoading.set(false);
  }
}

export function navigateTo(path) {
  showDevices.set(false); // DevicesView açıksa kapat
  const id  = get(activeTabId);
  tabs.update(ts => ts.map(t => {
    if (t.id !== id) return t;
    const newHist = [...t.history.slice(0, t.historyIndex + 1), path];
    return { ...t, history: newHist, historyIndex: newHist.length - 1 };
  }));
  loadDirectory(path);
}

export function navigateBack() {
  const id = get(activeTabId);
  const tab = get(tabs).find(t => t.id === id);
  if (!tab || tab.historyIndex <= 0) return;
  tabs.update(ts => ts.map(t =>
    t.id === id ? { ...t, historyIndex: t.historyIndex - 1 } : t
  ));
  loadDirectory(get(tabs).find(t => t.id === id).history[get(tabs).find(t => t.id === id).historyIndex]);
}

export function navigateForward() {
  const id = get(activeTabId);
  const tab = get(tabs).find(t => t.id === id);
  if (!tab || tab.historyIndex >= tab.history.length - 1) return;
  tabs.update(ts => ts.map(t =>
    t.id === id ? { ...t, historyIndex: t.historyIndex + 1 } : t
  ));
  loadDirectory(get(tabs).find(t => t.id === id).history[get(tabs).find(t => t.id === id).historyIndex]);
}

export function navigateUp() {
  const path = get(currentPath);
  if (path === "/" || path.startsWith("adb://") && (path.endsWith("/sdcard") || !path.includes("/sdcard/"))) return;
  const parent = path.split("/").slice(0, -1).join("/") || "/";
  navigateTo(parent);
}

export const rightSelectedPaths = writable(new Set());
export const rightEntries = writable([]);

// ---- Clipboard ----
export function copySelected() {
  const isRight = get(activePaneId) === "right" && get(showDualPane);
  let paths = [...(isRight ? get(rightSelectedPaths) : get(selectedPaths))];
  if (!paths.length) {
    paths = [...get(selectedPaths), ...get(rightSelectedPaths)];
  }
  if (!paths.length && get(previewEntry)?.path) {
    paths = [get(previewEntry).path];
  }
  if (!paths.length) {
    notify("info", "Kopyalanacak dosya seçilmedi");
    return;
  }
  clipboard.set({ paths, mode: "copy", op: "copy" });
  invoke("clipboard_copy", { paths }).catch(() => {});
  notify("success", `Kopyalandı: ${paths.length} öğe`);
}

export function cutSelected() {
  const isRight = get(activePaneId) === "right" && get(showDualPane);
  let paths = [...(isRight ? get(rightSelectedPaths) : get(selectedPaths))];
  if (!paths.length) {
    paths = [...get(selectedPaths), ...get(rightSelectedPaths)];
  }
  if (!paths.length && get(previewEntry)?.path) {
    paths = [get(previewEntry).path];
  }
  if (!paths.length) {
    notify("info", "Kesilecek dosya seçilmedi");
    return;
  }
  clipboard.set({ paths, mode: "cut", op: "cut" });
  invoke("clipboard_cut", { paths }).catch(() => {});
  notify("info", `Kesildi: ${paths.length} öğe — yapıştırmak için Ctrl+V`);
}

// ---- Güvenli Çakışma Yönetimli Toplu Dosya İşlemleri ----
export async function safeBatchFileOp(sources, destDir, op = "move") {
  if (!sources || !sources.length) return { success_count: 0, skipped_count: 0, failed_items: [] };

  const cleanDst = destDir.replace(/\/+$/, "") || "/";

  // 1. Hedefteki çakışmaları tespit et
  let conflicts = [];
  try {
    conflicts = await invoke("check_file_conflicts", {
      sources,
      destDir: cleanDst,
    });
  } catch (err) {
    console.warn("Çakışma kontrolü yapılamadı, normal devam ediliyor:", err);
  }

  // 2. Çakışma kararlarını belirle
  const itemDecisions = new Map(); // src -> "replace" | "rename" | "skip"

  if (conflicts && conflicts.length > 0) {
    const filePref = get(conflictFileAction);
    const dirPref = get(conflictDirAction);

    const pendingInteractiveConflicts = [];
    for (const c of conflicts) {
      const pref = c.is_dir ? dirPref : filePref;
      if (pref && pref !== "ask") {
        itemDecisions.set(c.src, pref);
      } else {
        pendingInteractiveConflicts.push(c);
      }
    }

    // Kullanıcıya interaktif sorma gerekiyorsa modalı sırayla aç
    if (pendingInteractiveConflicts.length > 0) {
      let bulkDecision = null;
      for (let i = 0; i < pendingInteractiveConflicts.length; i++) {
        const c = pendingInteractiveConflicts[i];
        if (bulkDecision) {
          itemDecisions.set(c.src, bulkDecision);
          continue;
        }

        const decision = await new Promise((resolve) => {
          conflictDialog.set({
            op,
            conflicts: pendingInteractiveConflicts,
            currentIndex: i,
            onResolve: (action, applyAll) => {
              if (applyAll) {
                bulkDecision = action;
              }
              conflictDialog.set(null);
              resolve(action);
            },
          });
        });

        if (decision === "cancel") {
          notify("info", "İşlem iptal edildi");
          return { success_count: 0, skipped_count: 0, failed_items: [] };
        }
        itemDecisions.set(c.src, decision);
      }
    }
  }

  // 3. Batch planını hazırla
  const items = [];
  for (const src of sources) {
    const filename = src.split("/").filter(Boolean).pop();
    const dst = cleanDst + "/" + filename;
    let action = itemDecisions.get(src) || "replace";
    if (op === "copy" && src === dst && !itemDecisions.has(src)) {
      action = "rename"; // Varsayılan otomatik yeni isim: (2)
    }
    if (action === "skip") continue;
    items.push({
      src,
      dst,
      conflict_action: action,
    });
  }

  if (items.length === 0) {
    notify("info", "Tüm öğeler atlandı");
    return { success_count: 0, skipped_count: sources.length, failed_items: [] };
  }

  // 4. Backend execute_file_batch komutunu çağır (Tek bir UndoBatch oluşturur!)
  try {
    const res = await invoke("execute_file_batch", {
      req: { op, items },
    });

    if (res.failed_items && res.failed_items.length > 0) {
      const firstErr = res.failed_items[0];
      const errStr = firstErr[1].toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("access denied") || errStr.includes("operation not permitted") || errStr.includes("yetki") || errStr.includes("os error 13")) {
        // Sudo ile dene
        try {
          await requestSudo(
            op === "move" ? "Taşıma İçin Yetki Gerekli" : "Kopyalama İçin Yetki Gerekli",
            `'${firstErr[0]}' için yönetici yetkisi gerekiyor.`,
            async (password) => {
              for (const item of items) {
                if (op === "move") {
                  await invoke("sudo_move_item", { src: item.src, dst: item.dst, password });
                } else {
                  await invoke("sudo_copy_item", { src: item.src, dst: item.dst, password });
                }
              }
            }
          );
          notify("success", `${items.length} öğe ${op === "move" ? "taşındı" : "kopyalandı"} (Root)`);
          return {
            success_count: items.length,
            skipped_count: 0,
            failed_items: [],
            created_paths: items.map(it => it.dst)
          };
        } catch (sudoErr) {
          notify("error", `İşlem başarısız: ${sudoErr}`);
        }
      } else {
        notify("error", `Hata (${firstErr[0]}): ${firstErr[1]}`);
      }
    } else {
      const actionName = op === "move" ? "taşındı" : "kopyalandı";
      notify("success", `${res.success_count} öğe ${actionName}`);
    }

    return res;
  } catch (err) {
    notify("error", String(err));
    return { success_count: 0, skipped_count: 0, failed_items: [[sources[0], String(err)]], created_paths: [] };
  }
}

export async function pasteClipboard() {
  let cb = get(clipboard);
  const isRight = get(activePaneId) === "right";
  const dst = isRight ? get(rightPanePath) : get(currentPath);

  // Eğer frontend store'da yoksa sistem panosunu sorgula
  if (!cb.paths || !cb.paths.length || !cb.mode) {
    try {
      const backendCb = await invoke("clipboard_get");
      if (backendCb && backendCb.paths && backendCb.paths.length) {
        cb = {
          paths: backendCb.paths,
          mode: backendCb.op === "cut" ? "cut" : "copy",
          op: backendCb.op || "copy"
        };
        clipboard.set(cb);
      }
    } catch {}
  }

  if (!cb.paths || !cb.paths.length || !cb.mode) {
    notify("info", "Pano boş (Kopyalanmış dosya yok)");
    return;
  }
  const op = cb.mode === "cut" ? "move" : "copy";
  const res = await safeBatchFileOp(cb.paths, dst, op);
  if (cb.mode === "cut" && res.success_count > 0) {
    clipboard.set({ paths: [], mode: null });
    invoke("clipboard_clear").catch(() => {});
  }
  if (isRight) {
    rightPaneNavRequest.set({ type: "navigate", path: dst });
    if (res.created_paths && res.created_paths.length > 0) {
      setTimeout(() => {
        rightSelectedPaths.set(new Set(res.created_paths));
      }, 150);
    }
  } else {
    await loadDirectory(dst, true);
    if (res.created_paths && res.created_paths.length > 0) {
      selectedPaths.set(new Set(res.created_paths));
      const allEntries = get(entries);
      const firstEntry = allEntries.find(e => e.path === res.created_paths[0]);
      if (firstEntry) {
        previewEntry.set(firstEntry);
      }
    }
  }
}

export function selectAll() {
  const isRight = get(activePaneId) === "right";
  if (isRight) {
    const all = new Set(get(rightEntries).map(e => e.path));
    rightSelectedPaths.set(all);
  } else {
    const all = new Set(get(entries).map(e => e.path));
    selectedPaths.set(all);
  }
}

// ---- File ops (Çoklu Undo kayıtlı silme) ----
export async function deleteItems(paths, permanent = false) {
  if (!paths || !paths.length) return;
  const isRight = get(activePaneId) === "right";
  try {
    const res = await invoke("batch_delete", { paths, permanent });
    if (res.failed_items && res.failed_items.length > 0) {
      const firstErr = res.failed_items[0];
      const errStr = firstErr[1].toLowerCase();
      if (errStr.includes("permission denied") || errStr.includes("access denied") || errStr.includes("operation not permitted")) {
        try {
          await requestSudo(
            "Silme İçin Yetki Gerekli",
            `'${firstErr[0]}' öğesini silmek için yönetici yetkisi gerekiyor.`,
            async (password) => {
              for (const [p] of res.failed_items) {
                await invoke("sudo_delete_item", { path: p, permanent: true, password });
              }
            }
          );
          notify("success", `${paths.length} öğe silindi (Root)`);
        } catch (sudoErr) {
          notify("error", `Silinemedi: ${sudoErr}`);
        }
      } else {
        notify("error", `Silinemedi: ${firstErr[1]}`);
      }
    } else {
      notify("success", `${res.success_count} öğe ${permanent ? "kalıcı silindi" : "çöpe taşındı"}`);
    }
  } catch (e) {
    notify("error", String(e));
  }
  if (isRight) {
    rightPaneNavRequest.set({ type: "refresh" });
  } else {
    await loadDirectory(get(currentPath));
  }
}

// ---- Search ----
export async function searchFiles(query) {
  if (!query.trim()) { searchResults.set([]); return; }
  isSearching.set(true);
  try {
    const results = await invoke("search_files", {
      root: get(currentPath), query, maxResults: 300,
    });
    searchResults.set(results);
  } catch (e) { notify("error", String(e)); }
  finally { isSearching.set(false); }
}

// ---- Bookmarks ----
export async function loadBookmarks() {
  try { bookmarks.set(await invoke("get_bookmarks")); } catch {}
}
export async function addBookmark(name, path) {
  try {
    await invoke("add_bookmark", { name, path });
    await loadBookmarks();
    notify("success", `Bookmarked: ${name}`);
  } catch (e) { notify("error", String(e)); }
}
export async function removeBookmark(path) {
  try { await invoke("remove_bookmark", { path }); await loadBookmarks(); } catch {}
}

// ---- Drives / Commands ----
export async function loadDrives() {
  try { drives.set(await invoke("get_drives")); } catch {}
}
export async function loadBlockDevices() {
  try { blockDevices.set(await invoke("list_block_devices")); } catch {}
}
export async function loadUserCommands() {
  try { userCommands.set(await invoke("load_user_commands")); } catch {}
}
export async function runCommand(commandId, selectedPath) {
  try {
    const isRight = get(activePaneId) === "right";
    const dir = isRight ? get(rightPanePath) : get(currentPath);
    const result = await invoke("run_user_command", {
      commandId, selectedPath: selectedPath || null, currentDir: dir,
    });
    if (result.success) notify("success", "Komut tamamlandı");
    else notify("error", `Komut başarısız: ${result.stderr}`);
    return result;
  } catch (e) { notify("error", String(e)); throw e; }
}

// ---- Notifications ----
let notifId = 0;
export function notify(type, message, duration = 3500) {
  const id = ++notifId;
  notifications.update(n => [...n, { id, type, message }]);
  setTimeout(() => notifications.update(n => n.filter(x => x.id !== id)), duration);
}

// ---- Derived ----
export const displayEntries = derived(
  [entries, sortBy, sortAsc, searchQuery, searchResults],
  ([$entries, $sortBy, $sortAsc, $searchQuery, $searchResults]) => {
    const raw = $searchQuery.trim() ? $searchResults : $entries;
    const list = Array.isArray(raw) ? raw : [];
    return [...list].sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      switch ($sortBy) {
        case "name":
          cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
          break;
        case "size":
          cmp = (a.size || 0) - (b.size || 0);
          break;
        case "modified":
        case "date": {
          const dateA = a.modified ? new Date(a.modified).getTime() : 0;
          const dateB = b.modified ? new Date(b.modified).getTime() : 0;
          if (!isNaN(dateA) && !isNaN(dateB) && (dateA > 0 || dateB > 0)) {
            cmp = dateA - dateB;
          } else {
            cmp = (a.modified || "").localeCompare(b.modified || "");
          }
          break;
        }
        case "perms":
        case "permissions":
          cmp = (a.permissions || "").localeCompare(b.permissions || "");
          break;
        case "type":
          cmp = (a.extension || "").localeCompare(b.extension || "");
          break;
        default:
          cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
      }
      if (cmp === 0) {
        cmp = (a.name || "").localeCompare(b.name || "", undefined, { numeric: true, sensitivity: "base" });
      }
      return $sortAsc ? cmp : -cmp;
    });
  }
);
