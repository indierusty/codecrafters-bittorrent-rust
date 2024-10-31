#![allow(dead_code)]
#![allow(unused_variables)]
use std::env;
use std::fs;
use std::net::SocketAddrV4;

mod peer;
mod torrent;
mod tracker;
mod value;

use anyhow::Context;
use peer::*;
use torrent::*;
use tracker::*;
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
            let torrent = parse_torrent_file(&file_path)?;
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
            let torrent = parse_torrent_file(&file_path)?;
            let peers = get_peers(&torrent).await?;
            for peer in peers {
                println!("{}:{}", peer.ip(), peer.port());
            }
        }
        "handshake" => {
            let file_path = args.next().expect("path to torrent file");
            let peer_add = args.next().expect("peer address");
            let torrent = parse_torrent_file(&file_path)?;

            let info_hash = torrent.info.hash();
            let peer_address = peer_add.parse::<SocketAddrV4>().unwrap();

            let (handshake_msg, _peer_stream) =
                handsake_peer(peer_address, info_hash, *b"asdf5asdf5asdf5asdf5").await?;

            println!("Peer ID: {}", hex::encode(&handshake_msg.peer_id));
        }
        _ => {}
    }
    Ok(())
}

fn parse_torrent_file(file_path: &str) -> anyhow::Result<Torrent> {
    let file = fs::read(file_path).context("read torrent file")?;
    let decoded_value = Value::decode(&file).context("decode bencode value")?;
    let torrent = Torrent::from_value(&decoded_value).context("parse MetaInfo from value")?;
    Ok(torrent)
}
