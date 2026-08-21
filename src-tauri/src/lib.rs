mod commands;
mod error;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::reindex,
            commands::search_symbols,
            commands::get_symbol_profile,
            commands::get_callers,
            commands::get_callees,
            commands::get_blast_radius,
            commands::get_before_you_change_this,
            commands::get_history,
            commands::get_related_tests,
            commands::export_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
