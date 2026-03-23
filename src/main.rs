mod auth;
mod config;
mod discord;

use config::Config;

fn main() {
    let _config = Config::load().expect("failed to load config");
    println!("tiscord");
}
