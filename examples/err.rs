use anyhow::Context;

use std::{fs, mem::size_of};

use thiserror::Error;
#[derive(Error, Debug)]
pub enum MyError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(#[from] std::num::ParseIntError),
    #[error("serialization json error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Error: {0:?}")]
    BigError(Box<BigError>),
    #[error("Custom error: {0}")]
    Custom(String),
}

#[allow(unused)]
#[derive(Debug)]
pub struct BigError {
    a: String,
    b: Vec<String>,
    c: [u8; 64],
    d: u64,
}

fn main() -> Result<(), anyhow::Error> {
    println!("size of MyError is {}", size_of::<MyError>());
    println!(
        "size of Box<dyn std::error::Error> is {}",
        size_of::<Box<dyn std::error::Error>>()
    );
    println!("size of std::io::Error is {}", size_of::<std::io::Error>());
    println!(
        "size of std::num::ParseIntError is {}",
        size_of::<std::num::ParseIntError>()
    );
    println!(
        "size of serde_json::Error is {}",
        size_of::<serde_json::Error>()
    );
    println!("size of String is {}", size_of::<String>());
    let filename = "non-existent-file.txt";
    let _fd = fs::File::open(filename).with_context(|| format!("Can't open file: {}", filename))?;
    fail_with_error()?;
    Ok(())
}

fn fail_with_error() -> Result<(), MyError> {
    Err(MyError::Custom("This is a custom error".to_string()))
}
