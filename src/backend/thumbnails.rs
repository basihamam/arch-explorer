// src/backend/thumbnails.rs
// Disk-persistent thumbnail cache.
// 3 kalite seviyesi: Low(64) / Mid(128) / High(256)
// Crop-to-fill (cover) mod — aspect ratio bozulmuyor, merkez crop.
// Bir kere üretilip ~/.cache/arch-explorer/thumbnails/ altına kaydedilir.
// Kaynak dosya mtime değişirse stale sayılır ve yeniden üretilir.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThumbQuality { Low, Mid, High }

impl ThumbQuality {
    pub fn px(self) -> u32 {
        match self { Self::Low => 64, Self::Mid => 128, Self::High => 256 }
    }
    pub fn jpeg_q(self) -> u8 {
        match self { Self::Low => 70, Self::Mid => 82, Self::High => 90 }
    }
    pub fn suffix(self) -> &'static str {
        // px boyutunu suffix'e dahil et — kalite değişince eskiler karışmaz
        match self { Self::Low => "64", Self::Mid => "128", Self::High => "256" }
    }
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("arch-explorer")
        .join("thumbnail")
}

fn thumb_path(src: &Path, quality: ThumbQuality) -> PathBuf {
    // Dosya path'ini hash'le — çakışmayı önle
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    let hash = h.finish();
    cache_dir().join(format!("{:x}_{}.jpg", hash, quality.suffix()))
}

fn src_mtime(path: &Path) -> Option<SystemTime> {
    path.metadata().ok()?.modified().ok()
}

fn thumb_mtime(thumb: &Path) -> Option<SystemTime> {
    thumb.metadata().ok()?.modified().ok()
}

/// Thumbnail'i döner. Önce cache'i kontrol eder, stale ise yeniden üretir.
/// `None` → üretilemedi (desteklenmeyen format vb.)
pub fn get_thumbnail(src_path: &str, quality: ThumbQuality) -> Option<Vec<u8>> {
    let src  = Path::new(src_path);
    let dst  = thumb_path(src, quality);

    // Cache geçerli mi?
    if dst.exists() {
        if let (Some(src_t), Some(dst_t)) = (src_mtime(src), thumb_mtime(&dst)) {
            if dst_t >= src_t {
                return std::fs::read(&dst).ok();
            }
        }
    }

    // Üret + kaydet
    generate_and_save(src, &dst, quality)
}

fn generate_and_save(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let ext = src.extension()?.to_str()?.to_lowercase();

    // GIF — sadece ilk kare
    if ext == "gif" {
        return generate_gif_thumb(src, dst, quality);
    }

    // SVG — rsvg-convert veya inkscape ile rasterize
    if ext == "svg" {
        return generate_svg_thumb(src, dst, quality);
    }

    // Video — ffmpeg ile ilk saniyeden kare yakala
    if matches!(ext.as_str(), "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv"|"wmv"|"m4v"|"ts") {
        return generate_video_thumb(src, dst, quality);
    }

    // Ses — ffmpeg ile gömülü albüm kapağını çıkar
    if matches!(ext.as_str(), "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a"|"opus"|"wma") {
        return generate_audio_thumb(src, dst, quality);
    }

    // Normal raster formatlar
    let img = image::open(src).ok().or_else(|| {
        let path_str = src.to_string_lossy().to_string();
        let bytes = crate::backend::fs_ops::read_file_bytes_elevated(&path_str).ok()?;
        image::load_from_memory(&bytes).ok()
    })?;
    let px  = quality.px();

    // Crop-to-fill: önce cover resize, sonra merkez crop
    let cropped = crop_to_fill(img, px, px);

    // JPEG olarak kaydet (quality 75 — thumbnail için yeterli)
    std::fs::create_dir_all(dst.parent()?).ok()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q());
    enc.encode_image(&cropped).ok()?;
    std::fs::write(dst, &buf).ok();
    Some(buf)
}

fn generate_video_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    for ss in ["00:00:01", "00:00:00"] {
        let out = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-ss", ss,
                "-i", src.to_str()?,
                "-vframes", "1",
                "-f", "image2",
                "-vcodec", "mjpeg",
                "pipe:1",
            ])
            .output();
        if let Ok(o) = out {
            if o.status.success() && !o.stdout.is_empty() {
                if let Ok(img) = image::load_from_memory(&o.stdout) {
                    let cropped = crop_to_fill(img, px, px);
                    std::fs::create_dir_all(dst.parent()?).ok()?;
                    let mut buf = Vec::new();
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
                        .encode_image(&cropped).ok()?;
                    std::fs::write(dst, &buf).ok();
                    return Some(buf);
                }
            }
        }
    }
    None
}

fn generate_audio_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", src.to_str()?,
            "-an",
            "-vframes", "1",
            "-f", "image2",
            "-vcodec", "mjpeg",
            "pipe:1",
        ])
        .output();
    if let Ok(o) = out {
        if o.status.success() && !o.stdout.is_empty() {
            if let Ok(img) = image::load_from_memory(&o.stdout) {
                let cropped = crop_to_fill(img, px, px);
                std::fs::create_dir_all(dst.parent()?).ok()?;
                let mut buf = Vec::new();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
                    .encode_image(&cropped).ok()?;
                std::fs::write(dst, &buf).ok();
                return Some(buf);
            }
        }
    }
    None
}

/// Cover resize + center crop
fn crop_to_fill(img: image::DynamicImage, tw: u32, th: u32) -> image::DynamicImage {
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 { return img; }

    // Scale to cover: fit en küçük kenarı hedef boyutuna
    let scale_x = tw as f64 / iw as f64;
    let scale_y = th as f64 / ih as f64;
    let scale   = scale_x.max(scale_y);

    let new_w = (iw as f64 * scale).ceil() as u32;
    let new_h = (ih as f64 * scale).ceil() as u32;

    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle);

    // Merkez crop
    let cx = (new_w.saturating_sub(tw)) / 2;
    let cy = (new_h.saturating_sub(th)) / 2;
    resized.crop_imm(cx, cy, tw.min(new_w), th.min(new_h))
}

fn generate_gif_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let path_str = src.to_str()?;
    let frames = crate::ui::preview_panel::load_gif_frames(path_str);
    let frame = frames.into_iter().next()?;
    let img_buf = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)?;
    let img = image::DynamicImage::ImageRgba8(img_buf);
    let px = quality.px();
    let cropped = crop_to_fill(img, px, px);

    std::fs::create_dir_all(dst.parent()?).ok()?;
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
        .encode_image(&cropped).ok()?;
    std::fs::write(dst, &buf).ok();
    Some(buf)
}

fn generate_svg_thumb(src: &Path, dst: &Path, quality: ThumbQuality) -> Option<Vec<u8>> {
    let px = quality.px();
    // rsvg-convert dene (librsvg)
    let tmp = dst.with_extension("_tmp.png");
    let ok = std::process::Command::new("rsvg-convert")
        .args(["-w", &px.to_string(), "-h", &px.to_string(),
               "-o", tmp.to_str()?, src.to_str()?])
        .status().ok()?.success();

    if ok && tmp.exists() {
        let img  = image::open(&tmp).ok()?;
        let _ = std::fs::remove_file(&tmp);
        let cropped = crop_to_fill(img, px, px);
        std::fs::create_dir_all(dst.parent()?).ok()?;
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
            .encode_image(&cropped).ok()?;
        std::fs::write(dst, &buf).ok();
        return Some(buf);
    }

    // inkscape fallback
    let ok2 = std::process::Command::new("inkscape")
        .args(["--export-type=png",
               &format!("--export-width={}", px),
               &format!("--export-filename={}", dst.with_extension("_tmp2.png").display()),
               src.to_str()?])
        .status().ok()?.success();
    let tmp2 = dst.with_extension("_tmp2.png");
    if ok2 && tmp2.exists() {
        let img = image::open(&tmp2).ok()?;
        let _ = std::fs::remove_file(&tmp2);
        let cropped = crop_to_fill(img, px, px);
        std::fs::create_dir_all(dst.parent()?).ok()?;
        let mut buf = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.jpeg_q())
            .encode_image(&cropped).ok()?;
        std::fs::write(dst, &buf).ok();
        return Some(buf);
    }

    None
}

/// JPEG bytes'ı egui ColorImage'a çevir
pub fn jpeg_to_color_image(jpeg: &[u8]) -> Option<egui::ColorImage> {
    let img  = image::load_from_memory(jpeg).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize], rgba.as_raw()))
}

/// Tüm cache'i temizle
pub fn clear_cache() {
    let _ = std::fs::remove_dir_all(cache_dir());
}

pub fn cache_entry_count() -> usize {
    std::fs::read_dir(cache_dir())
        .map(|d| d.count()).unwrap_or(0)
}
