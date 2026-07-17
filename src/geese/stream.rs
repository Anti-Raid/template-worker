use std::str::FromStr;

use rand::Rng;

const N: usize = 25;
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct StreamId([u8; N]); // 8 bytes for worker_id (big-endian), rest for id

impl StreamId {
    /// Generates a new StreamId given worker_id and unique_id (entropy)
    pub fn new(worker_id: u64, unique_id: [u8; N-8]) -> Self {
        let mut bytes = [0u8; N];
        
        // <worker_id><unique_id>
        bytes[0..8].copy_from_slice(&worker_id.to_be_bytes());
        bytes[8..N].copy_from_slice(&unique_id);
        
        Self(bytes)
    }

    /// Generates a new StreamId with random unique ID (see gen_unique_id)
    pub fn new_rand(worker_id: u64) -> Self {
        Self::new(worker_id, Self::gen_unique_id())
    }

    pub fn gen_unique_id() -> [u8; N-8] {
        let mut rng = rand::rng();
        let mut unique_id = [0u8; N-8];
        rng.fill(&mut unique_id);
        unique_id        
    }

    pub fn worker_id(&self) -> u64 {
        let bytes: [u8; 8] = self.0[0..8].try_into().unwrap();
        u64::from_be_bytes(bytes)
    }

    /// Extracts the remaining ENTROPY (N-8) bytes representing the unique stream
    pub fn unique_id(&self) -> &[u8] {
        &self.0[8..N]
    }

    /// Creates a StreamId from a &[u8]
    pub fn try_from_slice(id: &[u8]) -> Option<Self> {
        if id.len() != N { return None }
        return Some(Self(id.try_into().unwrap()));
    }

    /// Convert StreamId to a Vec<u8>
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    pub fn to_hex_array(&self) -> [u8; N*2] {
        let mut s: [u8; N*2] = [0; N*2];
        hex::encode_to_slice(self.0, &mut s).unwrap();
        s
    }

    pub fn to_hex_str(&self) -> String {
        let mut s: [u8; N*2] = [0; N*2];
        hex::encode_to_slice(self.0, &mut s).unwrap();

        // SAFETY: the hex will always be utf8
        str::from_utf8(&s).unwrap().to_string()
    }
}

impl FromStr for StreamId {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != N * 2 {
            return Err("invalid stream id [invalid size]")
        }
        let mut bytes: [u8; N] = [0; N];
        match hex::decode_to_slice(s, &mut bytes) {
            Ok(_) => {
                Ok(Self(bytes))
            },
            Err(_) => {
                Err("invalid stream id [decode error]")
            }
        }
    }
}