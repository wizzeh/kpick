mod keepassxc;

use keepassxc::KeePassXCClient;

fn main() {
    println!("kpick - KeePassXC password picker");

    match KeePassXCClient::connect() {
        Ok(client) => {
            println!("Connected to KeePassXC! Client ID: {}", client.client_id());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
