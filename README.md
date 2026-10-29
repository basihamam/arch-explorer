# ◈ Arch Explorer 0.2v

<p align="center">
  <img src="docs/screenshots/arch_explorer_multiselection_transparent.gif" alt="Arch Explorer Transparent Demo" width="90%">
</p>

<p align="center">
  <strong>Modern, transparent glassmorphism file manager for Arch Linux.</strong><br>
  Instant Launch · Zero Setup · True Desktop Wallpaper Transparency · Built-in ADB Android Support
</p>

---

## 🚀 İndir ve Doğrudan Çalıştır (Hemen Kullan!)

Bu repoda derleme veya kod karmaşası yok! Doğrudan çalışan uygulama ve Arch Linux paketi hazır olarak yer almaktadır:

### 1. Doğrudan Çalıştır (Tek Komut):
```bash
git clone https://github.com/basihamam/arch-explorer-tauri.git
cd arch-explorer-tauri
./arch-explorer
```
*(veya `./arch-explorer` dosyasına çift tıklayarak anında başlatabilirsiniz!)*

### 2. Sisteme Kurmak İsterseniz:
```bash
# Kurulum betiği ile:
sudo ./install.sh

# Veya pacman paketi ile:
sudo pacman -U arch-explorer-bin-0.2.0-1-x86_64.pkg.tar.zst
```

---

## 📸 Arayüz & Transparan Cam Vitrini

### Saydam Masaüstü & Akrilik Bulanıklık
Arch Explorer arkasındaki duvar kağıdını ve masaüstü pencerelerini tül gibi gösterir. Donanım hızlandırmalı blur ve özel opaklık ayarlarıyla tamamen kişiselleştirilebilir.

<p align="center">
  <img src="docs/screenshots/01_arch_explorer_main_transparent.png" alt="Ana Pencere Saydamlığı" width="88%">
</p>

### Canlı Tema ve Ayarlar Paneli
8 adet hazır tema, renk seçici, arayüz bulanıklığı (0–40px) ve panel şeffaflığı.

<p align="center">
  <img src="docs/screenshots/02_settings_appearance_transparent.png" alt="Ayarlar ve Saydam Blur" width="88%">
</p>

### Yerleşim & Klasör Önizleme Kontrolleri
İster her zaman açık, ister sadece dosyalarda veya otomatik. Klasörler için önizleme tek tıkla açılıp kapatılabilir.

<p align="center">
  <img src="docs/screenshots/04_settings_layout_folder_preview.png" alt="Klasör Önizleme Ayarları" width="88%">
</p>

### Saydam Sağ Tık ve Çöp Kutusu Yönetimi
Tek tıkla arşiv çıkarma, SHA256/MD5 sağlama toplamı, çöp kutusundan tekli veya toplu geri yükleme.

<p align="center">
  <img src="docs/screenshots/05_context_menu_transparent.png" alt="Saydam Bağlam Menüsü" width="88%">
</p>

---

## ✨ Öne Çıkan Özellikler

- **Masaüstü Şeffaflığı:** Gerçek saydamlık ve canlı blur filtresi.
- **%100 İki Dilli (TR & EN):** Türkçe ve İngilizce arasında sayfa yenilemeden anlık geçiş.
- **Akıllı Önizleme:** F3 ile açılan, klasör boyutlarını ve disk kullanımını analiz eden önizleme paneli.
- **Çift Panel (Ctrl+P):** Yan yana iki bağımsız klasör paneli ile hızlı dosya yönetimi.
- **Yerleşik Android / ADB Entegrasyonu:** USB ile bağlı telefonları sürücü gibi listeler ve dosyaları doğrudan yönetir.
- **Güvenli Geri Alma (Ctrl+Z):** Yanlışlıkla yapılan veya üzerine yazılan dosyalar güvenle kurtarılır.
- **Sıfır Bellek Şişmesi:** Dev boyutlu klasörlerde sanal pencerelenme (Virtual Chunking) ve tek tıkla RAM temizliği.

---

## ⌨️ Temel Kısayollar

| Kısayol | İşlev |
|---|---|
| `Alt + ←` / `Alt + →` | Geri / İleri |
| `Alt + ↑` | Üst Klasöre Git |
| `F3` | Önizleme Panelini Aç / Kapat |
| `F5` | Klasörü Yenile |
| `Ctrl + P` | Çift Panel (Dual Pane) |
| `Ctrl + T` | Klasörde Terminal Aç |
| `Ctrl + Z` | Son İşlemi Geri Al |
| `Alt + Enter` | Dosya / Klasör Özellikleri & İzinleri |

---

## 👤 Geliştirici & İletişim

- **Geliştirici:** LordBlue / ALX ([@basihamam](https://github.com/basihamam))
- **Depo:** [basihamam/arch-explorer-tauri](https://github.com/basihamam/arch-explorer-tauri)
- **Sürüm:** 0.2v (Open Source Community Edition)
