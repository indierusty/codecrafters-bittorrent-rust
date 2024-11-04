use std::collections::BTreeMap;

use crate::value::*;
use anyhow::{Context, Error};
use bytes::BufMut;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct Torrent {
    pub announce: Vec<u8>,
    pub info: Info,
}

pub trait TorrentInfo {
    fn announce(&self) -> String;
    fn info_hash(&self) -> [u8; 20];
    fn length(&self) -> u32;
}

impl TorrentInfo for Torrent {
    fn announce(&self) -> String {
        String::from_utf8(self.announce.to_vec()).unwrap()
    }

    fn info_hash(&self) -> [u8; 20] {
        self.info.hash()
    }

    fn length(&self) -> u32 {
        self.info.length
    }
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

    pub fn piece_hashes(&self) -> &Vec<[u8; 20]> {
        &self.info.pieces
    }
}

#[derive(Debug, Clone)]
pub struct Info {
    // size of the file in bytes, for single-file torrents
    pub length: u32,
    // suggested name to save a file UTF-8 encoded
    pub name: String,
    // number of bytes in each piece
    pub piece_length: u32,
    // concatenated SHA-1 hashes of each piece
    pub pieces: Vec<[u8; 20]>,
}

impl Info {
    pub fn from_value(value: &Value) -> anyhow::Result<Self> {
        if let Value::Dict(info) = value {
            let length = if let Value::Integer(a) = info[&b"length"[..]] {
                a as u32
            } else {
                return Err(Error::msg("cannot get Length from Info dict."));
            };

            let name: String = if let Value::String(a) = info[&b"name"[..]].clone() {
                String::from_utf8(a).context("info name must be UTF-8 encoded")?
            } else {
                return Err(Error::msg("cannot parse Info Name"));
            };

            let piece_length = if let Value::Integer(a) = info[&b"piece length"[..]] {
                a as u32
            } else {
                return Err(Error::msg("cannot parse Info piece length"));
            };

            let pieces: Vec<[u8; 20]> = if let Value::String(a) = info[&b"pieces"[..]].clone() {
                // TODO: validate if value len is exact multiple of 20 and non-zero and return error
                a.chunks_exact(20)
                    .map(|c| std::array::from_fn(|i| c[i]))
                    .collect()
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
        map.insert(
            b"name"[..].to_vec(),
            Value::String(self.name.as_bytes().to_vec()),
        );
        map.insert(
            b"piece length"[..].to_vec(),
            Value::Integer(self.piece_length as isize),
        );
        map.insert(
            b"pieces"[..].to_vec(),
            Value::String(self.pieces.iter().fold(Vec::new(), |mut acc, v| {
                acc.put_slice(v);
                acc
            })),
        );
        Value::Dict(map)
    }

    pub fn hash(&self) -> [u8; 20] {
        let bencode = self.to_value().encode();
        let mut hasher = Sha1::new();
        hasher.update(&bencode);
        hasher.finalize().into()
    }
}
