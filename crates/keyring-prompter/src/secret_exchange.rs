use aes::Aes128;
use base64::prelude::*;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockEncryptMut, KeyIvInit};
use crypto_bigint::modular::{MontyForm, MontyParams};
use crypto_bigint::{NonZero, Odd, U1536};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroizing;

const SECTION: &str = "[sx-aes-1]";

const PRIME: U1536 = U1536::from_be_hex(concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD1",
    "29024E088A67CC74020BBEA63B139B22514A08798E3404DD",
    "EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245",
    "E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED",
    "EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE45B3D",
    "C2007CB8A163BF0598DA48361C55D39A69163FA8FD24CF5F",
    "83655D23DCA3AD961C62F356208552BB9ED529077096966D",
    "670C354E4ABC9804F1746C08CA237327FFFFFFFFFFFFFFFF",
));

fn modpow(base: &U1536, exponent: &U1536) -> U1536 {
    let params = MontyParams::new(Odd::new(PRIME).expect("the DH modulus is odd"));
    MontyForm::new(base, params).pow(exponent).retrieve()
}

fn to_minimal_be(value: &U1536) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    bytes[start..].to_vec()
}

fn load(bytes: &[u8]) -> Option<U1536> {
    let width = U1536::BYTES;
    if bytes.len() > width {
        return None;
    }
    let mut buf = [0u8; U1536::BYTES];
    buf[width - bytes.len()..].copy_from_slice(bytes);
    Some(U1536::from_be_slice(&buf))
}

#[derive(Clone)]
pub struct SecretExchange {
    private: U1536,
    public: U1536,
}

impl SecretExchange {
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; U1536::BYTES]);
        rand::thread_rng().fill_bytes(&mut *bytes);
        let modulus =
            NonZero::new(PRIME.wrapping_sub(&U1536::from_u8(3))).expect("p - 3 is nonzero");
        let private = (U1536::from_be_slice(&*bytes) % modulus).wrapping_add(&U1536::from_u8(2));
        let public = modpow(&U1536::from_u8(2), &private);
        SecretExchange { private, public }
    }

    pub fn public_message(&self) -> String {
        format!(
            "{SECTION}\npublic={}\n",
            BASE64_STANDARD.encode(to_minimal_be(&self.public))
        )
    }

    pub fn transport_key(&self, peer_message: &str) -> Option<Zeroizing<[u8; 16]>> {
        let peer_public = load(&field(peer_message, "public")?)?;
        let shared = modpow(&peer_public, &self.private);
        let shared_bytes = Zeroizing::new(to_minimal_be(&shared));
        let mut key = Zeroizing::new([0u8; 16]);
        Hkdf::<Sha256>::new(None, &shared_bytes)
            .expand(&[], &mut *key)
            .ok()?;
        Some(key)
    }

    pub fn encrypted_message(&self, key: &[u8; 16], secret: &[u8]) -> String {
        let mut iv = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut iv);
        let cipher_text = cbc::Encryptor::<Aes128>::new_from_slices(key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(secret);
        format!(
            "{SECTION}\npublic={}\nsecret={}\niv={}\n",
            BASE64_STANDARD.encode(to_minimal_be(&self.public)),
            BASE64_STANDARD.encode(&cipher_text),
            BASE64_STANDARD.encode(iv),
        )
    }
}

fn field(message: &str, key: &str) -> Option<Vec<u8>> {
    message.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key)
            .then(|| BASE64_STANDARD.decode(value.trim()).ok())
            .flatten()
    })
}

#[cfg(test)]
fn decrypt(key: &[u8; 16], message: &str) -> Option<Vec<u8>> {
    use cbc::cipher::BlockDecryptMut;

    let iv = field(message, "iv")?;
    let cipher_text = field(message, "secret")?;
    cbc::Decryptor::<Aes128>::new_from_slices(key, &iv)
        .ok()?
        .decrypt_padded_vec_mut::<Pkcs7>(&cipher_text)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_derive_the_same_key_and_secret_round_trips() {
        let server = SecretExchange::generate();
        let client = SecretExchange::generate();

        let server_key = server.transport_key(&client.public_message()).unwrap();
        let client_key = client.transport_key(&server.public_message()).unwrap();
        assert_eq!(*server_key, *client_key);

        let message = server.encrypted_message(&server_key, b"hunter2");
        assert_eq!(decrypt(&client_key, &message).unwrap(), b"hunter2");
    }

    #[test]
    fn rejects_a_message_without_a_public_key() {
        let exchange = SecretExchange::generate();
        assert!(exchange.transport_key("[sx-aes-1]\n").is_none());
    }
}
