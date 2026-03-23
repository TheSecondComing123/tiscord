mod auth;
mod config;
mod discord;
mod store;

use config::Config;

fn main() {
    let _config = Config::load().expect("failed to load config");
    println!("tiscord");
}
