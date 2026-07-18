use std::str::FromStr;

use khronos_ext::mluau_ext::prelude::*;
use rand::{CryptoRng, Rng, rngs::ThreadRng};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
        // StreamId is completely unsafe if rand::rng (ThreadRng) is not cryptographically secure, assert this at comptime
        const _: () = {
            const fn assert_crypto_rng<T: CryptoRng>() {}
            assert_crypto_rng::<ThreadRng>();
        };

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

    pub fn with_hex_str<R>(&self, cb: impl FnOnce(&str) -> R) -> R {
        let harr = self.to_hex_array();
        // SAFETY: Hex encoding only produces valid ASCII (UTF-8 compatible) bytes.
        let harr_str = unsafe { std::str::from_utf8_unchecked(&harr) };
        cb(harr_str)
    }
}

impl FromLua for StreamId {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let LuaValue::String(s) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "StreamId".to_string(),
                message: Some("expected a string".to_string()),
            })
        };

        Self::try_from_slice(&s.as_bytes()).ok_or_else(|| LuaError::external("failed to convert to stream id"))
    }
}

impl IntoLua for StreamId {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_string(&self.to_hex_array()).map(LuaValue::String)
    }
}

impl std::fmt::Display for StreamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with_hex_str(|s| f.write_str(s))
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

impl Serialize for StreamId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.with_hex_str(move |s| serializer.serialize_str(s))
    }
}

impl<'de> Deserialize<'de> for StreamId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SidVisitor;
        impl<'de> serde::de::Visitor<'de> for SidVisitor {
            type Value = StreamId;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a {}-character hex string", N * 2)
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<StreamId, E> {
                StreamId::from_str(v).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(SidVisitor)
    }
}