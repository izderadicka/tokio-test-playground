extern crate rand;
extern crate tokio;
extern crate tokio_threadpool;

use rand::Rng;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path};
use std::time::Duration;
use tokio::io;
use tokio::net::TcpListener;
use tokio::prelude::*;

fn prepare_server(
    addr: std::net::SocketAddr,
    idx: Index,
) -> Box<Future<Item = (), Error = io::Error> + Send> {
    println!("Starting at {}", &addr);
    let tcp = TcpListener::bind(&addr).unwrap();

    let server = tcp.incoming().for_each(move |socket| {
        println!("Received connection from {}", socket.peer_addr().unwrap());
        let i = rand::thread_rng().gen_range(0, idx.index.len());
        let (from, to) = idx.index[i];
        println!("Sending joke from lines: {} - {}", from, to);
        
        let joke: Vec<_> = idx
            .lines.iter()
            .skip(from)
            .take(to - from)
            .map(|s| s.to_owned())
            .collect();

        let mut text = joke.join("\n");
        text.push_str("\n");
        let write_future= io::write_all(socket, text)
        .map_err(|e| eprintln!("Write error: {}", e))
        .map(|_| ());

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

struct Index {
    index: Vec<(usize, usize)>,
    lines: Vec<String>
}

fn create_index<P: AsRef<Path>>(f: P) -> Result<Index, std::io::Error> {
    let reader = BufReader::new(File::open(f)?);
    let mut start: Option<usize> = None;
    let mut idx = vec![];
    let mut lines = vec![];
    let mut no = 0;
    for line in reader.lines() {
        match line {
            Ok(l) => {
                if l.starts_with("---") {
                    if let Some(s) = start {
                        //println!("joke from {} to {}", s, no);
                        idx.push((s, no));
                    }
                    start = Some(no)
                } else {
                    let s = l.trim_left();
                    if s.len() > 1 {
                        lines.push(s.to_owned());
                        no+=1;
                    }
                }
                
            }

            Err(e) => eprintln!("Error reading line {}: {}", no, e),
        }
    }
    Ok(Index {
        index:idx,
        lines
    })
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
    //let idx = Arc::new(idx);

    let addr = "127.0.0.1:12345".parse().unwrap();
    let server = prepare_server(addr, idx);

    let mut rt = create_runtime().unwrap();

    rt.spawn(server.map_err(|e| eprintln!("Server error {}", e)));
    rt.shutdown_on_idle().wait().unwrap()
}
