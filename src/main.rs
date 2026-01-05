mod config;
mod keepassxc;

use config::Config;
use keepassxc::KeePassXCClient;

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

    // Check if we have an existing association
    if let Some(ref assoc) = config.association {
        match client.test_associate(&assoc.id, &assoc.id_key) {
            Ok(true) => {
                println!("Association valid: {}", assoc.id);
            }
            Ok(false) | Err(_) => {
                println!("Association invalid, need to re-associate");
                config.association = None;
            }
        }
    }

    // Associate if needed
    if config.association.is_none() {
        println!("Associating with KeePassXC (check KeePassXC for prompt)...");
        match client.associate() {
            Ok((id, id_key)) => {
                println!("Associated as: {}", id);
                config.association = Some(config::Association { id, id_key });
                config.save().expect("Failed to save config");
            }
            Err(e) => {
                eprintln!("Association failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Fetch entries
    let assoc = config.association.as_ref().unwrap();
    match client.get_logins(&assoc.id, &assoc.id_key) {
        Ok(entries) => {
            println!("\nFound {} entries:", entries.len());
            for entry in entries.iter().take(10) {
                println!("  {} - {}", entry.name, entry.login);
            }
            if entries.len() > 10 {
                println!("  ... and {} more", entries.len() - 10);
            }
        }
        Err(e) => {
            eprintln!("Failed to get logins: {}", e);
            std::process::exit(1);
        }
    }
}
