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
use tokio_io::codec::LinesCodec;
use std::time::{Instant};

const DEFAULT_CONNECTIONS:usize = 100;

lazy_static! {
    static ref JOKES: AtomicUsize  = AtomicUsize::new(0);
    static ref LINES: AtomicUsize = AtomicUsize::new(0);
}

fn main() {

    let count:usize = env::args().nth(1)
    .map(|x| x.parse().unwrap_or(DEFAULT_CONNECTIONS)).unwrap_or(DEFAULT_CONNECTIONS);
    let addr:SocketAddr = "127.0.0.1:12345".parse().unwrap();
    let mut rt = tokio::runtime::Runtime::new().unwrap();

    let start = Instant::now();
    for _i in 0..count {
        let client = TcpStream::connect(&addr)
        .and_then(|socket| {
            let stream = socket.framed(LinesCodec::new());
            let task = stream.map(|_l| {
                1
                })
                .fold(0, {
                    |acc,x| futures::future::ok::<_, std::io::Error>(acc+x)
                })
            .and_then(|num_lines| {
                LINES.fetch_add(num_lines, Ordering::Relaxed);
                if num_lines > 0 {
                    let prev = JOKES.fetch_add(1, Ordering::Relaxed);
                    println!("Got joke  {}", prev);
                } else {
                    eprintln!("Got empty joke");
                }
                Ok(())
            });
            
            task
        })
        .map_err(|e| eprintln!("IO Error: {:?}",e));

        rt.spawn(client);
    }

    
    rt.shutdown_on_idle().wait().unwrap();

    let dur = start.elapsed();

    println!("FINISHED - jokes {}, lines {}, duration {}.{:03}", 
    JOKES.load(Ordering::Relaxed),
    LINES.load(Ordering::Relaxed),
    dur.as_secs(),
    dur.subsec_nanos() / 1_000_000
    );
}
