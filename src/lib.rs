extern crate nix;

use nix::sys::signal::{kill, SIGTERM};
use nix::unistd::{fork, getpid, ForkResult};
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::Command;
use std::thread::spawn;

/// Los modos en los que puede correr el servidor
/// son simple, modo fork y modo thread
pub enum Mode {
    Simple,
    Forked,
    Threaded,
}

/// Config struct
/// Una configuracion basica para el servidor
pub struct Config {
    host: String,
    port: u16,
    mode: Mode,
}

/// Metodos para el struct config
impl Config {
    /// Crea una nueva config con los argumentos pasados por terminal
    pub fn new(mut args: std::env::Args) -> Result<Config, &'static str> {
        // ignora el primer argumento, es el nombre del archivo
        args.next();

        // el segundo argumento tiene que ser el modo en el que va a correr el servidor
        let mode: Mode = match args.next() {
            Some(arg) => match arg.as_ref() {
                "h" => {
                    return Err("Help is sily. But you should run with params [MODE] [HOST] [PORT]")
                }
                "simple" => Mode::Simple,
                "threaded" => Mode::Threaded,
                "forked" => Mode::Forked,
                _ => {
                    return Err(
                        "not a valid mode of operation! try 'forked', 'threaded' or 'simple'",
                    )
                }
            },
            None => return Err("Did not provide any params!"),
        };

        // la ip en la que va a correr el servidor
        let host: String = match args.next() {
            Some(arg) => arg.clone(),
            None => return Err("Did not provide any host!"),
        };

        // el puerto donde va a estar escuchando
        let port: u16 = match args.next() {
            Some(arg) => match arg.parse() {
                Ok(n) => n,
                Err(_) => return Err("Invalid port number."),
            },
            None => return Err("Did not get a port to run on."),
        };
        Ok(Config { host, port, mode })
    }

    pub fn host(&self) -> &str {
        return &self.host;
    }

    pub fn port(&self) -> &u16 {
        return &self.port;
    }
}

/// Tiene logica para lidiar con un cliente.
/// Tiene un buffer para escribir y otro para leer del socket tcp.
/// Se le pasa un strem TCP como parametro.
fn handle_client(mut stream: TcpStream, ip: String) -> Result<(), Box<dyn Error>> {
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(&mut stream);
    let mut buffer: String = String::new();

    let welcome = format!("Bienvenido {}\n", ip);
    writer.write(welcome.as_ref())?;

    // hasta que el usuario no escriba "SALIR", el loop no se termina
    loop {
        match reader.read_line(&mut buffer)? {
            0 => return Ok(()),
            _ => {
                match buffer.trim() {
                    "usuarios" => {
                        writer.write(b"OK.\n")?;
                        writer.write(&Command::new("who").output()?.stdout)?;
                        writer.write(b"\nFIN.\n")?;
                    }
                    "fecha" => {
                        writer.write(b"OK.\n")?;
                        writer.write(&Command::new("date").output()?.stdout)?;
                        writer.write(b"\nFIN.\n")?;
                    }
                    "procesos" => {
                        writer.write(b"OK.\n")?;
                        writer.write(&Command::new("ps").output()?.stdout)?;
                        writer.write(b"\nFIN.\n")?;
                    }
                    "salir" => {
                        writer.write(b"ADIOS.\n")?;
                        writer.shutdown(Shutdown::Both)?;
                        return Ok(());
                    }
                    _ => {
                        writer.write(b"ERR.\n")?;
                    }
                }
                buffer.clear();
            }
        }
    }
}

/// Corre un servidor con una config en particular.
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    match config.mode {
        Mode::Simple => return run_single_threaded(config),
        Mode::Threaded => return run_with_threads(config),
        Mode::Forked => return run_with_fork(config),
    }
}

/// Corre el servidor con un solo thread y un solo proceso (ie: secuencialmente)
fn run_single_threaded(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;
    loop {
        let (stream, addr) = listener.accept()?;
        handle_client(stream, format!("{}", addr.ip()))?;
    }
}

/// Corre usando varios procesos: uno por cliente
fn run_with_fork(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;

    loop {
        let (stream, addr) = listener.accept()?;
        // por cada vez que se acepta una conexión, hacé el fork
        match fork() {
            // no necesita actuarse en el padre :)
            Ok(ForkResult::Parent { child: _, .. }) => {}
            // en el hijo, maneja el cliente y después terminá el proceso
            Ok(ForkResult::Child) => {
                let res = handle_client(stream, format!("{}", addr.ip()));
                kill(getpid(), SIGTERM)?;
                res?
            }
            Err(e) => return Err(Box::new(e)),
        }
    }
}

/// Corre con threads!
fn run_with_threads(config: Config) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(format!("{}:{}", config.host(), config.port()))?;
    loop {
        let (stream, addr) = listener.accept()?;
        // por cada conexión aceptada, iniciá un nuevo thread
        spawn(move || {
            if let Err(e) = handle_client(stream, format!("{}", addr.ip())) {
                eprintln!("Thread failed {}", e);
            };
        });
    }
}
