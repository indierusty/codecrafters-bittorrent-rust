#![allow(dead_code)]
#![allow(unused_variables)]
use std::env;
use std::fs;

mod torrent;
mod value;

use torrent::*;
use value::*;

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() -> anyhow::Result<()> {
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
        _ => {}
    }
    Ok(())
}
