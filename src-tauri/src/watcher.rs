// watcher.rs — inotify tabanlı dizin izleme
use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use crate::du_cache;

pub struct WatcherState(pub Mutex<Option<RecommendedWatcher>>);

#[tauri::command]
pub fn start_watching(
    path: String,
    app: AppHandle,
    state: State<'_, WatcherState>,
) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;

    let app_handle = app.clone();
    let mut watcher = notify::recommended_watcher(move |res: NotifyResult<Event>| {
        if let Ok(event) = res {
            let relevant = !matches!(event.kind, notify::EventKind::Access(_));
            if relevant {
                // Değişen path'in du + dir cache'ini temizle
                for path in &event.paths {
                    let path_str = path.to_string_lossy().to_string();
                    du_cache::invalidate_du_cache(path_str.clone());
                    crate::fs_ops::invalidate_dir_cache(&path_str);
                }
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

#[tauri::command]
pub fn stop_watching(state: State<'_, WatcherState>) -> Result<(), String> {
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = None;
    Ok(())
}
