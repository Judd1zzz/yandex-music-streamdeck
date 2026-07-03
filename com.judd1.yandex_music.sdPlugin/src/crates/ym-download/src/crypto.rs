use aes::{Aes128, Aes192, Aes256};
use ctr::cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;

pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    let h = hex.trim();
    if h.is_empty() {
        return None;
    }
    let chunks = h.as_bytes().chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return None;
    }
    chunks.map(|c| u8::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok()).collect()
}

pub fn decrypt_ctr(key_hex: &str, data: &mut [u8]) -> Result<(), String> {
    let key = hex_to_bytes(key_hex).ok_or_else(|| "bad key hex".to_owned())?;
    let iv = [0u8; 16];
    match key.len() {
        16 => {
            let mut c = Ctr128BE::<Aes128>::new_from_slices(&key, &iv).map_err(|e| e.to_string())?;
            c.apply_keystream(data);
        }
        24 => {
            let mut c = Ctr128BE::<Aes192>::new_from_slices(&key, &iv).map_err(|e| e.to_string())?;
            c.apply_keystream(data);
        }
        32 => {
            let mut c = Ctr128BE::<Aes256>::new_from_slices(&key, &iv).map_err(|e| e.to_string())?;
            c.apply_keystream(data);
        }
        n => return Err(format!("unsupported key length {n}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_parsing() {
        assert_eq!(hex_to_bytes("00ff10"), Some(vec![0, 255, 16]));
        assert_eq!(hex_to_bytes("0"), None);
        assert_eq!(hex_to_bytes("zz"), None);
    }

    #[test]
    fn aes128_ctr_iv0_matches_openssl_vector() {
        let key = "000102030405060708090a0b0c0d0e0f";
        let mut buf = crate::crypto::hex_to_bytes("8ec4575be8a37bdb0e21e507d9e8950c002f70b4").unwrap();
        decrypt_ctr(key, &mut buf).unwrap();
        assert_eq!(&buf, b"Hello, Yandex Music!");
    }

    #[test]
    fn ctr_is_symmetric_roundtrip() {
        let key = "0f0e0d0c0b0a09080706050403020100";
        let original = b"some raw mp4 bytes spanning >1 block".to_vec();
        let mut buf = original.clone();
        decrypt_ctr(key, &mut buf).unwrap();
        assert_ne!(buf, original);
        decrypt_ctr(key, &mut buf).unwrap();
        assert_eq!(buf, original);
    }
}
