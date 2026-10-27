mod fs_ops;
mod mounts;
mod commands;
mod clipboard;
mod watcher;
mod preview;
mod archive;
mod du_cache;
mod plugins;
mod network;
mod git;
mod thumbnails;
mod open_with;
mod media_server;
pub mod adb;

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
            media_server::start_media_server();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Filesystem
            fs_ops::list_directory,
            fs_ops::sudo_list_directory,
            fs_ops::verify_sudo_password,
            fs_ops::set_session_sudo_password,
            fs_ops::get_file_info,
            fs_ops::get_file_properties,
            fs_ops::set_file_permissions,
            fs_ops::sudo_set_file_permissions,
            fs_ops::set_file_owner,
            fs_ops::sudo_set_file_owner,
            fs_ops::get_system_users_and_groups,
            fs_ops::create_dir,
            fs_ops::create_file,
            fs_ops::delete_item,
            fs_ops::rename_item,
            fs_ops::copy_item,
            fs_ops::move_item,
            fs_ops::sudo_create_dir,
            fs_ops::sudo_create_file,
            fs_ops::sudo_delete_item,
            fs_ops::sudo_rename_item,
            fs_ops::sudo_copy_item,
            fs_ops::sudo_move_item,
            fs_ops::read_text_file,
            fs_ops::get_drives,
            fs_ops::search_files,
            fs_ops::path_exists,
            fs_ops::path_is_dir,
            fs_ops::debug_log,
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
            commands::open_external_url,
            commands::send_crash_report,
            // Clipboard + Undo + Safe Batch Ops
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
            clipboard::check_file_conflicts,
            clipboard::execute_file_batch,
            clipboard::batch_delete,
            clipboard::restore_trash_items,
            clipboard::restore_all_trash,
            clipboard::empty_trash,
            // Watcher (inotify)
            watcher::start_watching,
            watcher::stop_watching,
            // Preview
            preview::get_file_preview,
            // Archive
            archive::extract_archive,
            archive::compress_files,
            archive::is_archive,
            archive::get_checksum,
            // Du cache
            du_cache::get_dir_size,
            du_cache::sudo_get_dir_size,
            du_cache::get_big_files,
            du_cache::invalidate_du_cache,
            du_cache::clear_du_cache,
            du_cache::get_du_cache_stats,
            // Plugins
            plugins::list_plugins,
            plugins::run_plugin,
            plugins::set_plugin_enabled,
            plugins::get_plugins_dir,
            plugins::create_example_plugin,
            // Network shares
            network::list_smb_shares,
            network::mount_smb,
            network::mount_nfs,
            network::list_network_mounts,
            network::unmount_network,
            network::ping_host,
            // Git
            git::get_git_status,
            // Thumbnails
            thumbnails::get_thumbnail,
            thumbnails::get_video_hover_frame,
            thumbnails::clear_thumbnail_cache,
            thumbnails::get_thumbnail_cache_stats,
            // Open With
            open_with::get_open_with_info,
            open_with::get_mime_type,
            open_with::open_with_app,
            open_with::set_default_app,
            open_with::reset_default_for_mime,
            open_with::open_with_custom_cmd,
            open_with::add_app_association,
            open_with::get_all_apps,
            open_with::open_file_default,
            // Media Streamer (HTTP with Range for video/audio)
            media_server::get_media_url,
        ])
        .run(tauri::generate_context!())
        .expect("arch-explorer crash — tauri context hatası");
}
