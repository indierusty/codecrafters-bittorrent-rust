#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::{Error, Result};
use serde_json::{self, json};
use std::{collections::HashMap, env, fs};

// Available if you need it!
// use serde_bencode

fn decode_string(mut encoded_value: &[u8]) -> Result<(Option<Value>, &[u8])> {
    let mut len: usize = 0;
    let mut index = 0;

    while encoded_value[index].is_ascii_digit() && encoded_value[index] != b':' {
        let value = encoded_value[index] - b'0';
        len *= 10;
        len += value as usize;
        index += 1;
    }
    // len /= 10;
    index += 1; // skip b':'
    encoded_value = &encoded_value[index..];

    let string = encoded_value[..len].to_vec();
    Ok((Some(Value::String(string)), &encoded_value[len..]))
}

fn decode_integer(encoded_value: &[u8]) -> Result<(Option<Value>, &[u8])> {
    if encoded_value[0] == b'i' {
        // b'i'
        let mut len = 1;
        let integer = encoded_value[1..]
            .iter()
            .take_while(|c| {
                len += 1;
                **c != b'e'
            })
            .map(|c| *c as char)
            .collect::<String>()
            .parse::<isize>()
            .unwrap();

        let encoded_value = &encoded_value[len..];

        return Ok((Some(Value::Integer(integer)), encoded_value));
    }

    Err(Error::msg("Failed decoding Integer"))
}

fn decode_list(encoded_value: &[u8]) -> Result<(Option<Value>, &[u8])> {
    if encoded_value[0] == b'l' {
        let mut values = vec![];
        let mut rest = &encoded_value[1..];
        loop {
            let (value, rest_inner) = decode_bencoded_value(rest);
            if let Some(v) = value {
                values.push(v);
            }
            rest = rest_inner;
            if rest[0] == b'e' {
                break;
            }
        }

        return Ok((Some(Value::Array(values)), &rest[1..]));
    }
    Err(Error::msg("Failed decoding List"))
}

fn decode_dict(encoded_value: &[u8]) -> Result<(Option<Value>, &[u8])> {
    if encoded_value[0] == b'd' {
        let mut values = HashMap::new();
        let mut rest = &encoded_value[1..];
        loop {
            if rest[0] == b'e' {
                break;
            }
            let (key, rest_inner) = decode_string(rest)?;
            let (value, rest_inner) = decode_bencoded_value(rest_inner);
            if let (Some(k), Some(v)) = (key, value) {
                // NOTE: key must be string in bencoded value
                if let Value::String(s) = k {
                    values.insert(s, v);
                }
            }
            rest = rest_inner;
        }

        return Ok((Some(Value::Dict(values)), &rest[1..]));
    }
    Err(Error::msg("Failed decoding Dict"))
}

fn decode_bencoded_value(encoded_value: &[u8]) -> (Option<Value>, &[u8]) {
    match encoded_value[0] {
        b'i' => decode_integer(encoded_value).unwrap(),
        b'l' => decode_list(encoded_value).unwrap(),
        b'd' => decode_dict(encoded_value).unwrap(),
        c if c.is_ascii_digit() => decode_string(encoded_value).unwrap(),
        _ => (None, encoded_value),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Value {
    String(Vec<u8>),
    Integer(isize),
    Array(Vec<Value>),
    Dict(HashMap<Vec<u8>, Value>),
}

impl Value {
    fn to_json(&self) -> serde_json::Value {
        match self {
            Value::String(s) => {
                json!(s.iter().map(|c| *c as char).collect::<String>())
            }
            Value::Integer(i) => json!(i),
            Value::Array(a) => json!(a
                .iter()
                .map(|ai| ai.to_json())
                .collect::<Vec<serde_json::Value>>()),
            Value::Dict(d) => {
                json!(d.iter().fold(serde_json::Map::new(), |mut acc, n| {
                    acc.insert(
                        n.0.iter().map(|c| *c as char).collect::<String>(),
                        n.1.to_json(),
                    );
                    acc
                }))
            }
        }
    }
}

#[derive(Debug)]
struct MetaInfo {
    announce: String,
    info: Info,
}

impl MetaInfo {
    fn from_json(mut json: serde_json::Value) -> Result<Self> {
        let announce: String = if let serde_json::Value::String(a) = json["announce"].take() {
            a
        } else {
            return Err(Error::msg("cannot parse announce from bencode value"));
        };

        let info = Info::from_json(json["info"].take()).expect("parse MetaInfo info");
        Ok(Self { announce, info })
    }
}

#[derive(Debug)]
struct Info {
    // size of the file in bytes, for single-file torrents
    length: usize,
    // suggested name to save a file
    name: String,
    // number of bytes in each piece
    piece_length: usize,
    // concatenated SHA-1 hashes of each piece
    pieces: String,
}

impl Info {
    fn from_json(mut json: serde_json::Value) -> Result<Self> {
        let length: usize = if let serde_json::Value::Number(a) = json["length"].take() {
            a.to_string().parse().unwrap()
        } else {
            return Err(Error::msg("cannot parse Info length from bencode value"));
        };

        let name: String = if let serde_json::Value::String(a) = json["name"].take() {
            a
        } else {
            return Err(Error::msg("cannot parse Infor name from bencode value"));
        };

        let piece_length: usize = if let serde_json::Value::Number(a) = json["piece length"].take()
        {
            a.to_string().parse().unwrap()
        } else {
            return Err(Error::msg(
                "cannot parse Info piece length from bencode value",
            ));
        };

        let pieces: String = if let serde_json::Value::String(a) = json["pieces"].take() {
            a
        } else {
            return Err(Error::msg("cannot parse Info peices from bencode value"));
        };

        Ok(Self {
            length,
            name,
            piece_length,
            pieces,
        })
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let mut args = env::args().skip(1);

    match args.next().expect("command").as_str() {
        "decode" => {
            let encoded_value = args.next().expect("encoded value");
            let decoded_value = decode_bencoded_value(&encoded_value.as_bytes());
            if let Some(value) = decoded_value.0 {
                println!("{}", value.to_json());
            }
        }
        "info" => {
            let file_path = args.next().expect("path to torrent file");
            let file = fs::read(file_path).expect("read file");
            // for bytes in file {
            //     print!("{}", bytes as char);
            // }
            let decoded_value = decode_bencoded_value(&file)
                .0
                .expect("decode bencode value");

            dbg!(decoded_value.to_json());
            let decoded_value = decoded_value.to_json();
            // let metainfo = MetaInfo::from_json(decoded_value).unwrap();
            println!(
                "Tracker URL: {}",
                decoded_value["announce"].as_str().unwrap()
            );
            println!("Length: {}", decoded_value["info"]["length"]);
        }
        _ => {}
    }
}
