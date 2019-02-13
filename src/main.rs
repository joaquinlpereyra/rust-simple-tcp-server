extern crate serveto;

use std::env;
use std::process;
use serveto::Config;

fn main() {
    println!(" --- SERVETO ---");
    println!("La plej simpla TCP-a servo");
    let config = Config::new(env::args()).unwrap_or_else( |err| {
        eprintln!("There was a problem parsing arguments: {}", err);
        process::exit(1);
    });
    println!("Listening on:  {}", config.port());
    if let Err(e) = serveto::run(config) {
        eprintln!("Server had a problem: {}", e);
        process::exit(1);
    }
}
