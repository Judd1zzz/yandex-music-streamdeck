use std::io::Cursor;

use anyhow::{Result, anyhow};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct Pcm {
    pub interleaved: Vec<f32>,
    pub channels: usize,
    pub rate: u32,
}

pub fn decode_all(bytes: Vec<u8>, ext_hint: &str) -> Result<Pcm> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if !ext_hint.is_empty() {
        hint.with_extension(ext_hint);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| anyhow!("формат не распознан: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("аудиодорожка не найдена"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow!("кодек не поддерживается: {e}"))?;

    let mut pcm: Option<Pcm> = None;
    let mut sbuf: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(anyhow!("чтение потока: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
            Err(e) => return Err(anyhow!("декодирование: {e}")),
        };
        let spec = *decoded.spec();
        let frames = decoded.capacity();
        let out = pcm.get_or_insert_with(|| Pcm {
            interleaved: Vec::new(),
            channels: spec.channels.count(),
            rate: spec.rate,
        });
        let need = frames * spec.channels.count();
        let recreate = sbuf.as_ref().is_none_or(|b| b.capacity() < need);
        if recreate {
            sbuf = Some(SampleBuffer::<f32>::new(frames as u64, spec));
        }
        let buf = sbuf.as_mut().expect("sample buffer");
        buf.copy_interleaved_ref(decoded);
        out.interleaved.extend_from_slice(buf.samples());
    }
    pcm.filter(|p| !p.interleaved.is_empty() && p.channels > 0 && p.rate > 0)
        .ok_or_else(|| anyhow!("пустой аудиопоток — файл повреждён?"))
}
