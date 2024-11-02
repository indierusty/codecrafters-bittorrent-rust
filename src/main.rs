#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::Context;
use bytes::BufMut;
use std::env;
use std::fs;
use std::net::SocketAddrV4;
use tokio::net::TcpStream;

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
            for hash in piece_hashes {
                for p in hash {
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
            let my_peer_id = b"randombyterandombyte";
            let peers = get_peers(&torrent).await?;
            let info = &torrent.info;

            let (_handshake_msg, mut peer_stream) =
                handsake_peer(peers[1], info.hash(), *my_peer_id).await?;

            // recieve [bitfield] message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_type != MsgType::Bitfield {
                todo!()
            }
            // send [interested] message
            let pmf = PeerMsgFrame::new(MsgType::Interested, Vec::new());
            pmf.write(&mut peer_stream).await?;
            // recieve unchoke message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_type != MsgType::Unchoke {
                todo!()
            }

            let npieces = info.pieces.len() as u32;
            let rem = info.length % info.piece_length;
            let piece_len = if piece_index == npieces - 1 && rem != 0 {
                rem
            } else {
                info.piece_length
            };

            let piece = download_piece(piece_index, piece_len, &mut peer_stream).await?;
            fs::write(output_path, piece).expect("write piece to file");
        }
        _ => {}
    }
    Ok(())
}

async fn download_piece(
    piece_index: u32,
    piece_len: usize,
    peer_stream: &mut TcpStream,
) -> anyhow::Result<Vec<u8>> {
    const SIXTEEN_KB: usize = 16 * 1024;

    struct Block {
        index: usize,
        begin: usize,
        length: usize,
        data: Vec<u8>,
    }

    impl Block {
        fn new(index: usize, begin: usize, length: usize) -> Self {
            Self {
                index,
                begin,
                length,
                data: Vec::with_capacity(length),
            }
        }
    }

    let nblocks = if piece_len % SIXTEEN_KB == 0 {
        piece_len / SIXTEEN_KB
    } else {
        piece_len / SIXTEEN_KB + 1
    };

    let mut blocks = Vec::<Block>::new();

    for index in 0..nblocks {
        let begin = SIXTEEN_KB * index;
        let rem = piece_len % SIXTEEN_KB;
        let length = if index == nblocks - 1 && rem != 0 {
            rem
        } else {
            SIXTEEN_KB
        };
        blocks.push(Block::new(index, begin, length));
    }

    for block_chunk in blocks.chunks_mut(5) {
        for block in block_chunk.iter() {
            let mut payload = Vec::new();
            payload.put_slice(&(piece_index as u32).to_be_bytes());
            payload.put_slice(&(block.begin as u32).to_be_bytes());
            payload.put_slice(&(block.length as u32).to_be_bytes());
            assert_eq!(
                block.length as u32,
                u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]])
            );

            let pmf = PeerMsgFrame::new(MsgType::Request, payload);
            pmf.write(peer_stream).await?;
        }

        for block in block_chunk.iter_mut() {
            let pmf = PeerMsgFrame::read(peer_stream)
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
            assert_eq!(data.len(), block.length);
            block.data.put_slice(data);
        }
    }
    let mut piece = Vec::new();
    for Block { data, .. } in blocks {
        piece.put_slice(&data);
    }
    Ok(piece)
}

fn parse_torrent_file(file_path: &str) -> anyhow::Result<Torrent> {
    let file = fs::read(file_path).context("read torrent file")?;
    let decoded_value = Value::decode(&file).context("decode bencode value")?;
    let torrent = Torrent::from_value(&decoded_value).context("parse MetaInfo from value")?;
    Ok(torrent)
}
