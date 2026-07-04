use std::path::Path;

use anyhow::{Result, anyhow};
use symphonia::core::codecs::CODEC_TYPE_FLAC;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const PADDING_LEN: usize = 4096;

pub fn streaminfo_block(extra: &[u8]) -> Vec<u8> {
    let len = extra.len() as u32;
    let mut v = Vec::with_capacity(4 + extra.len());
    v.push(0x00);
    v.extend_from_slice(&len.to_be_bytes()[1..]);
    v.extend_from_slice(extra);
    v
}

pub fn padding_block(len: usize) -> Vec<u8> {
    let mut v = vec![0u8; 4 + len];
    v[0] = 0x81;
    v[1..4].copy_from_slice(&(len as u32).to_be_bytes()[1..]);
    v
}

pub fn flac_from_mp4_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| anyhow!("MP4 не распознан: {e}"))?;
    let mut format = probed.format;
    let Some(track) = format.tracks().iter().find(|t| t.codec_params.codec == CODEC_TYPE_FLAC) else {
        return Ok(None);
    };
    let track_id = track.id;
    let extra = track
        .codec_params
        .extra_data
        .clone()
        .ok_or_else(|| anyhow!("в MP4 нет STREAMINFO (dfLa)"))?;

    let mut out = Vec::with_capacity(64 * 1024);
    out.extend_from_slice(b"fLaC");
    out.extend_from_slice(&streaminfo_block(&extra));
    out.extend_from_slice(&padding_block(PADDING_LEN));
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(anyhow!("чтение FLAC-фреймов из MP4: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        out.extend_from_slice(&packet.data);
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streaminfo_block_header_for_34_bytes() {
        let block = streaminfo_block(&[0u8; 34]);
        assert_eq!(&block[..4], &[0x00, 0x00, 0x00, 0x22]);
        assert_eq!(block.len(), 38);
    }

    #[test]
    fn streaminfo_block_encodes_length_big_endian() {
        let block = streaminfo_block(&[7u8; 300]);
        assert_eq!(block[0], 0x00, "STREAMINFO не последний блок — за ним PADDING");
        assert_eq!(&block[1..4], &[0x00, 0x01, 0x2C]);
        assert_eq!(&block[4..], &[7u8; 300][..]);
    }

    #[test]
    fn padding_block_is_last_and_zeroed() {
        let block = padding_block(4096);
        assert_eq!(block[0], 0x81);
        assert_eq!(&block[1..4], &[0x00, 0x10, 0x00]);
        assert_eq!(block.len(), 4 + 4096);
        assert!(block[4..].iter().all(|b| *b == 0));
    }
}
