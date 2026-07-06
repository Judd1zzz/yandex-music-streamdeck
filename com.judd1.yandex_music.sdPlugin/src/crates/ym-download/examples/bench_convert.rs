use std::time::Instant;

use ym_download::decode::{Pcm, decode_all};
use ym_download::mp3_encode::{Quality, encode_320_with};

fn xorshift32(state: &mut u32) -> f32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn synth_pcm(secs: u32) -> Pcm {
    let rate = 44100u32;
    let frames = (rate * secs) as usize;
    let mut interleaved = Vec::with_capacity(frames * 2);
    let mut noise = 0x1234_5678u32;
    let freqs = [220.0f32, 440.0, 1760.0, 5273.0];
    for i in 0..frames {
        let t = i as f32 / rate as f32;
        let mut s = 0.0f32;
        for (k, f) in freqs.iter().enumerate() {
            s += (t * f * 2.0 * std::f32::consts::PI).sin() * (0.15 / (k as f32 + 1.0));
        }
        s += xorshift32(&mut noise) * 0.3;
        let s = s.clamp(-1.0, 1.0) * 0.8;
        interleaved.push(s);
        interleaved.push(-s * 0.9);
    }
    Pcm { interleaved, channels: 2, rate }
}

fn audio_secs(pcm: &Pcm) -> f32 {
    pcm.interleaved.len() as f32 / pcm.channels as f32 / pcm.rate as f32
}

fn bench_encode(pcm: &Pcm) {
    let secs = audio_secs(pcm);
    for (name, q) in [("q0", Quality::Best), ("q2", Quality::NearBest), ("q3", Quality::VeryNice)] {
        let t = Instant::now();
        let mp3 = encode_320_with(pcm, q).expect("encode");
        let wall = t.elapsed().as_secs_f32();
        println!("encode {name}: {wall:.2}с, {:.1}x realtime, {} байт", secs / wall, mp3.len());
    }
}

fn main() {
    match std::env::args().nth(1) {
        None => {
            let pcm = synth_pcm(60);
            println!("вход: синтетика {:.0}с stereo 44100", audio_secs(&pcm));
            bench_encode(&pcm);
        }
        Some(path) => {
            let bytes = std::fs::read(&path).expect("чтение входного файла");
            let size = bytes.len();
            let ext = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("");
            let hint = if ext.eq_ignore_ascii_case("m4a") { "mp4" } else { ext };
            let t = Instant::now();
            let pcm = decode_all(bytes, hint).expect("декодирование");
            let dec = t.elapsed().as_secs_f32();
            let secs = audio_secs(&pcm);
            println!("вход: {path} ({size} байт, {secs:.0}с аудио)");
            println!("decode: {dec:.2}с, {:.1}x realtime", secs / dec);
            bench_encode(&pcm);
        }
    }
}
