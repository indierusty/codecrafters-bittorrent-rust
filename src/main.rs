use anyhow::{Error, Result};
use serde_json::{self, json};
use std::env;

// Available if you need it!
// use serde_bencode

fn decode_string(encoded_value: &str) -> Result<(Option<serde_json::Value>, &str)> {
    if let Some((len, str)) = encoded_value.split_once(":") {
        let length = len.parse::<usize>().unwrap();
        let (parsed, rest) = str.split_at(length);
        return Ok((Some(serde_json::Value::String(parsed.to_string())), rest));
    }
    Err(Error::msg("Failed decoding string"))
}

fn decode_integer(encoded_value: &str) -> Result<(Option<serde_json::Value>, &str)> {
    if encoded_value.starts_with("i") {
        let encoded_integer = encoded_value[1..]
            .chars()
            .take_while(|c| *c != 'e')
            .collect::<String>()
            .parse::<isize>()
            .unwrap();

        return Ok((
            Some(json!(encoded_integer)),
            encoded_value.split_once('e').unwrap().1,
        ));
    }

    Err(Error::msg("Failed decoding Integer"))
}

fn decode_list(encoded_value: &str) -> Result<(Option<serde_json::Value>, &str)> {
    if encoded_value.starts_with("l") {
        let mut values = vec![];
        let mut rest = &encoded_value[1..];
        loop {
            dbg!(rest);
            let (value, rest_inner) = decode_bencoded_value(rest);
            if let Some(v) = value {
                values.push(v);
            }
            rest = rest_inner;
            if rest.starts_with('e') {
                break;
            }
        }

        return Ok((Some(json!(values)), &rest[1..]));
    }
    Err(Error::msg("Failed decoding List"))
}

#[allow(dead_code)]
fn decode_bencoded_value(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    match &encoded_value[..1] {
        "i" => decode_integer(encoded_value).unwrap(),
        "l" => decode_list(encoded_value).unwrap(),
        n if n.parse::<usize>().is_ok() => decode_string(encoded_value).unwrap(),
        _ => (None, encoded_value),
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        if let Some(value) = decoded_value.0 {
            println!("{}", value.to_string());
        }
    } else {
        println!("unknown command: {}", args[1])
    }
}
