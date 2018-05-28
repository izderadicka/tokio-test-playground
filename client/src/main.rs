extern crate tokio;
extern crate futures;
extern crate tokio_io;
#[macro_use]
extern crate lazy_static;

use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::net::SocketAddr;
use tokio::prelude::*;
use tokio::net::TcpStream;
use std::time::{Instant};

const DEFAULT_CONNECTIONS:usize = 100;
const DEFAULT_SERVER:&str = "127.0.0.1:12345";

lazy_static! {
    static ref JOKES: AtomicUsize  = AtomicUsize::new(0);
    static ref BYTES: AtomicUsize = AtomicUsize::new(0);
}

fn main() {

    let count:usize = env::args().nth(1)
    .map(|x| x.parse().unwrap_or(DEFAULT_CONNECTIONS)).unwrap_or(DEFAULT_CONNECTIONS);
    let server = env::args().nth(2)
    .unwrap_or(DEFAULT_SERVER.into());
    let addr:SocketAddr =server.parse().unwrap();
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    let start = Instant::now();
    for _i in 0..count {
        let client = TcpStream::connect(&addr)
        .map_err(|e| eprintln!("Read Error: {:?}",e))
        .and_then(|socket| {
            //tokio::io::write_all(socket, b"hey\n\n")
            //.map_err(|e| eprintln!("Write error: {}",e))
            //.and_then(|(socket, _x)| {
            tokio::io::read_to_end(socket, vec![]).map(|(_, v)| {
                let prev = JOKES.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(v.len(), Ordering::Relaxed);
                println!("Got joke  {}", prev);
                })
                .map_err(|e| eprintln!("Read Error: {:?}",e))
        //})
        });
        rt.spawn(client);
    }

    rt.shutdown_on_idle().wait().unwrap();

    let dur = start.elapsed();

    println!("FINISHED - jokes {}, bytes {}, duration {}.{:03}", 
    JOKES.load(Ordering::Relaxed),
    BYTES.load(Ordering::Relaxed),
    dur.as_secs(),
    dur.subsec_nanos() / 1_000_000
    );
}
