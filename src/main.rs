#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::Context;
use bytes::BufMut;
use std::env;
use std::fs;
use std::net::SocketAddrV4;
// use tokio::io::{AsyncReadExt, AsyncWriteExt};

mod peer;
mod torrent;
mod tracker;
mod value;

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
        "download_piece" => {
            let _ = args.next().context("expected -o")?;
            let output_path = args.next().context("get output path")?;
            let torrent_path = args.next().context("get torrent file path")?;
            let piece_index = args.next().context("get piece index to download")?;

            let torrent = parse_torrent_file(&torrent_path).context("parse torrent file")?;
            let piece_index = piece_index.parse::<u32>().expect("piece index must be u32");
            let info_hash = torrent.info.hash();
            let my_peer_id = b"randombyterandombyte";
            let peers = get_peers(&torrent).await?;

            let (_handshake_msg, mut peer_stream) =
                handsake_peer(peers[1], info_hash, *my_peer_id).await?;

            // recieve [bitfield] message
            {
                let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            }
            // send [interested] message
            {
                let pmf = PeerMsgFrame::new(MsgType::Interested, Vec::new());
                pmf.write(&mut peer_stream).await?;
            }
            // recieve unchoke message
            {
                let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
                assert_eq!(pmf.msg_type, MsgType::Unchoke);
            }

            let info = torrent.info.clone();
            let npieces = info.pieces.chunks(20).count() as u32;
            let piece_len = if piece_index == npieces - 1 {
                let md = info.length % info.piece_length;
                if md == 0 {
                    info.piece_length
                } else {
                    md
                }
            } else {
                info.piece_length
            };

            let sixteen_kb = 16 * 1024;

            let mut blocks = Vec::<(usize, usize)>::new();
            let total_blocks = if piece_len % sixteen_kb == 0 {
                piece_len / sixteen_kb
            } else {
                piece_len / sixteen_kb + 1
            };

            for block_index in 0..total_blocks {
                let begin = sixteen_kb * block_index;
                let mut length = sixteen_kb;

                if block_index == total_blocks - 1 {
                    let remaining = piece_len % sixteen_kb;
                    length = if remaining == 0 {
                        sixteen_kb
                    } else {
                        remaining
                    };
                }
                blocks.push((begin, length));
            }

            let mut blocks_recieved = Vec::<(usize, Vec<u8>)>::new();
            for five_blocks_mx in blocks.chunks(5) {
                for (begin, length) in five_blocks_mx {
                    let mut payload = Vec::new();
                    payload.put_slice(&(piece_index as u32).to_be_bytes());
                    payload.put_slice(&(*begin as u32).to_be_bytes());
                    payload.put_slice(&(*length as u32).to_be_bytes());
                    assert_eq!(
                        *length as u32,
                        u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]])
                    );

                    let pmf = PeerMsgFrame::new(MsgType::Request, payload);
                    pmf.write(&mut peer_stream).await?;
                }

                for (begin, length) in five_blocks_mx {
                    let pmf = PeerMsgFrame::read(&mut peer_stream)
                        .await
                        .context("read message")?;

                    let index = u32::from_be_bytes([
                        pmf.payload[0],
                        pmf.payload[1],
                        pmf.payload[2],
                        pmf.payload[3],
                    ]);
                    assert_eq!(index, piece_index);
                    let begin = u32::from_be_bytes([
                        pmf.payload[4],
                        pmf.payload[5],
                        pmf.payload[6],
                        pmf.payload[7],
                    ]);
                    let data = &pmf.payload[8..];
                    assert_eq!(data.len(), *length);
                    blocks_recieved.push((begin as usize, data.to_vec()));
                    dbg!(&blocks_recieved.len());
                }
            }

            let mut file = Vec::new();
            for (_, data) in blocks_recieved {
                file.put_slice(&data);
            }

            fs::write(output_path, file).expect("write piece to file");
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
