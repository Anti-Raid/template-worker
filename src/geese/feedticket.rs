use dapi::UserId;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CONFIG, worker::workervmmanager::Id};

pub const FT_EXPIRATION_SECONDS: u64 = 90; // 90 seconds expiry

// Type alias for convenience
type HmacSha256 = Hmac<Sha256>;

/// Errors that can occur during URL verification
#[derive(Debug, PartialEq)]
pub enum VerifyError {
    Expired,
    InvalidSignature,
}

impl VerifyError {
    pub fn message(&self) -> &'static str {
        match self {
            VerifyError::Expired => "Ticket has expired",
            VerifyError::InvalidSignature => "Invalid ticket signature",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Payload<'a> {
    tenant_type: &'a str,
    tenant_id: &'a str,
    user_id: u64,
    expires_at: u64,
    #[serde(borrow)]
    subscribed_topics: Vec<&'a str>,
}

fn construct_payload(tenant_type: &str, tenant_id: &str, user_id: u64, expires_at: u64, subscribed_topics: &[&str]) -> Result<Vec<u8>, crate::Error> {
    rmp_serde::to_vec(&Payload { tenant_type, tenant_id, user_id, expires_at, subscribed_topics: subscribed_topics.to_vec() })
        .map_err(|e| format!("Failed to serialize payload: {}", e).into())
}

fn decode_payload(payload: &[u8]) -> Result<Payload<'_>, crate::Error> {
    rmp_serde::from_slice(payload)
        .map_err(|e| format!("Failed to deserialize payload: {}", e).into())
}

/// Generates a presigned URL that is valid for `expires_in_seconds`
pub fn create_feedticket(tid: Id, user_id: UserId, subscribed_topics: &[&str]) -> Result<(String, String), crate::Error> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let expires_at = now + FT_EXPIRATION_SECONDS;

    // The payload we are signing
    let ttype = tid.tenant_type();
    let tid = tid.tenant_id();
    let payload = construct_payload(&ttype, &tid, user_id.get(), expires_at, subscribed_topics)?;

    let mut mac = HmacSha256::new_from_slice(CONFIG.blob_token.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(&payload);
    
    let signature = hex::encode(mac.finalize().into_bytes());
    let payload = hex::encode(payload);

    Ok((payload, signature))
}

pub struct FeedTicket {
    pub id: Id,
    pub user_id: UserId,
    pub subscribed_topics: Vec<String>
}

/// Verifies that a given signature is valid and has not expired
pub fn verify_feedticket(provided_payload: &str, provided_signature: &str) -> Result<FeedTicket, VerifyError> {    
    // Decode the payload from hex
    let payload_bytes = hex::decode(provided_payload).map_err(|_| VerifyError::InvalidSignature)?;
    
    // Verify signature before parsing anything
    let mut mac = HmacSha256::new_from_slice(CONFIG.blob_token.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(&payload_bytes);

    let decoded_signature = hex::decode(provided_signature).map_err(|_| VerifyError::InvalidSignature)?;
    
    if mac.verify_slice(&decoded_signature).is_err() {
        return Err(VerifyError::InvalidSignature);
    }
    
    // At this point we know the signature is valid, so we can safely decode the payload
    let payload = decode_payload(&payload_bytes).map_err(|_| VerifyError::InvalidSignature)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Check expiration before returning the decoded payload
    if now > payload.expires_at {
        return Err(VerifyError::Expired);
    }

    Ok(FeedTicket {
        id: Id::from_parts(payload.tenant_type, payload.tenant_id).ok_or(VerifyError::InvalidSignature)?,
        user_id: UserId::new(payload.user_id),
        subscribed_topics: payload.subscribed_topics.into_iter().map(|s| s.to_string()).collect()
    })
}
