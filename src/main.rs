#![allow(dead_code)]
#![allow(unused_variables)]
use std::env;
use std::fs;
use std::net::Ipv4Addr;
use std::net::SocketAddrV4;

mod torrent;
mod value;

use anyhow::Context;
use torrent::*;
use value::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);

    match args.next().expect("command").as_str() {
        "decode" => {
            let encoded_value = args.next().expect("encoded value");
            let decoded_value = Value::decode(&encoded_value.as_bytes())?;
            println!("{}", decoded_value.to_json());
        }
        "info" => {
            let file_path = args.next().expect("path to torrent file");
            let file = fs::read(file_path).expect("read file");
            let decoded_value = Value::decode(&file).expect("decode bencode value");
            let torrent = Torrent::from_value(&decoded_value).expect("parse MetaInfo from value");
            let info_hash = torrent.info.hash();
            let piece_hashes = torrent.piece_hashes();

            println!(
                "Tracker URL: {}",
                torrent
                    .announce
                    .iter()
                    .map(|c| *c as char)
                    .collect::<String>()
            );
            println!("Length: {}", torrent.info.length);
            println!(
                "Info Hash: {}",
                info_hash.iter().fold(String::new(), |mut acc, c| {
                    acc.push_str(&format!("{:02x}", *c));
                    acc
                })
            );
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            for hash in &piece_hashes {
                for p in *hash {
                    print!("{:02x}", p);
                }
                println!();
            }
        }
        "peers" => {
            let file_path = args.next().expect("path to torrent file");
            let file = fs::read(file_path).expect("read file");
            let decoded_value = Value::decode(&file).expect("decode bencode value");
            let torrent = Torrent::from_value(&decoded_value).expect("parse MetaInfo from value");

            let tracker = TrackerRequest {
                info_hash: torrent.info.hash(),
                port: 6881,
                peer_id: "code5craf5ters5code5".to_string(),
                uploaded: 0,
                downloaded: 0,
                left: torrent.info.length,
                compact: 1,
            };

            let info_hash_url = tracker.info_hash.iter().fold(String::new(), |mut acc, c| {
                acc.push('%');
                acc.push_str(&format!("{:02x}", c));
                acc
            });

            let request_url = format!(
                "{}?port={}&peer_id={}&uploaded={}&downloaded={}&left={}&compact={}&info_hash={}",
                String::from_utf8(torrent.announce)?,
                tracker.port,
                tracker.peer_id,
                tracker.uploaded,
                tracker.downloaded,
                tracker.left,
                tracker.compact,
                info_hash_url
            );

            let response = reqwest::get(&request_url).await.context("query tracker")?;

            let value = Value::decode(&response.bytes().await?)?;
            let tracker_res = TrackerResponse::from_value(value)?;

            for peer in tracker_res.peers.chunks(6) {
                let peer = SocketAddrV4::new(
                    Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]),
                    u16::from_be_bytes([peer[4], peer[5]]),
                );
                println!("{}:{}", peer.ip(), peer.port());
            }
        }
        _ => {}
    }
    Ok(())
}

pub struct TrackerResponse {
    // An integer, indicating how often your client should make a request to the tracker.
    // You can ignore this value for the purposes of this challenge.
    interval: usize,
    // A string, contains list of peers that your client can connect to.
    // Each peer is represented using 6 bytes. The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number
    peers: Vec<u8>,
}

impl TrackerResponse {
    fn from_value(value: Value) -> anyhow::Result<Self> {
        if let Value::Dict(res) = value {
            let interval = if let Some(Value::Integer(i)) = res.get(&b"interval"[..]) {
                *i as usize
            } else {
                return Err(anyhow::Error::msg("no interval in tracker response"));
            };
            let peers = if let Some(Value::String(v)) = res.get(&b"peers"[..]) {
                v
            } else {
                return Err(anyhow::Error::msg("no peers in tracker response"));
            };
            return Ok(Self {
                interval,
                peers: peers.clone(),
            });
        }

        Err(anyhow::Error::msg(
            "failed parse tracker response from value",
        ))
    }
}

/// Query Params for making Get requet to Tracker
pub struct TrackerRequest {
    /// 20 bytes long info hash of the torrent need to be URL encoded
    info_hash: [u8; 20],
    /// port your client is listening on set 6881 for this challenge
    port: usize,
    /// a unique identifier for your client of length 20 that you get to pick.
    peer_id: String,
    /// the total amount uploaded so far, 0 as default
    uploaded: usize,
    /// the total amount downloaded so far, 0 as default
    downloaded: usize,
    /// number of bytes left to download, total length of file as default
    left: usize,
    // whether the peer list should use the compact representation
    // set true as default. used mostly for backward compatibily
    compact: usize,
}
