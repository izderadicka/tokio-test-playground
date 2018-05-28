extern crate futures;
extern crate rand;
extern crate tokio;
extern crate tokio_threadpool;
extern crate tokio_io;

use futures::future::{err, poll_fn, Future};
use futures::stream::{Stream};
use futures::sync::mpsc::{unbounded,};
use rand::Rng;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener};
use tokio_io::codec::LinesCodec;
use tokio::prelude::*;
use tokio_threadpool::blocking;

fn prepare_server(
    addr: std::net::SocketAddr,
    file_name: PathBuf,
    idx: Arc<Index>,
) -> Box<Future<Item = (), Error = io::Error> + Send> {
    println!("Starting at {}", &addr);
    let tcp = match TcpListener::bind(&addr) {
        Ok(t) => t,
        Err(e) => return Box::new(err(e)),
    };

    let server = tcp.incoming().for_each(move |socket| {
        println!("Received connection from {}", socket.peer_addr().unwrap());
        let file_name = file_name.clone();
        let idx = idx.clone();
        let (tx,rx) = unbounded::<String>();
        let joker = poll_fn(move || {
            blocking(|| {
                let i = rand::thread_rng().gen_range(0, idx.len());
                let (from, to) = idx[i];
                println!("Sending joke from lines: {} - {}", from, to);
                let reader = BufReader::new(File::open(&file_name).unwrap());
                reader
                    .lines()
                    .skip(from)
                    .take(to - from)
                    .filter(|r| r.is_ok())
                    .map(|s| s.unwrap())
                    .filter_map(|l| {
                        let s = l.trim_left();
                        if s.len() > 0 {
                            Some(s.to_owned())
                        } else {
                            None
                        }
                    })
                    .for_each(|l| tx.unbounded_send(l).unwrap())
            })
        })
        .map_err(|e| eprintln!("Blocking error: {}", e));
        tokio::spawn(joker);

        let framed_socket = socket.framed(LinesCodec::new())
        .sink_map_err(|e| eprintln!("Write error {}", e));
        let write_future = rx.forward(framed_socket).
            map(|_| ());
        tokio::spawn(write_future);

        Ok(())
    });

    Box::new(server)
}

fn create_runtime() -> Result<tokio::runtime::Runtime, io::Error> {
    let mut tp_builder = tokio_threadpool::Builder::new();
    tp_builder
        .name_prefix("ttest-worker-")
        .pool_size(8)
        .keep_alive(Some(Duration::from_secs(60)));

    tokio::runtime::Builder::new()
        .threadpool_builder(tp_builder)
        .build()
}

type Index = Vec<(usize, usize)>;

fn create_index<P: AsRef<Path>>(f: P) -> Result<Index, std::io::Error> {
    let reader = BufReader::new(File::open(f)?);
    let mut start: Option<usize> = None;
    let mut idx = vec![];
    for (no, line) in reader.lines().enumerate() {
        match line {
            Ok(l) => {
                if l.starts_with("---") {
                    if let Some(s) = start {
                        //println!("joke from {} to {}", s, no);
                        idx.push((s, no));
                    }
                    start = Some(no + 1)
                }
            }

            Err(e) => eprintln!("Error reading line {}: {}", no, e),
        }
    }
    Ok(idx)
}

fn main() {
    let jokes_file = match env::args().nth(1) {
        Some(s) => s,
        None => {
            eprintln!("text file is required as first argument");
            return;
        }
    };
    let idx = create_index(&jokes_file).unwrap();
    let idx = Arc::new(idx);

    let addr = "127.0.0.1:12345".parse().unwrap();
    let server = prepare_server(addr, jokes_file.into(), idx);

    let mut rt = create_runtime().unwrap();

    rt.spawn(server.map_err(|e| eprintln!("Server error {}", e)));
    rt.shutdown_on_idle().wait().unwrap()
}
