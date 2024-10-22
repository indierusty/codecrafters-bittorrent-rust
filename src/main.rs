use anyhow::{Error, Result};
use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

fn decode_string(encoded_value: &str) -> Result<serde_json::Value> {
    if let Some((len, str)) = encoded_value.split_once(":") {
        let length = len.parse::<usize>().unwrap();
        let (parsed, _rest) = str.split_at(length);
        return Ok(serde_json::Value::String(parsed.to_string()));
    }
    Err(Error::msg("Failed decoding string"))
}

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    decode_string(encoded_value).unwrap()
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
