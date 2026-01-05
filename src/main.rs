mod clipboard;
mod config;
mod keepassxc;
mod search;

use clipboard::copy_with_clear;
use config::Config;
use keepassxc::KeePassXCClient;
use search::{FrecencyData, Searcher};

fn main() {
    println!("kpick - KeePassXC password picker");

    let mut config = Config::load().expect("Failed to load config");

    let mut client = match KeePassXCClient::connect() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Check/create association
    if let Some(ref assoc) = config.association {
        if !client.test_associate(&assoc.id, &assoc.id_key).unwrap_or(false) {
            config.association = None;
        }
    }

    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        let (id, id_key) = client.associate().expect("Association failed");
        config.association = Some(config::Association { id, id_key });
        config.save().expect("Failed to save config");
    }

    // Fetch entries
    let assoc = config.association.as_ref().unwrap();
    let entries = client
        .get_logins(&assoc.id, &assoc.id_key)
        .expect("Failed to get logins");

    // Search
    let frecency = FrecencyData::default();
    let mut searcher = Searcher::new();

    let query = std::env::args().nth(1).unwrap_or_default();
    let results = searcher.search(&query, &entries, &frecency);

    if results.is_empty() {
        println!("No matching entries found");
        return;
    }

    // For now, just copy the first result
    let selected = &results[0];
    println!("Copying password for: {} - {}", selected.entry.name, selected.entry.login);

    if let Err(e) = copy_with_clear(&selected.entry.password, 10) {
        eprintln!("Failed to copy to clipboard: {}", e);
        std::process::exit(1);
    }

    println!("Password copied! (will clear in 10 seconds)");
}
