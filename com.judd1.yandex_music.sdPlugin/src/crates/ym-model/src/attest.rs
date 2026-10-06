use anyhow::{anyhow, Context, Result};
use ssh_key::{PublicKey, SshSig};

pub const SIG_NAMESPACE: &str = "file";

pub const SIGNING_KEYS: &[&str] = &[
    "sk-ssh-ed25519@openssh.com AAAAGnNrLXNzaC1lZDI1NTE5QG9wZW5zc2guY29tAAAAIBFXhPYDjtB78qznuYA6ZTDPfOP/zlk4huaokRFcoNXYAAAAB3NzaDpnaXQ=",
    "sk-ssh-ed25519@openssh.com AAAAGnNrLXNzaC1lZDI1NTE5QG9wZW5zc2guY29tAAAAIBi2OUX3KX6jyBjfyTNeDi5TeLWQx5vMboxrbMoAsxvmAAAAEnNzaDpyZWxlYXNlLWJhY2t1cA==",
];

pub fn verify_signed_bytes(message: &[u8], sig_pem: &[u8]) -> Result<()> {
    verify_signed_bytes_with(SIGNING_KEYS, message, sig_pem)
}

pub fn bridge_trusted(path: &std::path::Path) -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    let mut sig = path.as_os_str().to_owned();
    sig.push(".sig");
    match (std::fs::read(path), std::fs::read(std::path::PathBuf::from(sig))) {
        (Ok(payload), Ok(sig)) => verify_signed_bytes(&payload, &sig).is_ok(),
        _ => false,
    }
}

fn verify_signed_bytes_with(keys: &[&str], message: &[u8], sig_pem: &[u8]) -> Result<()> {
    let sig = SshSig::from_pem(sig_pem).context("разбор ssh-подписи payload")?;
    for key in keys {
        let key = PublicKey::from_openssh(key).context("разбор доверенного ключа")?;
        if key.verify(SIG_NAMESPACE, message, &sig).is_ok() {
            return Ok(());
        }
    }
    Err(anyhow!("ssh-подпись не соответствует ни одному доверенному ключу"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFaFOsI3pqY1t5Yp0H91n7ug4UdsnQHIIuxjS5rcGkSE a2-test";
    const TEST_MSG: &[u8] = b"ym-bridge attest fixture payload";
    const TEST_SIG: &[u8] = br#"-----BEGIN SSH SIGNATURE-----
U1NIU0lHAAAAAQAAADMAAAALc3NoLWVkMjU1MTkAAAAgVoU6wjempjW3linQf3Wfu6DhR2
ydAcgi7GNLmtwaRIQAAAAEZmlsZQAAAAAAAAAGc2hhNTEyAAAAUwAAAAtzc2gtZWQyNTUx
OQAAAEAsxWhoSv0IuGSCEOu1WRUDN7gasWh1+Xp0BTEq9QyUni/UW//ZfhcdCy0p1j1hCV
1slxEqDDWGDaMdHG6vdckE
-----END SSH SIGNATURE-----
"#;

    #[test]
    fn accepts_trusted_signature() {
        assert!(verify_signed_bytes_with(&[TEST_KEY], TEST_MSG, TEST_SIG).is_ok());
    }

    #[test]
    fn rejects_tampered_message() {
        assert!(verify_signed_bytes_with(&[TEST_KEY], b"tampered payload", TEST_SIG).is_err());
    }

    #[test]
    fn rejects_when_key_not_trusted() {
        assert!(verify_signed_bytes_with(SIGNING_KEYS, TEST_MSG, TEST_SIG).is_err());
    }
}
