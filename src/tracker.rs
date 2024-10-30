use crate::value::Value;

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
    pub port: usize,
    /// a unique identifier for your client of length 20 that you get to pick.
    pub peer_id: String,
    /// the total amount uploaded so far, 0 as default
    pub uploaded: usize,
    /// the total amount downloaded so far, 0 as default
    pub downloaded: usize,
    /// number of bytes left to download, total length of file as default
    pub left: usize,
    // whether the peer list should use the compact representation
    // set true as default. used mostly for backward compatibily
    pub compact: usize,
}
