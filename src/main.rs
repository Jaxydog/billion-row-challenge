#![deny(clippy::expect_used, clippy::unwrap_used)]
#![warn(clippy::nursery, clippy::todo, clippy::pedantic)]

use std::{
    fs::File,
    io::{BufRead, BufReader},
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

pub enum Message {
    Next { name: Box<str>, value: f64 },
    Done,
}

fn main() -> std::io::Result<()> {
    let Some(path) = std::env::args_os().nth(1) else {
        panic!("expected a file path");
    };

    let file = File::open(path)?;
    let cursor = Cursor::new(&file, (0, 10));

    for value in cursor {
        println!("{} => {}", value.0, value.1);
    }

    Ok(())
}
