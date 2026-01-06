mod clipboard;
mod config;
mod keepassxc;
mod search;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use clipboard::copy_with_clear;
use config::Config;
use keepassxc::{ClientError, KeePassXCClient, LoginEntry};
use search::Searcher;
use ui::AppState;
use wayland_client::Connection;

/// Format a user-friendly error message for KeePassXC errors
fn format_client_error(e: &ClientError) -> String {
    match e {
        ClientError::NotRunning => "KeePassXC is not running. Please start it first.".to_string(),
        ClientError::DatabaseLocked => "Please unlock your KeePassXC database.".to_string(),
        ClientError::Io(io_err) if io_err.kind() == std::io::ErrorKind::ConnectionRefused => {
            "KeePassXC is not running. Please start it first.".to_string()
        }
        _ => format!("KeePassXC error: {}", e),
    }
}

fn main() {
    // 1. Load config and connect to KeePassXC
    let config = Rc::new(RefCell::new(Config::load().expect("Failed to load config")));

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", format_client_error(&e));
            std::process::exit(1);
        }
    };

    // 2. Handle association
    {
        let mut cfg = config.borrow_mut();
        if let Some(ref assoc) = cfg.association {
            match client.test_associate(&assoc.id, &assoc.id_key) {
                Ok(true) => {}
                Ok(false) => cfg.association = None,
                Err(ClientError::DatabaseLocked) => {
                    eprintln!("Please unlock your KeePassXC database.");
                    std::process::exit(1);
                }
                Err(_) => cfg.association = None,
            }
        }

        if cfg.association.is_none() {
            eprintln!("Associating with KeePassXC (check KeePassXC for prompt)...");
            match client.associate() {
                Ok((id, id_key)) => {
                    cfg.association = Some(config::Association { id, id_key });
                    if let Err(e) = cfg.save() {
                        eprintln!("Warning: Failed to save association: {}", e);
                    }
                }
                Err(ClientError::DatabaseLocked) => {
                    eprintln!("Please unlock your KeePassXC database.");
                    std::process::exit(1);
                }
                Err(e) => {
                    // User may have cancelled the association prompt
                    eprintln!("Association cancelled or failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    // 3. Fetch all entries
    let assoc = config.borrow().association.clone().unwrap();
    let entries = match client.get_logins(&assoc.id, &assoc.id_key) {
        Ok(e) => e,
        Err(ClientError::DatabaseLocked) => {
            eprintln!("Please unlock your KeePassXC database.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{}", format_client_error(&e));
            std::process::exit(1);
        }
    };

    if entries.is_empty() {
        eprintln!("No password entries found.");
        std::process::exit(0);
    }

    // 4. Set up UI with entries
    let conn = Connection::connect_to_env().expect("Failed to connect to Wayland");
    let (mut app_state, mut event_queue) = AppState::new(&conn);

    // Store entries for searching
    let entries = Rc::new(entries);
    let searcher = Rc::new(RefCell::new(Searcher::new()));

    // Current filtered results - will be updated as user types
    let current_results: Rc<RefCell<Vec<LoginEntry>>> = Rc::new(RefCell::new(Vec::new()));

    // Initialize with all entries sorted by frecency
    {
        let frecency = &config.borrow().frecency;
        let mut search = searcher.borrow_mut();
        let results = search.search("", &entries, frecency);
        let display_entries: Vec<(String, String)> = results
            .iter()
            .map(|r| (r.entry.name.clone(), r.entry.login.clone()))
            .collect();
        *current_results.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
        app_state.set_entries(display_entries);
    }

    // 5. Set callbacks

    // On query change: re-filter entries
    let entries_for_query = entries.clone();
    let config_for_query = config.clone();
    let searcher_for_query = searcher.clone();
    let results_for_query = current_results.clone();
    app_state.on_query_change = Some(Box::new(move |query: &str| {
        let frecency = &config_for_query.borrow().frecency;
        let mut search = searcher_for_query.borrow_mut();
        let results = search.search(query, &entries_for_query, frecency);
        *results_for_query.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
        // Note: The UI will be updated in the main run loop after this callback
    }));

    // Selected entry tracking for after the loop
    let selected_entry: Rc<RefCell<Option<LoginEntry>>> = Rc::new(RefCell::new(None));

    // On select: store the selected entry
    let results_for_select = current_results.clone();
    let selected_for_select = selected_entry.clone();
    app_state.on_select = Some(Box::new(move |index: usize| {
        let results = results_for_select.borrow();
        if index < results.len() {
            *selected_for_select.borrow_mut() = Some(results[index].clone());
        }
    }));

    // On escape: just exit (running = false is already set by the handler)
    app_state.on_escape = Some(Box::new(|| {
        // Nothing extra needed, running is set to false in the handler
    }));

    // 6. Run event loop
    // We need to update entries display when query changes
    // Since callbacks can't directly modify app_state, we need to handle this differently
    // Let's store a flag to track if we need to update

    // Actually, let's restructure: the on_query_change updates current_results,
    // and after each event dispatch we update the UI entries from current_results

    // Clear the callback since we'll handle updates differently
    app_state.on_query_change = None;

    // Run the event loop manually to handle entry updates
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
                    .map(|r| (r.entry.name.clone(), r.entry.login.clone()))
                    .collect();
                *current_results.borrow_mut() = results.iter().map(|r| r.entry.clone()).collect();
                app_state.set_entries(display_entries);

                // Clamp selected_index to valid range
                if !app_state.entries.is_empty() && app_state.selected_index >= app_state.entries.len() {
                    app_state.selected_index = app_state.entries.len() - 1;
                }
            }

            // Check if selection was made via callback
            {
                let results = current_results.borrow();
                if app_state.selected_index < results.len() {
                    // Store current selection for potential use
                    *selected_entry.borrow_mut() = Some(results[app_state.selected_index].clone());
                }
            }

            // Redraw after processing events
            app_state.request_redraw(&qh);
        }
    }

    // 7. On selection: copy password, update frecency, exit
    // Clone the entry to avoid borrow lifetime issues
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

        eprintln!("Password copied for: {} - {} (clears in 10s)", entry.name, entry.login);
    }
}
