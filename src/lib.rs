extern crate nix;

use std::thread::spawn;
use std::error::Error;
use std::process::Command;
use std::net::{TcpStream, TcpListener, Shutdown};
use std::io::{BufReader, BufRead, Write};
use nix::unistd::{fork, ForkResult, getpid};
use nix::sys::signal::{SIGTERM, SIGCHLD, kill,
                       sigaction, SigAction,
                       SigHandler::SigIgn, SaFlags, SigSet};

pub enum Mode {
    Simple,
    Forked,
    Threaded,
}

pub struct Config {
    host: String,
    port: u16,
    mode: Mode,
}

impl Config {
    pub fn new(mut args: std::env::Args) -> Result<Config, &'static str> {
        // ignore first argument as it's usually the filename
        args.next();
        let mode: Mode = match args.next() {
            Some(arg) => match arg.as_ref() {
                // it is late so help will be an err, whatever
                "h" => return Err("Help is sily. But you should run with params [MODE] [HOST] [PORT]"),
                "simple" => Mode::Simple,
                "threaded" => Mode::Threaded,
                "forked" => Mode::Forked,
                _ => return Err("not a valid mode of operation! try 'forked', 'threaded' or 'simple'")
            }
            None => return Err("Did not provide any params!")
        };
        let host: String = match args.next() {
            Some(arg) => arg.clone(),
            None => return Err("Did not provide any host!")
        };

        let port: u16 = match args.next() {
            Some(arg) => match arg.parse() {
                Ok(n) => n,
                Err(_) => return Err("Invalid port number.")
            },
            None => return Err("Did not get a port to run on."),
        };
        Ok(Config{host, port, mode})
    }

    pub fn host(&self) -> &str {
        return &self.host;
    }

    pub fn port(&self) -> &u16 {
        return &self.port;
    }
}


/// Deals with a single client.
/// Has absolutely all the complicated, very interesting bussiness logic of this program.
///
/// Will take ownership of a stream representing the TCP socket and will write to and read from it.
/// It would be a shame if this super interesting logic leaked.
fn handle_client(mut stream : TcpStream, ip: String) -> Result<(), Box<dyn Error>> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(&mut stream);
    let mut buffer : String = String::new();

    let welcome = format!("Bienvenido {}\n", ip);
    writer.write(welcome.as_ref())?;
    loop {
        match reader.read_line(&mut buffer)? {
            0 => return Ok(()),
            _ => {
                match buffer.trim() {
                    "usuarios" => {writer.write(b"OK.\n")?;
                        writer.write(&Command::new("who").output()?.stdout)?;
                        writer.write(b"\nFIN.\n")?;
                    }
                    "fecha" => {
                        writer.write(b"OK.\n")?;
                        writer.write(&Command::new("date").output()?.stdout)?;
                        writer.write(b"\nFIN.\n")?;
                    }
                    "salir" => {
                        writer.write(b"ADIOS.\n")?;
                        writer.shutdown(Shutdown::Both)?;
                        return Ok(())
                    }
                    _ => { writer.write(b"ERR.\n")?; }
                }
                buffer.clear();
            },
        }
    }
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    match config.mode {
        Mode::Simple => return run_single_threaded(config),
        Mode::Threaded => return run_with_threads(config),
        Mode::Forked => return run_with_fork(config),
    }
}

/// Runs the server with a single thread.
/// Try connecting with two telnets at the same time, should not work.
fn run_single_threaded(config: Config) -> Result<(), Box<dyn Error>>{
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;
    loop {
        let (stream, addr) = listener.accept()?;
        handle_client(stream, format!("{}", addr.ip()))?;
    }
}

/// Runs using OS's forks!
/// Fun to use ps -aux | grep serveto while you open up new connections.
fn run_with_fork(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;

    // THIS IS FINE.
    unsafe {
        sigaction(SIGCHLD, &SigAction::new(SigIgn, SaFlags::empty(), SigSet::empty()))?;
    }

    loop {
        let (stream, addr) = listener.accept()?;
        match fork() {
            Ok(ForkResult::Parent {child: _ , ..}) => {},
            Ok(ForkResult::Child) => {
                let res = handle_client(stream, format!("{}", addr.ip()));
                kill(getpid(), SIGTERM)?;
                res?
            },
            Err(e) => return Err(Box::new(e))
        }
    }
}

/// Runs with threads.
fn run_with_threads(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;
    loop {
        let (stream, addr) = listener.accept()?;
        spawn( move || {
            if let Err(e) = handle_client(stream, format!("{}", addr.ip())) {
                eprintln!("Thread failed {}", e);
            };
        });
    }
}
