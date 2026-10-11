mod fs_ops;
mod mounts;
mod commands;
mod clipboard;
mod watcher;
mod preview;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(watcher::WatcherState(std::sync::Mutex::new(None)))
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.set_decorations(false).ok();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Filesystem
            fs_ops::list_directory,
            fs_ops::get_file_info,
            fs_ops::create_dir,
            fs_ops::create_file,
            fs_ops::delete_item,
            fs_ops::rename_item,
            fs_ops::copy_item,
            fs_ops::move_item,
            fs_ops::read_text_file,
            fs_ops::get_drives,
            fs_ops::search_files,
            fs_ops::path_exists,
            fs_ops::path_is_dir,
            // Bookmarks
            fs_ops::get_bookmarks,
            fs_ops::add_bookmark,
            fs_ops::add_bookmark_full,
            fs_ops::update_bookmark,
            fs_ops::remove_bookmark,
            fs_ops::reorder_bookmarks,
            // Drag-drop
            fs_ops::drag_drop_files,
            fs_ops::drop_to_bookmark_sidebar,
            // Mounts
            mounts::list_block_devices,
            mounts::mount_device,
            mounts::unmount_device,
            mounts::mount_ntfs_windows_disk,
            mounts::mount_ntfs_force_fix,
            mounts::mount_ntfs_with_password,
            mounts::unmount_with_password,
            mounts::get_mount_status,
            mounts::get_fallback_state,
            mounts::reset_fallback_state,
            // User commands
            commands::load_user_commands,
            commands::run_user_command,
            commands::save_user_command,
            commands::delete_user_command,
            // Clipboard + Undo
            clipboard::clipboard_copy,
            clipboard::clipboard_cut,
            clipboard::clipboard_paste,
            clipboard::clipboard_get,
            clipboard::clipboard_clear,
            clipboard::undo_last,
            clipboard::get_undo_stack,
            clipboard::clear_undo_stack,
            clipboard::tracked_rename,
            clipboard::tracked_create_file,
            clipboard::tracked_create_dir,
            clipboard::tracked_delete,
            // Watcher (inotify)
            watcher::start_watching,
            watcher::stop_watching,
            // Preview
            preview::get_file_preview,
        ])
        .run(tauri::generate_context!())
        .expect("arch-explorer crash — tauri context hatası");
}
