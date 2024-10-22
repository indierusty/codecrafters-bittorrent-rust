use anyhow::{Error, Result};
use core::panic;
use serde_json::{self, json};
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

fn decode_integer(encoded_value: &str) -> Result<serde_json::Value> {
    if encoded_value.starts_with("i") {
        let encoded_integer = encoded_value[1..]
            .chars()
            .take_while(|c| *c != 'e')
            .collect::<String>()
            .parse::<isize>()
            .unwrap();

        return Ok(json!(encoded_integer));
    }

    Err(Error::msg("Failed decoding Integer"))
}

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    match &encoded_value[..1] {
        "i" => decode_integer(encoded_value).unwrap(),
        n if n.parse::<usize>().is_ok() => decode_string(encoded_value).unwrap(),
        _ => panic!(),
    }
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
