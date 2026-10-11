# ◈ Arch Explorer

Blazing fast, fully customizable file explorer for Arch Linux.
Built with Rust + Tauri + Svelte.

## Features

- **Full transparency/blur** — backdrop-filter, tam kontrol
- **Custom commands** — kendi komutlarını ekle, context menu'ya bas
- **NTFS Fix** — Windows disklerini tek tıkla mount et
- **Grid + List view** — istediğin gibi bak
- **Bookmarks** — sağ tıkla bookmark ekle/sil
- **Theming** — accent rengi, opacity, dark/light, 8 preset
- **Custom CSS** — `src/app.css`'de `--variable` ile her şeyi değiştir
- **Search** — bulunduğun klasörde recursive arama
- **Keyboard shortcuts** — Alt+←/→/↑, F5, Ctrl+T...

## Install

```bash
chmod +x scripts/install.sh
./scripts/install.sh
```

## Dev

```bash
npm run tauri dev
```

## Build

```bash
npm run tauri build
# Binary: src-tauri/target/release/arch-explorer
```

## Custom Commands

Titlebar'daki **Commands** butonuna tıkla.

Placeholder'lar:
- `{path}` — seçili dosyanın tam yolu
- `{file}` — sadece dosya adı
- `{dir}` — üst klasör
- `{name}` — uzantısız isim

Örnek komutlar (varsayılan olarak gelir):
- Open Terminal Here
- Git Status / Git Log
- Copy Path
- Compress tar.gz
- Open as Root

## Theming

`src/app.css`'deki `:root` bloğunu düzenle:

```css
:root {
  --accent: #8264ff;        /* Ana renk */
  --bg-primary: rgba(15, 15, 20, 0.82);  /* Opacity burada */
  --blur-main: blur(24px);  /* Blur miktarı */
  /* ... */
}
```

Veya Settings panelinden live olarak değiştir.

## NTFS / Windows Disk

Disks butonundan Windows diskleri görürsün.
"Mount (NTFS Fix)" tıkla — otomatik `uid/gid` permission fix uygular.

Gereklilik: `ntfs-3g` (install script kuruyor)

## Config Files

- `~/.config/arch-explorer/commands.toml` — kullanıcı komutları
- `~/.config/arch-explorer/bookmarks.json` — bookmarklar
