extern crate rand;
extern crate tokio;
extern crate tokio_threadpool;
#[macro_use]
extern crate futures;
extern crate tokio_io;

use rand::Rng;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path};
use std::time::Duration;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use futures::stream::iter_ok;
use futures::sink::{Sink};

static mut GLOBAL_INDEX: Option<Index> = None;

fn get_index() -> &'static Index {
    unsafe {
        GLOBAL_INDEX.as_ref().unwrap()
    }
}

fn prepare_server(
    addr: std::net::SocketAddr
) -> Box<Future<Item = (), Error = io::Error> + Send> {
    println!("Starting at {}", &addr);
    let tcp = TcpListener::bind(&addr).unwrap();

    let server = tcp.incoming().for_each(move |socket| {
        let idx = get_index();
        println!("Received connection from {}", socket.peer_addr().unwrap());
        let i = rand::thread_rng().gen_range(0, idx.index.len());
        let (from, to) = idx.index[i];
        println!("Sending joke from lines: {} - {}", from, to);
        
        let joke_iter= idx
            .lines.iter()
            .skip(from)
            .take(to - from)
            ;

        let stream = iter_ok(joke_iter);
        let sink = LineSink::new(socket);
        let write_future= stream.forward(sink)
        .map_err(|e:io::Error| eprintln!("Write error: {}", e))
        .map(|_| ());

        tokio::spawn(write_future);

        Ok(())
    });

    Box::new(server)
}

struct LineSink {
    socket: TcpStream,
    line: Option<&'static [u8]>,
    line_pos: usize,
    next_line: usize,
    lines: Vec<&'static [u8]>
}

impl LineSink {
    fn new(socket: TcpStream) -> Self {
        LineSink {
            socket:socket,
            line: None,
            line_pos: 0,
            next_line: 0,
            lines: Vec::with_capacity(20)

        }
    }
}

impl Sink for LineSink {
    type SinkItem = &'static String;
    type SinkError = io::Error;
    fn start_send(&mut self, item: Self::SinkItem) -> Result<AsyncSink<Self::SinkItem>, Self::SinkError> {
        println!("SINK - start send line: {}", item);
        self.lines.push(item.as_bytes());
        self.line_pos = 0;
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Self::SinkError> {
         println!("SINK - calling poll_complete with line_num = {}/{}", self.next_line, self.lines.len());
        while self.next_line <= self.lines.len() {
            match self.line {
                Some(line) => {
                    while self.line_pos < line.len() {
                        let n = try_ready!( self.socket.poll_write(&line[self.line_pos..]));
                        println!("send {} bytes of line {}", n, std::str::from_utf8(line).unwrap());
                        if n == 0 {
                            return Err(io::Error::new(io::ErrorKind::Other, "write 0 bytes"));
                            }
                        self.line_pos += n
                    }
                    let _n = try_ready!(self.socket.poll_write(b"\n"));
                    self.line = None;
                    self.line_pos = 0
                },
                None => {
                    self.line = Some(self.lines[self.next_line]);
                     println!("Moved to line {}", self.next_line);
                    self.next_line+=1;
                }
            }
        }
        Ok(Async::Ready(()))
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

struct Index {
    index: Vec<(usize, usize)>,
    lines: Vec<String>
}

fn create_index<P: AsRef<Path>>(f: P) -> Result<(), std::io::Error> {
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
    // It's ok to use unsafe, as initialization happens in main thread before any other thread starts
    // later threads only read from it
    unsafe {
    GLOBAL_INDEX = Some(Index {
        index:idx,
        lines
    });
    }
    Ok(())
}

fn main() {
    let jokes_file = match env::args().nth(1) {
        Some(s) => s,
        None => {
            eprintln!("text file is required as first argument");
            return;
        }
    };
    create_index(&jokes_file).unwrap();

    let addr = "127.0.0.1:12345".parse().unwrap();
    let server = prepare_server(addr);

    let mut rt = create_runtime().unwrap();

    rt.spawn(server.map_err(|e| eprintln!("Server error {}", e)));
    rt.shutdown_on_idle().wait().unwrap()
}
