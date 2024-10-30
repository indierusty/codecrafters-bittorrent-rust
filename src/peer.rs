#[repr(C)]
pub struct Handshake {
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

impl Handshake {
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
