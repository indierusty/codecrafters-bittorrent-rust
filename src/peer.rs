use bytes::BufMut;
use std::net::SocketAddrV4;
use tokio::net::TcpStream;

use anyhow::{Context, Error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MsgType {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

#[derive(Debug)]
pub struct PeerMsgFrame {
    /// first 4 bytes are length of payload and rest 1 byte is Msg ID
    pub msg_type: MsgType,
    /// payload of variable size of length given in prefix
    pub payload: Vec<u8>,
}

impl PeerMsgFrame {
    pub fn new(msg_type: MsgType, payload: Vec<u8>) -> Self {
        Self { msg_type, payload }
    }

    pub async fn read(stream: &mut TcpStream) -> anyhow::Result<Self> {
        let mut buf = vec![0u8; 5];
        stream.read_exact(&mut buf).await?;

        let msg_type = match buf[4] {
            0 => MsgType::Choke,
            1 => MsgType::Unchoke,
            2 => MsgType::Interested,
            3 => MsgType::NotInterested,
            4 => MsgType::Have,
            5 => MsgType::Bitfield,
            6 => MsgType::Request,
            7 => MsgType::Piece,
            8 => MsgType::Cancel,
            m => return Err(Error::msg(format!("Unknown message type {}.", m))),
        };

        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        // subtract the length of 'message type' which is 1 for a byte
        let mut payload = vec![0u8; len - 1];
        if len > 0 {
            _ = stream.read_exact(&mut payload).await?;
        }

        Ok(Self { msg_type, payload })
    }

    pub async fn write(&self, stream: &mut TcpStream) -> anyhow::Result<()> {
        let bytes = self.to_bytes();
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // NOTE: + 1 for length of MsgType
        buf.put_slice(&(self.payload.len() + 1).to_be_bytes());
        buf.put_u8(self.msg_type as u8);
        buf.put(&self.payload[..]);
        buf
    }
}

#[derive(Debug)]
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
        // enable bittorent extention system by setting 20th bit in reserved byte.
        let reserved_bytes = [0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00];
        Self {
            len: 19,
            string: *b"BitTorrent protocol",
            reserved_bytes,
            info_hash,
            peer_id,
        }
    }
}

pub async fn handshake_peer(
    peer_address: SocketAddrV4,
    info_hash: &[u8; 20],
    my_peer_id: &[u8; 20],
) -> anyhow::Result<(HandshakeMsg, TcpStream)> {
    let mut handshake_msg = HandshakeMsg::new(info_hash.clone(), my_peer_id.clone());
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
