// i18n.js — Turkish (tr) and English (en) localization engine for Arch Explorer
import { writable, derived } from "svelte/store";

export const currentLocale = writable(localStorage.getItem("appLocale") || "en");
currentLocale.subscribe(l => localStorage.setItem("appLocale", l));

export function setLocale(lang) {
  if (lang === "tr" || lang === "en") {
    currentLocale.set(lang);
  }
}

export function toggleLocale() {
  currentLocale.update(l => l === "tr" ? "en" : "tr");
}

const translations = {
  tr: {
    // Toolbar & Nav
    back: "Geri (Alt+←)",
    forward: "İleri (Alt+→)",
    up: "Yukarı (Alt+↑)",
    refresh: "Yenile (F5)",
    search: "Dosya veya klasör ara…",
    search_placeholder: "Ara…",
    grid_view: "Izgara Görünümü (Grid)",
    list_view: "Liste Görünümü (List)",
    show_hidden: "Gizli Dosyaları Göster/Gizle",
    preview_panel: "Önizleme Paneli (F3)",
    dual_pane: "Çift Panel (Ctrl+P)",
    settings: "Ayarlar",
    language: "Dil",
    lang_tr: "Türkçe",
    lang_en: "English",

    // Tabs
    new_tab: "Yeni Sekme",
    close_tab: "Sekmeyi Kapat",

    // Sidebar & Bookmarks
    bookmarks: "Yer İmleri",
    devices: "Devices",
    add_bookmark: "Geçerli Klasörü Ekle",
    all_devices: "Devices",
    no_bookmarks: "Henüz yer imi yok",
    no_devices: "Aygıt bulunamadı",
    open_folder: "Aç",
    open_in_new_tab: "Yeni Sekmede Aç",
    rename: "Yeniden Adlandır",
    pin_to_top: "En Üste Sabitle",
    unpin: "Sabitlemeyi Kaldır",
    remove_bookmark: "Yer İmini Kaldır",
    go_to_disk: "Diske Git",
    unmount_disk: "Diski Çıkar / Ayır (Unmount)",
    mount_disk: "Diski Bağla (Mount)",
    hide_from_sidebar: "Sidebar'da Gizle",
    show_hidden_devices: "Gizlenen Diskleri Göster",
    more_devices: "daha fazla",
    less_devices: "Daha az",

    // Context Menu
    cut: "Kes",
    copy: "Kopyala",
    paste: "Yapıştır",
    delete: "Sil",
    move_to_trash: "Çöpe Taşı",
    permanent_delete: "Kalıcı Olarak Sil",
    new_folder: "Yeni Klasör",
    new_file: "Yeni Metin Dosyası",
    open_with: "Birlikte Aç (Open With)",
    all_apps: "Tüm Uygulamalar…",
    select_app: "Uygulama Seç…",
    recommended_apps: "Önerilen Uygulamalar",
    default_app: "Varsayılan Uygulama",
    other_apps: "Diğer Uygulamalar",
    compress: "Arşivle / Sıkıştır…",
    extract: "Buraya Çıkar",
    extract_to: "Klasöre Çıkar…",
    properties: "Özellikler",
    checksum: "Sağlama Toplamı (Checksum)",
    open_in_terminal: "Burada Terminal Aç",
    select_all: "Tümünü Seç",

    // Open With Panel
    open_with_title: "Birlikte Aç",
    search_apps: "Uygulama ara…",
    set_as_default: "Bu dosya türü için varsayılan yap",
    restore_system_default: "Sistem Varsayılanına Sıfırla",
    run_custom_cmd: "Özel Komut veya Betik Çalıştır:",
    browse: "Gözat",
    cancel: "İptal",
    open: "Aç",
    app_not_found: "Uygulama bulunamadı",

    // Preview Panel
    preview_title: "Önizleme",
    name: "Ad",
    size: "Boyut",
    modified: "Değiştirilme",
    permissions: "İzinler",
    items_count: "öğe",
    largest_files: "En Büyük Dosyalar",
    scanning: "Taranıyor…",
    used: "Kullanılan",
    free: "Boş",
    device: "Aygıt",
    current_disk: "Mevcut Disk",
    video_fs_enter: "Tam Ekran",
    video_fs_exit: "Çık",
    open_in_system_player: "Sistem Oynatıcısında Aç (mpv / VLC)",
    video_unsupported: "Bu video WebKit gömülü oynatıcısında gösterilemedi. Harici oynatıcı ile anında izleyebilirsiniz:",

    // Conflict Dialog
    conflict_title: "Dosya Çakışması",
    conflict_desc: "Hedef klasörde aynı isimde bir dosya zaten mevcut:",
    action_replace: "Üzerine Yaz (Değiştir)",
    action_rename: "Yeniden Adlandır (Yeni isim ver)",
    action_skip: "Bu Dosyayı Atla",
    apply_to_all: "Kalan tüm çakışmalarda aynı işlemi uygula",

    // Sudo & Auth
    sudo_title: "Yönetici İzni Gerekli",
    sudo_desc: "Bu işlem için root/yönetici yetkisi gerekiyor. Lütfen şifrenizi girin:",
    confirm: "Onayla",

    // Status bar
    selected_count: "seçildi",
    total_count: "öğe",
  },

  en: {
    // Toolbar & Nav
    back: "Back (Alt+←)",
    forward: "Forward (Alt+→)",
    up: "Up (Alt+↑)",
    refresh: "Refresh (F5)",
    search: "Search files or folders…",
    search_placeholder: "Search…",
    grid_view: "Grid View",
    list_view: "List View",
    show_hidden: "Show/Hide Hidden Files",
    preview_panel: "Preview Panel (F3)",
    dual_pane: "Dual Pane (Ctrl+P)",
    settings: "Settings",
    language: "Language",
    lang_tr: "Türkçe",
    lang_en: "English",

    // Tabs
    new_tab: "New Tab",
    close_tab: "Close Tab",

    // Sidebar & Bookmarks
    bookmarks: "Bookmarks",
    devices: "Devices",
    add_bookmark: "Bookmark Current Folder",
    all_devices: "Devices",
    no_bookmarks: "No bookmarks yet",
    no_devices: "No devices found",
    open_folder: "Open",
    open_in_new_tab: "Open in New Tab",
    rename: "Rename",
    pin_to_top: "Pin to Top",
    unpin: "Unpin",
    remove_bookmark: "Remove Bookmark",
    go_to_disk: "Go to Disk",
    unmount_disk: "Unmount / Eject Disk",
    mount_disk: "Mount Disk",
    hide_from_sidebar: "Hide from Sidebar",
    show_hidden_devices: "Show Hidden Drives",
    more_devices: "more",
    less_devices: "Less",

    // Context Menu
    cut: "Cut",
    copy: "Copy",
    paste: "Paste",
    delete: "Delete",
    move_to_trash: "Move to Trash",
    permanent_delete: "Permanently Delete",
    new_folder: "New Folder",
    new_file: "New Text File",
    open_with: "Open With",
    all_apps: "All Applications…",
    select_app: "Select Application…",
    recommended_apps: "Recommended Applications",
    default_app: "Default Application",
    other_apps: "Other Applications",
    compress: "Compress / Archive…",
    extract: "Extract Here",
    extract_to: "Extract to Folder…",
    properties: "Properties",
    checksum: "Checksum",
    open_in_terminal: "Open Terminal Here",
    select_all: "Select All",

    // Open With Panel
    open_with_title: "Open With",
    search_apps: "Search apps…",
    set_as_default: "Set as default for this file type",
    restore_system_default: "Reset to System Default",
    run_custom_cmd: "Run custom command or script:",
    browse: "Browse",
    cancel: "Cancel",
    open: "Open",
    app_not_found: "No application found",

    // Preview Panel
    preview_title: "Preview",
    name: "Name",
    size: "Size",
    modified: "Modified",
    permissions: "Permissions",
    items_count: "items",
    largest_files: "Largest Files",
    scanning: "Scanning…",
    used: "Used",
    free: "Free",
    device: "Device",
    current_disk: "Current Disk",
    video_fs_enter: "Fullscreen",
    video_fs_exit: "Exit",
    open_in_system_player: "Open in System Player (mpv / VLC)",
    video_unsupported: "This video cannot be played in WebKit embedded player. You can watch it directly in your external player:",

    // Conflict Dialog
    conflict_title: "File Conflict",
    conflict_desc: "A file with the same name already exists in target folder:",
    action_replace: "Replace (Overwrite)",
    action_rename: "Rename (Generate new name)",
    action_skip: "Skip this file",
    apply_to_all: "Apply to all remaining conflicts",

    // Sudo & Auth
    sudo_title: "Root Privileges Required",
    sudo_desc: "Root privileges required for this operation. Please enter password:",
    confirm: "Confirm",

    // Status bar
    selected_count: "selected",
    total_count: "items",
  }
};

export const t = derived(currentLocale, ($locale) => {
  return (key, fallback = "") => {
    return translations[$locale]?.[key] ?? translations.en?.[key] ?? fallback ?? key;
  };
});
