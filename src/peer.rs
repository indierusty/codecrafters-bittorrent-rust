use std::net::SocketAddrV4;
use tokio::net::TcpStream;

use anyhow::{Context, Error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[repr(C)]
pub struct HandshakeMsg {
    /// length of the protocol string (BitTorrent protocol) which is 19 (1 byte)
    pub len: u8,
    /// the string BitTorrent protocol (19 bytes)
    pub string: [u8; 19],
    /// eight reserved bytes, which are all set to zero (8 bytes)
    pub reserved_bytes: [u8; 8],
    /// sha1 infohash (20 bytes) (NOT the hexadecimal representation, which is 40 bytes long)
    pub info_hash: [u8; 20],
    /// peer id (20 bytes) (generate 20 random byte values)            
    pub peer_id: [u8; 20],
}

impl HandshakeMsg {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            len: 19,
            string: *b"BitTorrent protocol",
            reserved_bytes: [0u8; 8],
            info_hash,
            peer_id,
        }
    }
}

pub async fn handsake_peer(
    peer_address: SocketAddrV4,
    info_hash: [u8; 20],
    my_peer_id: [u8; 20],
) -> anyhow::Result<(HandshakeMsg, TcpStream)> {
    let mut handshake_msg = HandshakeMsg::new(info_hash, *b"asdf5asdf5asdf5asdf5");
    let mut peer = tokio::net::TcpStream::connect(peer_address).await.unwrap();

    let handshake_msg_bytes =
        &mut handshake_msg as *mut HandshakeMsg as *mut [u8; std::mem::size_of::<HandshakeMsg>()];
    // Safety: Handshake is a POD(Plain Old Data) with repr(c)
    let handshake_msg_bytes: &mut [u8; std::mem::size_of::<HandshakeMsg>()] =
        unsafe { &mut *handshake_msg_bytes };
    peer.write_all(handshake_msg_bytes)
        .await
        .context("write handshake")?;
    peer.read_exact(handshake_msg_bytes)
        .await
        .context("read handshake")?;

    // TODO: remove check if it is possilbly unccessory
    if handshake_msg.len != 19u8 || handshake_msg.string != *b"BitTorrent protocol" {
        return Err(Error::msg("Handshake Response didn't matched"));
    }

    Ok((handshake_msg, peer))
}
