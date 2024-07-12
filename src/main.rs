#![deny(clippy::expect_used, clippy::unwrap_used)]
#![warn(clippy::nursery, clippy::todo, clippy::pedantic)]

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    sync::{mpsc::Sender, Arc},
    thread::available_parallelism,
};

pub struct Cursor<'fl> {
    pub read: BufReader<&'fl File>,
    pub range: (usize, usize),
    pub index: usize,
}

impl<'fl> Cursor<'fl> {
    #[must_use]
    pub fn new(file: &'fl File, range: (usize, usize)) -> Self {
        let mut read = BufReader::new(file);
        let mut index = 0;
        let mut buffer = String::with_capacity(128);

        while index < range.0 {
            read.read_line(&mut buffer).ok();

            index += 1;
        }

        Self { read, range, index }
    }

    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.range.0 + self.index >= self.range.1
    }
}

impl<'fl> Iterator for Cursor<'fl> {
    type Item = (Box<str>, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_done() {
            return None;
        }

        self.index += 1;

        let mut string = String::with_capacity(128);

        self.read.read_line(&mut string).ok()?;

        let (name, value) = string.split_once(';')?;
        let value = value.trim().parse().ok()?;

        Some((name.into(), value))
    }
}

#[derive(Debug)]
pub struct Statistics {
    pub min: f64,
    pub max: f64,
    pub sum: f64,
    pub count: usize,
}

impl Statistics {
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self { min: value, max: value, sum: value, count: 1 }
    }

    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn average(&self) -> f64 {
        self.sum / self.count as f64
    }

    pub fn combine(&mut self, other: &Self) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.sum += other.sum;
        self.count += other.count;
    }
}

#[allow(clippy::unwrap_used)]
fn main() -> std::io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);

    let Some(path) = arguments.next() else {
        panic!("expected a file path");
    };
    let lines = arguments.next().map_or(1_000_000_000_usize, |s| s.to_string_lossy().parse().unwrap());

    let threads = available_parallelism()?.get();
    let lines_per_thread = lines / threads;

    let file = Arc::new(File::open(path)?);
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut thread_pool = Vec::with_capacity(threads);

    for iteration in 0..threads {
        let file = Arc::clone(&file);
        let start = iteration * lines_per_thread;
        let range = (start, start + lines_per_thread);
        let sender = sender.clone();

        thread_pool.push(std::thread::spawn(move || process(file, range, sender)));
    }

    drop(sender);

    let mut finished = 0;
    let mut finalized = HashMap::<_, Statistics>::new();

    loop {
        let map = receiver.recv().unwrap();

        for (name, statistics) in map {
            if let Some(entry) = finalized.get_mut(&name) {
                entry.combine(&statistics);
            } else {
                finalized.insert(name, statistics);
            }
        }

        finished += 1;

        if finished >= threads {
            break;
        }
    }

    for handle in thread_pool {
        handle.join().unwrap();
    }

    println!("{finalized:#?}");

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn process(file: Arc<File>, range: (usize, usize), sender: Sender<HashMap<Box<str>, Statistics>>) {
    let cursor = Cursor::new(&file, range);
    let mut map = HashMap::<Box<str>, Statistics>::with_capacity(420);

    for (name, value) in cursor {
        let statistics = dbg!(Statistics::new(value));

        if let Some(entry) = map.get_mut(&name) {
            entry.combine(&statistics);
        } else {
            map.insert(name, Statistics::new(value));
        }
    }

    #[allow(clippy::unwrap_used)]
    sender.send(map).unwrap();
}
