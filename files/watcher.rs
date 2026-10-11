// watcher.rs — inotify tabanlı dizin izleme
// Mevcut dizin değişince frontend'e "dir-changed" event'i gönderir.
// loadDirectory her çağrıldığında start_watching çağrılır, eski watcher drop edilir.

use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::sync::Mutex;
use tauri::{AppHandle, State};

/// Global watcher state — Tauri .manage() ile inject edilir
pub struct WatcherState(pub Mutex<Option<RecommendedWatcher>>);

/// Yeni dizin izlemeye başla (eski watcher otomatik drop edilir)
#[tauri::command]
pub fn start_watching(
    path: String,
    app: AppHandle,
    state: State<'_, WatcherState>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;

    // Eski watcher'ı kapat
    *guard = None;

    let app_handle = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<Event>| {
        if let Ok(event) = res {
            // Rename/Create/Remove/Modify — hepsini yakala, sadece kind'a göre filtrele
            use notify::EventKind::*;
            let relevant = matches!(
                event.kind,
                Create(_) | Remove(_) | Modify(_) | Rename(_)
            );
            if relevant {
                // Frontend debounce yapıyor (400ms), biz sadece sinyal gönderiyoruz
                let _ = app_handle.emit("dir-changed", ());
            }
        }
    })
    .map_err(|e| format!("Watcher oluşturulamadı: {}", e))?;

    watcher
        .watch(std::path::Path::new(&path), RecursiveMode::NonRecursive)
        .map_err(|e| format!("İzleme başlatılamadı: {}", e))?;

    *guard = Some(watcher);
    Ok(())
}

/// İzlemeyi durdur (uygulama kapatılırken veya gerektiğinde)
#[tauri::command]
pub fn stop_watching(state: State<'_, WatcherState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}
