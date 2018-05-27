#[macro_use]
extern crate futures;
extern crate rand;
extern crate tokio;
extern crate tokio_threadpool;

use futures::future::{err, poll_fn, Future};
use futures::stream::{iter_ok, Stream};
use futures::Async;
use rand::Rng;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
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
        let lines_future = poll_fn(move || {
            blocking(|| {
                let i = rand::thread_rng().gen_range(0, idx.len());
                let (from, to) = idx[i];
                println!("Sending joke from lines: {} - {}", from, to);
                let reader = BufReader::new(File::open(&file_name).unwrap());
                let joke_iter = reader
                    .lines()
                    .skip(from)
                    .take(to - from)
                    .filter(|r| r.is_ok())
                    .map(|s| s.unwrap())
                    .filter_map(|l| {
                        let s = l.trim_left();
                        if s.len() > 0 {
                            let mut l = s.to_owned();
                            l.push_str("\n");
                            Some(l)
                        } else {
                            None
                        }
                    });

                iter_ok::<_, ()>(joke_iter)
            })
        });

        let write_future = lines_future
            .map_err(|_| eprintln!("Blocking error"))
            .and_then(|lines_stream| Sender::new(Box::new(lines_stream), socket));
        // let write_future = work_future
        //     .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "Blocking Error"))
        //     .and_then(|text| {
        //         //println!("Joke is {}", text);
        //         io::write_all(socket, text)
        //     })
        //     .then(|res| {
        //         println!("Written joke -result is Ok {:?}", res.is_ok());
        //         Ok(())
        //     });

        tokio::spawn(write_future);

        Ok(())
    });

    Box::new(server)
}

type MyStream = Box<Stream<Item = String, Error = ()> + Send>;
struct Sender {
    stream: MyStream,
    socket: TcpStream,
    buf: Vec<u8>,
    pos: usize,
}

impl Sender {
    fn new(stream: MyStream, socket: TcpStream) -> Self {
        Sender {
            stream: stream,
            socket: socket,
            buf: vec![],
            pos: 0,
        }
    }

    fn write(&mut self) -> futures::Poll<usize, ()> {
        while self.pos < self.buf.len() {
            match self.socket.poll_write(&self.buf[self.pos..]) {
                Err(e) => {
                    eprintln!("Error writing to socket: {}", e);
                    return Err(());
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Ok(Async::Ready(0)) => {
                    eprintln!("Error write 0 bytes");
                    return Err(());
                }
                Ok(Async::Ready(n)) => self.pos += n,
            };
        }
        Ok(Async::Ready(self.pos))
    }
}

impl Future for Sender {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        // write remainder of previous line
        try_ready!(self.write());
        while let Async::Ready(x) = self.stream.poll()? {
            match x {
                Some(l) => {
                    self.buf = l.into_bytes();
                    self.pos = 0;
                    // write what we can
                    try_ready!(self.write());
                }
                None => return Ok(Async::Ready(())),
            }
        }
        Ok(Async::NotReady)
    }
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
