#![allow(dead_code)]
#![allow(unused_variables)]
use std::env;
use std::fs;
use std::net::Ipv4Addr;
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
        "handshake" => {
            use tokio::io::AsyncReadExt;
            use tokio::io::AsyncWriteExt;

            let file_path = args.next().expect("path to torrent file");
            let peer_add = args.next().expect("peer address");
            let torrent = parse_torrent_file(&file_path)?;

            let info_hash = torrent.info.hash();
            let peer = peer_add.parse::<SocketAddrV4>().unwrap();
            let mut handshake = Handshake::new(info_hash, *b"asdf5asdf5asdf5asdf5");

            let mut peer = tokio::net::TcpStream::connect(peer).await.unwrap();

            let handshake_bytes =
                &mut handshake as *mut Handshake as *mut [u8; std::mem::size_of::<Handshake>()];
            // Safety: Handshake is a POD(Plain Old Data) with repr(c)
            let handshake_bytes: &mut [u8; std::mem::size_of::<Handshake>()] =
                unsafe { &mut *handshake_bytes };
            peer.write_all(handshake_bytes)
                .await
                .context("write handshake")?;
            peer.read_exact(handshake_bytes)
                .await
                .context("read handshake")?;

            println!("Peer ID: {}", hex::encode(&handshake.peer_id));
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
