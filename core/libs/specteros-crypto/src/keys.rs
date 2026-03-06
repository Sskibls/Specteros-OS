//! Key management for PhantomKernel OS

use ed25519_dalek::{SigningKey as Ed25519Secret, VerifyingKey as Ed25519Public, Signature};
use x25519_dalek::{StaticSecret as X25519Secret, PublicKey as X25519Public};
use rand::rngs::OsRng;
use zeroize::{Zeroize, ZeroizeOnDrop};
use serde::{Serialize, Deserialize};

/// Root keypair for signing capability tokens
#[derive(Clone, ZeroizeOnDrop)]
pub struct SigningKeypair {
    secret: Ed25519Secret,
    public: Ed25519Public,
}

impl SigningKeypair {
    /// Generate a new random signing keypair
    pub fn generate() -> Self {
        let secret = Ed25519Secret::generate(&mut OsRng);
        let public = Ed25519Public::from(&secret);
        Self { secret, public }
    }

    /// Sign data with the secret key
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.secret.sign(data)
    }

    /// Get the public key
    pub fn public(&self) -> Ed25519Public {
        self.public
    }

    /// Serialize secret key (use with caution)
    pub fn to_bytes(&self) -> [u8; 64] {
        self.secret.to_bytes()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        let secret = Ed25519Secret::from_bytes(&bytes);
        let public = Ed25519Public::from(&secret);
        Self { secret, public }
    }
}

/// Per-shard encryption keypair
#[derive(Clone, ZeroizeOnDrop)]
pub struct EncryptionKeypair {
    secret: X25519Secret,
    public: X25519Public,
}

impl EncryptionKeypair {
    pub fn generate() -> Self {
        let secret = X25519Secret::new(OsRng);
        let public = X25519Public::from(&secret);
        Self { secret, public }
    }

    pub fn public(&self) -> X25519Public {
        self.public
    }

    pub fn derive_shared(&self, peer_public: &X25519Public) -> [u8; 32] {
        self.secret.diffie_hellman(peer_public).to_bytes()
    }
}

/// Key identifiers for audit and lookup
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct KeyId(pub String);

impl KeyId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn for_shard(shard_id: &str) -> Self {
        Self(format!("shard-{}", shard_id))
    }

    pub fn for_service(service: &str) -> Self {
        Self(format!("svc-{}", service))
    }
}

impl Default for KeyId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signing_keypair_sign_verify() {
        let keypair = SigningKeypair::generate();
        let data = b"test capability token";
        let signature = keypair.sign(data);

        // Verify using public key
        assert!(keypair.public.verify(data, &signature).is_ok());
    }

    #[test]
    fn test_signing_keypair_serialization() {
        let keypair = SigningKeypair::generate();
        let bytes = keypair.to_bytes();
        let restored = SigningKeypair::from_bytes(bytes);

        let data = b"test after serialization";
        assert!(restored.public.verify(data, &restored.sign(data)).is_ok());
    }

    #[test]
    fn test_encryption_keypair_dh() {
        let alice = EncryptionKeypair::generate();
        let bob = EncryptionKeypair::generate();

        let shared_alice = alice.derive_shared(&bob.public());
        let shared_bob = bob.derive_shared(&alice.public());

        assert_eq!(shared_alice, shared_bob);
    }

    #[test]
    fn test_key_id_generation() {
        let id1 = KeyId::new();
        let id2 = KeyId::new();
        assert_ne!(id1, id2);

        let shard_id = KeyId::for_shard("work");
        assert!(shard_id.0.starts_with("shard-"));
    }
}
