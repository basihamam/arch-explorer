// watcher.rs — inotify file watcher (tauri bağımlılığı kaldırıldı)
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub fn watch_directory(path: &str, tx: Sender<String>) -> anyhow::Result<RecommendedWatcher> {
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if let Some(p) = event.paths.first() {
                    tx.send(p.to_string_lossy().to_string()).ok();
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(500)),
    )?;

    watcher.watch(Path::new(path), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
