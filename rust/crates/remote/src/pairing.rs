use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::RemoteError;

const SECRET_BYTES: usize = 32;
const PROOF_CONTEXT: &[u8] = b"feishin-remote-v1";
const DEVICE_ID_BYTES: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("not a {SECRET_BYTES}-byte hex value")]
pub struct InvalidSecret(#[source] hex::FromHexError);

macro_rules! secret_type {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name([u8; SECRET_BYTES]);

        impl $name {
            pub fn random() -> Result<Self, RemoteError> {
                random_bytes().map(Self)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                hex::encode(value.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidSecret;

            fn try_from(text: String) -> Result<Self, Self::Error> {
                let mut bytes = [0_u8; SECRET_BYTES];
                hex::decode_to_slice(text, &mut bytes).map_err(InvalidSecret)?;
                Ok(Self(bytes))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(…)"))
            }
        }
    };
}

secret_type!(Nonce);
secret_type!(PairingToken);
secret_type!(Proof);

fn random_bytes() -> Result<[u8; SECRET_BYTES], RemoteError> {
    let mut bytes = [0_u8; SECRET_BYTES];
    getrandom::fill(&mut bytes).map_err(RemoteError::Randomness)?;
    Ok(bytes)
}

pub fn new_device_id() -> String {
    let random = random_bytes().unwrap_or_else(|error| {
        tracing::error!("{error}");
        [0_u8; SECRET_BYTES]
    });
    let bytes: Vec<u8> = random.into_iter().take(DEVICE_ID_BYTES).collect();
    hex::encode(bytes)
}

pub fn proof_of(token: &PairingToken, nonce: &Nonce) -> Proof {
    let mut hasher = Sha256::new();
    hasher.update(PROOF_CONTEXT);
    hasher.update(token.0);
    hasher.update(nonce.0);
    Proof(hasher.finalize().into())
}

pub fn proves(proof: &Proof, token: &PairingToken, nonce: &Nonce) -> bool {
    let expected = proof_of(token, nonce);
    expected
        .0
        .iter()
        .zip(proof.0.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proof_only_matches_its_token_and_nonce() {
        let (token, nonce) = (PairingToken::random().unwrap(), Nonce::random().unwrap());
        let proof = proof_of(&token, &nonce);
        assert!(proves(&proof, &token, &nonce));
        assert!(!proves(&proof, &PairingToken::random().unwrap(), &nonce));
        assert!(!proves(&proof, &token, &Nonce::random().unwrap()));
    }

    #[test]
    fn secrets_travel_as_hex_and_reject_garbage() {
        let token = PairingToken::random().unwrap();
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json.len(), 2 + SECRET_BYTES * 2);
        assert_eq!(serde_json::from_str::<PairingToken>(&json).unwrap(), token);
        assert!(serde_json::from_str::<PairingToken>("\"abcd\"").is_err());
        assert_eq!(format!("{token:?}"), "PairingToken(…)");
        assert_eq!(new_device_id().len(), 24);
        assert_ne!(new_device_id(), new_device_id());
    }
}
