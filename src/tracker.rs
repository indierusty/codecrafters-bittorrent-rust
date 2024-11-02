use anyhow::Context;
use std::net::Ipv4Addr;
use std::net::SocketAddrV4;

use crate::{value::Value, Torrent};

pub struct TrackerResponse {
    // An integer, indicating how often your client should make a request to the tracker.
    // You can ignore this value for the purposes of this challenge.
    interval: usize,
    // A string, contains list of peers that your client can connect to.
    // Each peer is represented using 6 bytes. The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number
    pub peers: Vec<u8>,
}

impl TrackerResponse {
    pub fn from_value(value: Value) -> anyhow::Result<Self> {
        if let Value::Dict(res) = value {
            let interval = if let Some(Value::Integer(v)) = res.get(&b"interval"[..]) {
                *v as usize
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
            "failed to parse tracker response from value",
        ))
    }
}

/// Query Params for making Get requet to Tracker
pub struct TrackerRequest {
    /// 20 bytes long info hash of the torrent need to be URL encoded
    pub info_hash: [u8; 20],
    /// port your client is listening on set 6881 for this challenge
    pub port: u32,
    /// a unique identifier for your client of length 20 that you get to pick.
    pub peer_id: String,
    /// the total amount uploaded so far, 0 as default
    pub uploaded: u32,
    /// the total amount downloaded so far, 0 as default
    pub downloaded: u32,
    /// number of bytes left to download, total length of file as default
    pub left: u32,
    // whether the peer list should use the compact representation
    // set true as default. used mostly for backward compatibily
    pub compact: u32,
}

pub async fn get_peers(torrent: &Torrent) -> anyhow::Result<Vec<SocketAddrV4>> {
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
        String::from_utf8(torrent.announce.clone())?,
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

    let mut peers = Vec::new();

    for peer in tracker_res.peers.chunks(6) {
        let peer = SocketAddrV4::new(
            Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]),
            u16::from_be_bytes([peer[4], peer[5]]),
        );
        peers.push(peer);
    }

    Ok(peers)
}
