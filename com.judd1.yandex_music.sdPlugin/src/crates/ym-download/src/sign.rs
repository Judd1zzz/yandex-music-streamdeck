use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const SECRET: &str = "kzqU4XhfCaY6B6JTHODeq5";

type HmacSha256 = Hmac<Sha256>;

pub fn sign(secret: &str, sign_str: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(sign_str.as_bytes());
    let b64 = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
    b64[..b64.len().saturating_sub(1)].to_owned()
}

pub fn file_info_sign_str(ts: u64, track_id: &str, quality: &str, codecs: &[&str], transports: &[&str]) -> String {
    format!("{ts}{track_id}{quality}{}{}", codecs.concat(), transports.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_str_concatenates_without_separators() {
        let s = file_info_sign_str(1700000000, "12345", "lossless", &["flac", "aac"], &["encraw"]);
        assert_eq!(s, "170000000012345losslessflacaacencraw");
    }

    #[test]
    fn sign_is_base64_minus_last_char_stable_vector() {
        let s = sign(SECRET, "170000000012345losslessflacaacencraw");
        assert_eq!(s, "OtcJx9+Q9q4NGp/oZjrV1kQuls+FNFCDLyG4Ju0GmRM");
        assert!(!s.ends_with('='));
    }
}
