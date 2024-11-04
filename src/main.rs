#![allow(dead_code)]
#![allow(unused_variables)]
use anyhow::Context;
use bytes::BufMut;
use serde_json::json;
use std::env;
use std::fs;
use std::net::SocketAddrV4;
use tokio::net::TcpStream;

mod magnet;
mod peer;
mod torrent;
mod tracker;
mod value;

use magnet::*;
use peer::*;
use torrent::*;
use tracker::*;
use value::*;

const PEER_ID: [u8; 20] = *b"code5craf5ters5code5";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = env::args().skip(1);

    match args.next().expect("command").as_str() {
        "decode" => {
            let encoded_value = args.next().expect("encoded value");
            let decoded_value = Value::decode(&encoded_value.as_bytes())?;
            println!("{}", decoded_value.0.to_json());
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
            let peers = get_peers(&torrent, &PEER_ID).await?;
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
                handshake_peer(peer_address, &info_hash, &PEER_ID).await?;

            println!("Peer ID: {}", hex::encode(&handshake_msg.peer_id));
        }
        "download_piece" => {
            let _ = args.next().context("expected -o")?;
            let output_path = args.next().context("get output path")?;
            let torrent_path = args.next().context("get torrent file path")?;
            let piece_index = args.next().context("get piece index to download")?;

            let torrent = parse_torrent_file(&torrent_path).context("parse torrent file")?;
            let piece_index = piece_index.parse::<u32>().expect("piece index must be u32");
            let peers = get_peers(&torrent, &PEER_ID).await?;
            let info = &torrent.info;

            let (_handshake_msg, mut peer_stream) =
                handshake_peer(peers[1], &info.hash(), &PEER_ID).await?;

            // recieve [bitfield] message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_id != MsgID::Bitfield {
                todo!()
            }
            // send [interested] message
            let pmf = PeerMsgFrame::new(MsgID::Interested, Vec::new());
            pmf.write(&mut peer_stream).await?;
            // recieve unchoke message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_id != MsgID::Unchoke {
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
        "download" => {
            let _ = args.next().context("expected -o")?;
            let output_path = args.next().context("get output path")?;
            let torrent_path = args.next().context("get torrent file path")?;

            let torrent = parse_torrent_file(&torrent_path).context("parse torrent file")?;
            let my_peer_id = b"randombyterandombyte";
            let peers = get_peers(&torrent, &PEER_ID).await?;
            let info = &torrent.info;

            struct Piece {
                index: u32,
                length: u32,
                data: Vec<u8>,
            }

            impl Piece {
                fn new(index: u32, length: u32) -> Self {
                    Self {
                        index,
                        length,
                        data: Vec::new(),
                    }
                }
            }

            let npieces = info.pieces.len() as u32;
            let mut pieces = Vec::new();
            for i in 0..npieces {
                // calculate piece length in bytes
                let rem = info.length % info.piece_length;
                let len = if i == npieces - 1 && rem != 0 {
                    rem
                } else {
                    info.piece_length
                };

                pieces.push(Piece::new(i, len as u32));
            }

            let (_handshake_msg, mut peer_stream) =
                handshake_peer(peers[1], &info.hash(), &PEER_ID).await?;

            // recieve [bitfield] message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_id != MsgID::Bitfield {
                todo!()
            }
            // send [interested] message
            let pmf = PeerMsgFrame::new(MsgID::Interested, Vec::new());
            pmf.write(&mut peer_stream).await?;
            // recieve unchoke message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_id != MsgID::Unchoke {
                todo!()
            }

            for piece in &mut pieces {
                let mut data = download_piece(piece.index, piece.length, &mut peer_stream).await?;
                piece.data.append(&mut data);
            }

            pieces.sort_by(|a, b| a.index.cmp(&b.index));
            let file = pieces.iter_mut().fold(Vec::new(), |mut acc, p| {
                acc.append(&mut p.data);
                acc
            });
            fs::write(output_path, file).expect("write downloaded pieces to file");
        }
        "magnet_parse" => {
            let magnet_link = args.next().expect("magnet-link");
            let magnet = Magnet::parse(&magnet_link)?;

            println!("Tracker URL: {}", magnet.tracker_url);
            println!("Info Hash: {}", hex::encode(magnet.info_hash));
        }
        "magnet_handshake" => {
            let magnet_link = args.next().expect("magnet-link");
            let magnet = Magnet::parse(&magnet_link)?;
            let peers = get_peers(&magnet, &PEER_ID).await?;

            // 1. Establish a TCP connection with a peer
            // 2. Send the base handshake message
            // 3. Receive the base handshake message
            let (handshake_msg, mut peer_stream) =
                handshake_peer(peers[0], &magnet.info_hash, &PEER_ID).await?;
            let peer_id = handshake_msg.peer_id;

            // 4. Receive the bitfield message
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            if pmf.msg_id != MsgID::Bitfield {
                return Err(anyhow::Error::msg("did not recived bitfield msg"));
            }

            if !handshake_msg.is_supporting_extention() {
                return Err(anyhow::Error::msg("Peer does not support extension"));
            }

            let extension_json = json!({
                "m": {
                    "ut_metadata": 1,
                    "ut_pex": 2,
                },
            });

            let extension_value = Value::from_json(&extension_json)?;
            let mut extension_payload = Vec::new();
            extension_payload.push(0);
            extension_payload.append(&mut extension_value.encode());

            // 5. Send Extension Handshake Msg
            let pmf = PeerMsgFrame::new(MsgID::Extended, extension_payload);
            pmf.write(&mut peer_stream).await?;

            // 6. Receive Extension Handshake Msg
            let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
            let extension_msg = Value::decode(&pmf.payload[1..])?;
            let ut_metadata_id = extension_msg.0.to_json()["m"]["ut_metadata"]
                .clone()
                .as_i64()
                .unwrap() as u32;

            println!("Peer ID: {}", hex::encode(peer_id));
            println!("Peer Metadata Extension ID: {}", ut_metadata_id);
        }
        "magnet_info" => {
            let magnet_link = args.next().expect("magnet-link");
            let torrent = get_torrent_using_magnet(&magnet_link).await?;

            println!("Tracker URL: {}", torrent.announce());
            println!("Length: {}", torrent.length());
            println!("Info Hash: {}", hex::encode(torrent.info_hash()));
            println!("Piece Length: {}", torrent.info.piece_length);
            println!("Piece Hashes:");
            for hash in torrent.piece_hashes() {
                for p in hash {
                    print!("{:02x}", p);
                }
                println!();
            }
        }
        "magnet_download_piece" => {
            let _ = args.next().context("expected -o")?;
            let output_path = args.next().context("get output path")?;
            let magnet_link = args.next().context("get torrent file path")?;
            let piece_index = args.next().context("get piece index to download")?;

            let piece_index = piece_index.parse::<u32>().expect("piece index must be u32");
            let torrent = get_torrent_using_magnet(&magnet_link).await?;
            let peers = get_peers(&torrent, &PEER_ID).await?;
            let info = &torrent.info;

            let mut peer_stream = None;
            for i in 0..peers.len() {
                let (_handshake_msg, mut stream) =
                    handshake_peer(peers[i], &info.hash(), &PEER_ID).await?;

                // recieve [bitfield] message
                let pmf = PeerMsgFrame::read(&mut stream).await?;
                if pmf.msg_id != MsgID::Bitfield {
                    eprintln!("Didn't recieve unchocke msgid, got {:?}", pmf.msg_id);
                    continue;
                }

                // send [interested] message
                let pmf = PeerMsgFrame::new(MsgID::Interested, Vec::new());
                pmf.write(&mut stream).await?;

                // TODO: peer replie with unchoke message after this Extended Msg read. find out why?
                let pmf = PeerMsgFrame::read(&mut stream).await?;

                // recieve unchoke message
                let pmf = PeerMsgFrame::read(&mut stream).await?;
                if pmf.msg_id != MsgID::Unchoke {
                    eprintln!("Didn't recieve unchocke msgid, got {:?}", pmf.msg_id);
                    continue;
                }

                peer_stream = Some(stream);
                break;
            }

            let mut peer_stream = if let Some(stream) = peer_stream {
                stream
            } else {
                return Err(anyhow::Error::msg("Could not connect to any peer"));
            };

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
        "magnet_download" => {
            let _ = args.next().context("expected -o")?;
            let output_path = args.next().context("get output path")?;
            let magnet_link = args.next().context("get torrent file path")?;

            let torrent = get_torrent_using_magnet(&magnet_link).await?;
            let peers = get_peers(&torrent, &PEER_ID).await?;
            let info = &torrent.info;

            // connect to a peer
            let mut peer_stream = None;
            for i in 0..peers.len() {
                let (_handshake_msg, mut stream) =
                    handshake_peer(peers[i], &info.hash(), &PEER_ID).await?;

                // recieve [bitfield] message
                let pmf = PeerMsgFrame::read(&mut stream).await?;
                if pmf.msg_id != MsgID::Bitfield {
                    eprintln!("Didn't recieve unchocke msgid, got {:?}", pmf.msg_id);
                    continue;
                }

                // send [interested] message
                let pmf = PeerMsgFrame::new(MsgID::Interested, Vec::new());
                pmf.write(&mut stream).await?;

                // TODO: peer replie with unchoke message after this Extended Msg read. find out why?
                let pmf = PeerMsgFrame::read(&mut stream).await?;

                // recieve unchoke message
                let pmf = PeerMsgFrame::read(&mut stream).await?;
                if pmf.msg_id != MsgID::Unchoke {
                    eprintln!("Didn't recieve unchocke msgid, got {:?}", pmf.msg_id);
                    continue;
                }

                peer_stream = Some(stream);
                break;
            }

            let mut peer_stream = if let Some(stream) = peer_stream {
                stream
            } else {
                return Err(anyhow::Error::msg("Could not connect to any peer"));
            };

            // calculate peices
            struct Piece {
                index: u32,
                length: u32,
                data: Vec<u8>,
            }

            impl Piece {
                fn new(index: u32, length: u32) -> Self {
                    Self {
                        index,
                        length,
                        data: Vec::new(),
                    }
                }
            }

            let npieces = info.pieces.len() as u32;
            let mut pieces = Vec::new();
            for i in 0..npieces {
                // calculate piece length in bytes
                let rem = info.length % info.piece_length;
                let len = if i == npieces - 1 && rem != 0 {
                    rem
                } else {
                    info.piece_length
                };

                pieces.push(Piece::new(i, len as u32));
            }

            // dowload peices from peer
            for piece in &mut pieces {
                let mut data = download_piece(piece.index, piece.length, &mut peer_stream).await?;
                piece.data.append(&mut data);
            }

            // sort peices and save to file
            pieces.sort_by(|a, b| a.index.cmp(&b.index));
            let file = pieces.iter_mut().fold(Vec::new(), |mut acc, p| {
                acc.append(&mut p.data);
                acc
            });
            fs::write(output_path, file).expect("write downloaded pieces to file");
        }
        _ => {}
    }
    Ok(())
}

async fn get_torrent_using_magnet(magnet_link: &str) -> anyhow::Result<Torrent> {
    let magnet = Magnet::parse(&magnet_link)?;
    let peers = get_peers(&magnet, &PEER_ID).await?;

    // 1. Establish a TCP connection with a peer
    // 2. Send the base handshake message
    // 3. Receive the base handshake message
    let (handshake_msg, mut peer_stream) =
        handshake_peer(peers[0], &magnet.info_hash, &PEER_ID).await?;
    let peer_id = handshake_msg.peer_id;

    // 4. Receive the bitfield message
    let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
    if pmf.msg_id != MsgID::Bitfield {
        return Err(anyhow::Error::msg("did not recived bitfield msg"));
    }

    if !handshake_msg.is_supporting_extention() {
        return Err(anyhow::Error::msg("Peer does not support extension"));
    }

    let extension_json = json!({
        "m": {
            "ut_metadata": 1,
            "ut_pex": 2,
        },
    });

    let extension_value = Value::from_json(&extension_json)?;
    let mut extension_payload = Vec::new();
    extension_payload.push(0);
    extension_payload.append(&mut extension_value.encode());

    // 5. Send Extension Handshake Msg
    let pmf = PeerMsgFrame::new(MsgID::Extended, extension_payload);
    pmf.write(&mut peer_stream).await?;

    // 6. Receive Extension Handshake Msg
    let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
    let extension_msg = Value::decode(&pmf.payload[1..])?;
    let ut_metadata_id: u8 = extension_msg.0.to_json()["m"]["ut_metadata"]
        .clone()
        .as_i64()
        .unwrap() as u8;

    // 7. request info using Metadata extension Messages
    // {  msg_type will be 0 since this is a request message
    //    piece is the zero-based piece index of the metadata being requested
    //    Since we're only requesting one piece in this challenge, this will always be 0
    // }
    let metadata_request_msg = json!({
        "msg_type": 0,
        "piece": 0,
    });

    let value = Value::from_json(&metadata_request_msg)?;
    let mut payload = Vec::new();
    payload.push(ut_metadata_id);
    payload.append(&mut value.encode());

    // 8. Request MetaInfo using Metadata Extension Msg
    let pmf = PeerMsgFrame::new(MsgID::Extended, payload);
    pmf.write(&mut peer_stream).await?;

    // 9. Read MetaInfo
    let pmf = PeerMsgFrame::read(&mut peer_stream).await?;
    let (msg, rest) = Value::decode(&pmf.payload[1..])?;
    let metadata_size: usize = msg.to_json()["total_size"].take().as_u64().unwrap() as usize;
    let (metadata, _rest) = Value::decode(rest)?;
    let meta_info = Info::from_value(&metadata)?;

    let torrent = Torrent {
        announce: magnet.announce().as_bytes().to_vec(),
        info: meta_info,
    };

    Ok(torrent)
}

async fn download_piece(
    piece_index: u32,
    piece_len: u32,
    peer_stream: &mut TcpStream,
) -> anyhow::Result<Vec<u8>> {
    const SIXTEEN_KB: u32 = 16 * 1024;

    struct Block {
        index: u32,
        begin: u32,
        length: u32,
        data: Vec<u8>,
    }

    impl Block {
        fn new(index: u32, begin: u32, length: u32) -> Self {
            Self {
                index,
                begin,
                length,
                data: Vec::new(),
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

            let pmf = PeerMsgFrame::new(MsgID::Request, payload);
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
            assert_eq!(data.len() as u32, block.length);
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
    let decoded_value = Value::decode(&file).context("decode bencode value")?.0;
    let torrent = Torrent::from_value(&decoded_value).context("parse MetaInfo from value")?;
    Ok(torrent)
}
