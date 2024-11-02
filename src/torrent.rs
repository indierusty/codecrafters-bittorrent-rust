use std::collections::BTreeMap;

use crate::value::*;
use anyhow::Error;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct Torrent {
    pub announce: Vec<u8>,
    pub info: Info,
}

impl Torrent {
    pub fn from_value(value: &Value) -> anyhow::Result<Self> {
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

    pub fn piece_hashes(&self) -> Vec<&[u8]> {
        self.info.pieces.chunks(20).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Info {
    // size of the file in bytes, for single-file torrents
    pub length: usize,
    // suggested name to save a file
    pub name: Vec<u8>,
    // number of bytes in each piece
    pub piece_length: usize,
    // concatenated SHA-1 hashes of each piece
    pub pieces: Vec<u8>,
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

            let pieces = if let Value::String(a) = info[&b"pieces"[..]].clone() {
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

    pub fn to_value(&self) -> Value {
        let mut map = BTreeMap::new();
        map.insert(b"length"[..].to_vec(), Value::Integer(self.length as isize));
        map.insert(b"name"[..].to_vec(), Value::String(self.name.clone()));
        map.insert(
            b"piece length"[..].to_vec(),
            Value::Integer(self.piece_length as isize),
        );
        map.insert(b"pieces"[..].to_vec(), Value::String(self.pieces.clone()));
        Value::Dict(map)
    }

    pub fn hash(&self) -> [u8; 20] {
        let bencode = self.to_value().encode();
        let mut hasher = Sha1::new();
        hasher.update(&bencode);
        hasher.finalize().into()
    }
}
