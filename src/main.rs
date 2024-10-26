#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::Error;
use bytes::BufMut;
use serde_json::{self, json};
use sha1::{Digest, Sha1};
use std::{collections::BTreeMap, env, fs};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    String(Vec<u8>),
    Integer(isize),
    Array(Vec<Value>),
    Dict(BTreeMap<Vec<u8>, Value>),
}

fn encode_value(value: &Value) -> Vec<u8> {
    match value {
        Value::String(s) => {
            let mut len = s.len();
            let mut str = vec![];
            loop {
                if len < 10 {
                    str.push(len as u8 + b'0');
                    break;
                }
                let n = len % 10;
                len /= 10;
                str.push(n as u8 + b'0')
            }
            str.reverse();

            let mut string = vec![];
            string.put(&str[..]);
            string.put(&b":"[..]);
            string.put(&s[..]);
            string
        }
        Value::Integer(mut i) => {
            // -52
            let is_negative = i.is_negative();
            if i.is_negative() {
                i *= -1;
            }
            let mut integer = vec![];
            loop {
                if i < 10 {
                    integer.push(i as u8 + b'0');
                    break;
                }
                let n = i % 10;
                i /= 10;
                integer.push(n as u8 + b'0')
            }
            if is_negative {
                integer.push(b'-');
            }
            integer.push(b'i');
            integer.reverse();
            integer.push(b'e');
            integer
        }
        Value::Array(array) => {
            let mut encoded_array = vec![];
            encoded_array.push(b'l');
            for value in array {
                let mut v = encode_value(&value);
                encoded_array.append(&mut v);
            }
            encoded_array.push(b'e');
            encoded_array
        }
        Value::Dict(dict) => {
            let mut encoded_dict = vec![];
            encoded_dict.push(b'd');
            for (key, value) in dict {
                let mut key = encode_value(&Value::String(key.clone()));
                let mut value = encode_value(&value);
                encoded_dict.append(&mut key);
                encoded_dict.append(&mut value);
            }
            encoded_dict.push(b'e');
            encoded_dict
        }
    }
}

fn decode_string(mut encoded_value: &[u8]) -> anyhow::Result<(Option<Value>, &[u8])> {
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

fn decode_integer(encoded_value: &[u8]) -> anyhow::Result<(Option<Value>, &[u8])> {
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

fn decode_list(encoded_value: &[u8]) -> anyhow::Result<(Option<Value>, &[u8])> {
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

fn decode_dict(encoded_value: &[u8]) -> anyhow::Result<(Option<Value>, &[u8])> {
    if encoded_value[0] == b'd' {
        let mut values = BTreeMap::new();
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
    announce: Vec<u8>,
    info: Info,
}

impl MetaInfo {
    fn from_value(value: &Value) -> anyhow::Result<Self> {
        if let Value::Dict(meta_info) = value {
            let announce = if let Value::String(a) = meta_info[&b"announce"[..]].clone() {
                a
            } else {
                return Err(Error::msg("cannot parse announce from bencode value"));
            };

            let info = "info".as_bytes().to_vec();
            let info = Info::from_value(&meta_info[&info])?;
            Ok(Self { announce, info })
        } else {
            return Err(Error::msg("Provided value is not dictionary"));
        }
    }
}

#[derive(Debug)]
struct Info {
    // size of the file in bytes, for single-file torrents
    length: usize,
    // suggested name to save a file
    name: Vec<u8>,
    // number of bytes in each piece
    piece_length: usize,
    // concatenated SHA-1 hashes of each piece
    pieces: Vec<u8>,
}

impl Info {
    fn from_value(value: &Value) -> anyhow::Result<Self> {
        if let Value::Dict(info) = value {
            let length: usize = if let Value::Integer(a) = info[&b"length"[..]] {
                a as usize
            } else {
                return Err(Error::msg("cannot get Length from Info dict."));
            };

            let name: Vec<u8> = if let Value::String(a) = info[&b"name"[..]].clone() {
                a
            } else {
                return Err(Error::msg("cannot parse Info Name"));
            };

            let piece_length: usize = if let Value::Integer(a) = info[&b"piece length"[..]] {
                a as usize
            } else {
                return Err(Error::msg("cannot parse Info piece length"));
            };

            let pieces: Vec<u8> = if let Value::String(a) = info[&b"pieces"[..]].clone() {
                a
            } else {
                return Err(Error::msg("cannot parse Info peices"));
            };

            Ok(Self {
                length,
                name,
                piece_length,
                pieces,
            })
        } else {
            return Err(Error::msg("Provided value is not dictionary"));
        }
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() -> anyhow::Result<()> {
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

            let decoded_value = decode_bencoded_value(&file)
                .0
                .expect("decode bencode value");

            let meta_info =
                MetaInfo::from_value(&decoded_value).expect("parse MetaInfo from value");

            let info_hash = if let Value::Dict(meta_info) = decoded_value {
                let mut info_hash = Sha1::new();
                let info = &meta_info[&b"info"[..]];
                let info = encode_value(info);
                info_hash.update(&info);
                info_hash.finalize()
            } else {
                return Err(Error::msg("Cannot hash info"));
            };

            let piece_hashes = meta_info.info.pieces.chunks(20).collect::<Vec<&[u8]>>();

            println!(
                "Tracker URL: {}",
                meta_info
                    .announce
                    .iter()
                    .map(|c| *c as char)
                    .collect::<String>()
            );
            println!("Length: {}", meta_info.info.length);
            println!("Info Hash: {:x}", info_hash);
            println!("Piece Length: {}", meta_info.info.piece_length);
            println!("Piece Hashes:");
            for hash in &piece_hashes {
                for p in *hash {
                    print!("{:02x}", p);
                }
                println!();
            }
        }
        _ => {}
    }
    Ok(())
}
