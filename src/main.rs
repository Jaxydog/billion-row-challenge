#![warn(clippy::nursery, clippy::todo, clippy::pedantic)]

use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::{BufRead, BufReader, Write},
    sync::Mutex,
    thread::available_parallelism,
};

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
    const DEFAULT_LINES: usize = 1_000_000_000;

    let mut arguments = std::env::args_os().skip(1);
    let path = arguments.next().expect("missing file path");
    let line_count = arguments.next().map_or(DEFAULT_LINES, |s| s.to_string_lossy().parse().unwrap());

    let thread_count = available_parallelism()?.get();
    let lines_per_thread = line_count / thread_count;
    let results = Mutex::new(Vec::with_capacity(thread_count));

    std::thread::scope(|scope| {
        for iteration in 0..thread_count {
            let path = &path;
            let start = iteration * lines_per_thread;
            let range = (start, start + lines_per_thread);
            let results = &results;

            scope.spawn(move || {
                let result = run(path, range);

                results.lock().unwrap().push(result);
            });
        }
    });

    let mut statistics = HashMap::<Box<str>, Statistics>::new();

    for map in results.into_inner().unwrap() {
        for (name, data) in map {
            if let Some(stored) = statistics.get_mut(&name) {
                stored.combine(&data);
            } else {
                statistics.insert(name, data);
            }
        }
    }

    let mut statistics = statistics.into_iter().collect::<Box<[_]>>();
    let mut stdout = std::io::stdout().lock();

    statistics.sort_unstable_by_key(|(k, _)| k.clone());

    stdout.write_all(&[b'{'])?;

    #[allow(clippy::cast_precision_loss)]
    for (index, (name, data)) in statistics.iter().enumerate() {
        let min = data.min;
        let max = data.max;
        let mean = data.sum / data.count as f64;

        write!(&mut stdout, "{name}={min}/{mean:.1}/{max}")?;

        if index < statistics.len() - 1 {
            stdout.write_all(&[b','])?;
        }
    }

    stdout.write_all(&[b'}'])
}

#[allow(clippy::needless_pass_by_value)]
fn run(path: &OsStr, (start, end): (usize, usize)) -> HashMap<Box<str>, Statistics> {
    let file = File::open(path).unwrap();
    let mut reader = BufReader::new(file);
    let mut map = HashMap::<Box<str>, Statistics>::with_capacity(420);
    let mut buffer = String::with_capacity(128);

    for _ in 0..start {
        reader.read_line(&mut buffer).unwrap();
        buffer.clear();
    }

    let mut index = start;

    while index < end && reader.read_line(&mut buffer).is_ok_and(|n| n > 0) {
        buffer.pop();

        if buffer.is_empty() {
            break;
        }

        let (name, value) = buffer.split_once(';').unwrap();
        let name = Box::from(name);
        let value = value.parse().unwrap();

        let statistics = Statistics::new(value);

        if let Some(entry) = map.get_mut(&name) {
            entry.combine(&statistics);
        } else {
            map.insert(name, statistics);
        }

        index += 1;
        buffer.clear();
    }

    map
}
