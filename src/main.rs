mod config;

use config::Config;

fn main() {
    let _config = Config::load().expect("failed to load config");
    println!("tiscord");
}
