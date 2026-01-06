mod clipboard;
mod config;
mod database;
mod search;
mod ui;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use clipboard::copy_with_clear;
use config::Config;
use database::{open_database, prompt_password, DatabaseError, Entry};
use search::Searcher;
use ui::AppState;
use wayland_client::Connection;

fn main() {
    // 1. Load config
    let config = Rc::new(RefCell::new(Config::load().expect("Failed to load config")));

    // 2. Open database
    // For now, use test.kdbx in current directory
    let db_path = PathBuf::from("test.kdbx");

    let password = match prompt_password("Master password: ") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to read password: {}", e);
            std::process::exit(1);
        }
    };

    let entries = match open_database(&db_path, &password) {
        Ok(e) => e,
        Err(DatabaseError::NotFound(path)) => {
            eprintln!("Database not found: {}", path);
            std::process::exit(1);
        }
        Err(DatabaseError::InvalidPassword) => {
            eprintln!("Invalid password.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    if entries.is_empty() {
        eprintln!("No password entries found.");
        std::process::exit(0);
    }

    // 3. Set up UI with entries
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let colors = config.borrow().colors.to_rgb();
    let (mut app_state, mut event_queue) = AppState::new(&conn, colors);

    // Store entries for searching
    let entries = Rc::new(entries);
    let searcher = Rc::new(RefCell::new(Searcher::new()));

    // Current filtered results - will be updated as user types
    let current_results: Rc<RefCell<Vec<Entry>>> = Rc::new(RefCell::new(Vec::new()));

    // Initialize with all entries sorted by frecency
    {
        let frecency = &config.borrow().frecency;
        let mut search = searcher.borrow_mut();
        let results = search.search("", &entries, frecency);
        let display_entries: Vec<(String, String)> = results
            .iter()
            .map(|r| (r.entry.title.clone(), r.entry.username.clone()))
            .collect();
        *current_results.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
        app_state.set_entries(display_entries);
    }

    // 4. Set callbacks

    // Selected entry tracking - only set when user confirms with Enter
    let selected_entry: Rc<RefCell<Option<Entry>>> = Rc::new(RefCell::new(None));

    // On select (Enter): store the selected entry
    {
        let selected_entry = selected_entry.clone();
        let current_results = current_results.clone();
        app_state.on_select = Some(Box::new(move |index| {
            let results = current_results.borrow();
            if index < results.len() {
                *selected_entry.borrow_mut() = Some(results[index].clone());
            }
        }));
    }

    // On escape: just exit (running = false is already set by the handler)
    app_state.on_escape = Some(Box::new(|| {
        // Nothing extra needed, running is set to false in the handler
    }));

    // 5. Run event loop
    {
        let qh = event_queue.handle();
        let mut last_query = String::new();

        while app_state.running {
            // Flush pending requests
            conn.flush().unwrap();

            // Blocking dispatch - waits for events
            event_queue.blocking_dispatch(&mut app_state).unwrap();

            // Check if query changed and update entries
            if app_state.query != last_query {
                last_query = app_state.query.clone();
                let frecency = &config.borrow().frecency;
                let mut search = searcher.borrow_mut();
                let results = search.search(&last_query, &entries, frecency);
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
        }
    }

    // 6. On selection: copy password, update frecency, exit
    let final_entry = selected_entry.borrow().clone();
    if let Some(entry) = final_entry {
        // Update frecency
        {
            let mut cfg = config.borrow_mut();
            cfg.frecency.record_use(&entry.uuid);
            if let Err(e) = cfg.save() {
                eprintln!("Warning: Failed to save frecency data: {}", e);
            }
        }

        // Copy password to clipboard
        if let Err(e) = copy_with_clear(&entry.password, 10) {
            eprintln!("Failed to copy to clipboard: {}", e);
            std::process::exit(1);
        }

        eprintln!("Password copied for: {} - {} (clears in 10s)", entry.title, entry.username);
    }
}
