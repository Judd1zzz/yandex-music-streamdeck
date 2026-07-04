use anyhow::{Result, anyhow};
use mp3lame_encoder::{Bitrate, Builder, FlushNoGap, InterleavedPcm, MonoPcm, Quality};

use crate::decode::Pcm;

pub fn encode_320(pcm: &Pcm) -> Result<Vec<u8>> {
    if pcm.channels == 0 || pcm.channels > 2 {
        return Err(anyhow!("неподдерживаемое число каналов: {}", pcm.channels));
    }
    let mut b = Builder::new().ok_or_else(|| anyhow!("lame: не удалось инициализировать"))?;
    b.set_sample_rate(pcm.rate).map_err(|e| anyhow!("lame: частота {}: {e:?}", pcm.rate))?;
    b.set_num_channels(pcm.channels as u8).map_err(|e| anyhow!("lame: каналы: {e:?}"))?;
    b.set_brate(Bitrate::Kbps320).map_err(|e| anyhow!("lame: битрейт: {e:?}"))?;
    b.set_quality(Quality::Best).map_err(|e| anyhow!("lame: качество: {e:?}"))?;
    let mut enc = b.build().map_err(|e| anyhow!("lame: сборка энкодера: {e:?}"))?;

    let mut out = Vec::new();
    let chunk_len = (pcm.rate as usize).max(1) * pcm.channels;
    for chunk in pcm.interleaved.chunks(chunk_len) {
        out.reserve(mp3lame_encoder::max_required_buffer_size(chunk.len() / pcm.channels));
        if pcm.channels == 1 {
            enc.encode_to_vec(MonoPcm(chunk), &mut out).map_err(|e| anyhow!("lame: кодирование: {e:?}"))?;
        } else {
            enc.encode_to_vec(InterleavedPcm(chunk), &mut out)
                .map_err(|e| anyhow!("lame: кодирование: {e:?}"))?;
        }
    }
    out.reserve(mp3lame_encoder::max_required_buffer_size(0).max(7200));
    enc.flush_to_vec::<FlushNoGap>(&mut out).map_err(|e| anyhow!("lame: финализация: {e:?}"))?;
    if out.is_empty() {
        return Err(anyhow!("lame: пустой результат"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_pcm(channels: usize, rate: u32, secs: f32) -> Pcm {
        let frames = (rate as f32 * secs) as usize;
        let mut interleaved = Vec::with_capacity(frames * channels);
        for i in 0..frames {
            let s = (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / rate as f32).sin() * 0.5;
            for _ in 0..channels {
                interleaved.push(s);
            }
        }
        Pcm { interleaved, channels, rate }
    }

    #[test]
    fn encodes_stereo_sine_to_mp3() {
        let mp3 = encode_320(&sine_pcm(2, 44100, 0.3)).unwrap();
        assert!(mp3.len() > 4000, "0.3с при 320kbps ≈ 12КБ, получили {}", mp3.len());
        let sync = mp3.windows(2).any(|w| w[0] == 0xFF && w[1] & 0xE0 == 0xE0);
        assert!(sync, "в выводе должен быть mpeg frame sync");
    }

    #[test]
    fn encodes_mono_and_rejects_multichannel() {
        assert!(encode_320(&sine_pcm(1, 44100, 0.2)).is_ok());
        let bad = Pcm { interleaved: vec![0.0; 300], channels: 3, rate: 44100 };
        assert!(encode_320(&bad).is_err());
        let none = Pcm { interleaved: vec![], channels: 0, rate: 44100 };
        assert!(encode_320(&none).is_err());
    }
}
