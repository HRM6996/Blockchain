use secp256k1::{Secp256k1, SecretKey, PublicKey, Message};
use sha2::{Sha256, Digest};
use bip39::{Mnemonic, Language};
use crate::types::Wallet;
use anyhow::Result;

pub struct CryptoUtils;

impl CryptoUtils {
    pub fn generate_mnemonic() -> String {
        use rand::RngCore;
        let mut rng = rand::thread_rng();
        let mut entropy = [0u8; 16];
        rng.fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        mnemonic.to_string()
    }

    pub fn mnemonic_to_wallet(mnemonic: &str) -> Result<Wallet> {
        let mnemonic = Mnemonic::parse_in(Language::English, mnemonic)?;
        let seed = mnemonic.to_seed("");

        let secp = Secp256k1::new();
        let mut hasher = Sha256::new();
        hasher.update(&seed[..32]);
        let hash = hasher.finalize();

        let secret_key = SecretKey::from_slice(&hash)?;
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let address = Self::public_key_to_address(&public_key);

        Ok(Wallet {
            address,
            public_key: hex::encode(public_key.serialize()),
            private_key: hex::encode(secret_key.secret_bytes()),
        })
    }

    pub fn public_key_to_address(public_key: &PublicKey) -> String {
        use ripemd::Ripemd160;
        use ripemd::Digest as RipemdDigest;

        let mut hasher = Sha256::new();
        hasher.update(public_key.serialize());
        let hash = hasher.finalize();

        let mut ripemd = Ripemd160::new();
        ripemd.update(hash);
        let address_hash = ripemd.finalize();

        format!("0x{}", hex::encode(address_hash))
    }

    #[allow(dead_code)]
    pub fn sign_transaction(private_key: &str, data: &str) -> Result<String> {
        let secp = Secp256k1::new();
        let secret_key = SecretKey::from_slice(&hex::decode(private_key)?)?;

        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let hash = hasher.finalize();

        let message = Message::from_digest_slice(&hash)?;
        let signature = secp.sign_ecdsa(&message, &secret_key);

        Ok(hex::encode(signature.serialize_compact()))
    }

    #[allow(dead_code)]
    pub fn verify_signature(public_key: &str, signature: &str, data: &str) -> Result<bool> {
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_slice(&hex::decode(public_key)?)?;

        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let hash = hasher.finalize();

        let message = Message::from_digest_slice(&hash)?;
        let signature = secp256k1::ecdsa::Signature::from_compact(&hex::decode(signature)?)?;

        Ok(secp.verify_ecdsa(&message, &signature, &public_key).is_ok())
    }

    #[allow(dead_code)]
    pub fn calculate_hash(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string().replace("-", "")
    }
}
