mod clipboard;
mod config;
mod database;
mod frecency;
mod search;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use clipboard::copy_with_clear;
use config::Config;
use database::Entry;
use frecency::FrecencyData;
use search::Searcher;
use ui::{AppState, Mode};
use wayland_client::Connection;

fn main() {
    // 1. Load config and frecency data
    let config = match Config::load() {
        Ok(c) => Rc::new(RefCell::new(c)),
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            if let Ok(path) = Config::data_dir() {
                eprintln!("Config file: {}/config.toml", path.display());
            }
            std::process::exit(1);
        }
    };
    let frecency = Rc::new(RefCell::new(FrecencyData::load()));

    // 2. Get database path from config
    let db_path = match config.borrow().expand_database_path() {
        Some(p) => p,
        None => {
            eprintln!("No database path configured.");
            eprintln!("Please set database_path in your config file.");
            if let Ok(path) = Config::data_dir() {
                eprintln!("Config file: {}/config.toml", path.display());
            }
            eprintln!("\nExample config:");
            eprintln!("  database_path = \"~/passwords.kdbx\"");
            std::process::exit(1);
        }
    };

    // Store clipboard timeout before the event loop
    let clipboard_timeout = config.borrow().clipboard_timeout;

    // Check database exists before launching UI
    if !db_path.exists() {
        eprintln!("Database not found: {}", db_path.display());
        std::process::exit(1);
    }

    // 3. Set up UI - starts in password mode
    let conn = Connection::connect_to_env().unwrap_or_else(|e| {
        eprintln!("Failed to connect to Wayland display: {}", e);
        eprintln!("kpick requires a running Wayland compositor.");
        std::process::exit(1);
    });
    let (mut app_state, mut event_queue) = AppState::new(&conn, &config.borrow(), db_path)
        .unwrap_or_else(|e| {
            eprintln!("{}", e);
            std::process::exit(1);
        });

    // Shared state for entries and search
    let entries: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));
    let searcher = Rc::new(RefCell::new(Searcher::new()));
    let current_results: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));

    // Selected entry tracking - only set when user confirms with Enter
    let selected_entry: Rc<RefCell<Option<Entry>>> = Rc::new(RefCell::new(None));
    // Track whether to copy username (Shift+Enter) instead of password
    let copy_username: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    // 4. Set callbacks

    // On unlock: store entries
    {
        let entries = entries.clone();
        app_state.on_unlock = Some(Box::new(move |new_entries| {
            if new_entries.is_empty() {
                eprintln!("No password entries found.");
                return;
            }

            *entries.borrow_mut() = new_entries;
        }));
    }

    // On select (Enter in picker mode): store the selected entry
    {
        let selected_entry = selected_entry.clone();
        let current_results = current_results.clone();
        let copy_username = copy_username.clone();
        app_state.on_select = Some(Box::new(move |index, should_copy_username| {
            let results = current_results.borrow();
            if index < results.len() {
                *selected_entry.borrow_mut() = Some(results[index].clone());
                *copy_username.borrow_mut() = should_copy_username;
            }
        }));
    }

    // On escape: just exit
    app_state.on_escape = Some(Box::new(|| {}));

    // 5. Run event loop
    {
        let qh = event_queue.handle();
        let mut last_query = String::new();
        let mut was_password_mode = true;

        while app_state.running {
            // Flush pending requests
            conn.flush().unwrap();

            // Blocking dispatch - waits for events
            event_queue.blocking_dispatch(&mut app_state).unwrap();

            // Check if we just transitioned from password to picker mode
            let is_picker_mode = app_state.mode == Mode::Picker;
            if is_picker_mode && was_password_mode {
                // Initialize entries list after unlock
                let mut search = searcher.borrow_mut();
                let results = search.search("", &entries.borrow(), &frecency.borrow());
                let display_entries: Vec<(String, String)> = results
                    .iter()
                    .map(|r| (r.entry.title.clone(), r.entry.username.clone()))
                    .collect();
                *current_results.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
                app_state.set_entries(display_entries);
                was_password_mode = false;
            }

            // In picker mode, update entries when query changes
            if is_picker_mode && app_state.query != last_query {
                last_query = app_state.query.clone();
                let mut search = searcher.borrow_mut();
                let results = search.search(&last_query, &entries.borrow(), &frecency.borrow());
                let display_entries: Vec<(String, String)> = results
                    .iter()
                    .map(|r| (r.entry.title.clone(), r.entry.username.clone()))
                    .collect();
                *current_results.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
                app_state.set_entries(display_entries);

                // Clamp selected_index to valid range
                if !app_state.entries.is_empty() && app_state.selected_index >= app_state.entries.len() {
                    app_state.selected_index = app_state.entries.len() - 1;
                }
            }

            // Redraw after processing events
            app_state.request_redraw(&qh);

            // Flush to ensure the frame is sent to compositor
            conn.flush().unwrap();
        }
    }

    // 6. On selection: copy password or username, update frecency, exit
    let final_entry = selected_entry.borrow().clone();
    let should_copy_username = *copy_username.borrow();
    if let Some(entry) = final_entry {
        // Update frecency
        {
            let mut frec = frecency.borrow_mut();
            frec.record_use(&entry.uuid);
            if let Err(e) = frec.save() {
                eprintln!("Warning: Failed to save frecency data: {}", e);
            }
        }

        // Copy password or username to clipboard
        let (value, label) = if should_copy_username {
            (&entry.username, "Username")
        } else {
            (&entry.password, "Password")
        };

        if let Err(e) = copy_with_clear(value, clipboard_timeout) {
            eprintln!("Failed to copy to clipboard: {}", e);
            std::process::exit(1);
        }

        if clipboard_timeout > 0 {
            eprintln!("{} copied for: {} - {} (clears in {}s)", label, entry.title, entry.username, clipboard_timeout);
        } else {
            eprintln!("{} copied for: {} - {}", label, entry.title, entry.username);
        }
    }
}
