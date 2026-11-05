//! Minimal self-contained RIFF/WAVE I/O for the offline reference host.
//! Supported input: PCM 16/24/32-bit or IEEE float32 (mono/stereo).
//! Output: IEEE float32 WAV. No external audio libraries are required.
use std::{error::Error, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Audio {
    pub sample_rate: u32,
    pub channels: usize,
    pub samples: Vec<f32>, // interleaved
}

fn u16le(s: &[u8]) -> u16 { u16::from_le_bytes([s[0],s[1]]) }
fn u32le(s: &[u8]) -> u32 { u32::from_le_bytes([s[0],s[1],s[2],s[3]]) }

pub fn read(path: &Path) -> Result<Audio> {
    const MAX_FILE: u64 = 256 * 1024 * 1024;
    if fs::metadata(path)?.len() > MAX_FILE { return Err("WAV input exceeds the 256 MiB offline limit".into()); }
    let bytes=fs::read(path)?;
    if bytes.len()<44 || &bytes[0..4]!=b"RIFF" || &bytes[8..12]!=b"WAVE" {
        return Err("Expected an uncompressed RIFF/WAVE file".into());
    }
    let riff_end=(u32le(&bytes[4..8]) as usize).checked_add(8).ok_or("RIFF length overflow")?;
    if riff_end>bytes.len() {return Err("Truncated WAV RIFF data".into());}
    let (mut fmt, mut data)=(None,None);
    let mut cursor=12usize;
    while cursor+8<=riff_end {
        let id=&bytes[cursor..cursor+4];
        let len=u32le(&bytes[cursor+4..cursor+8]) as usize;
        let start=cursor+8;
        let end=start.checked_add(len).ok_or("Chunk overflow")?;
        if end>riff_end {return Err("Truncated WAV chunk".into());}
        if id==b"fmt " {fmt=Some(&bytes[start..end]);}
        if id==b"data" {data=Some(&bytes[start..end]);}
        cursor=end.checked_add(len%2).ok_or("Chunk padding overflow")?;
    }
    let fmt=fmt.ok_or("Missing WAV fmt chunk")?;
    let data=data.ok_or("Missing WAV data chunk")?;
    if fmt.len()<16 {return Err("Invalid WAV fmt chunk".into());}
    let codec=u16le(&fmt[0..2]);
    let channels=u16le(&fmt[2..4]) as usize;
    let sample_rate=u32le(&fmt[4..8]);
    let block_align=u16le(&fmt[12..14]) as usize;
    let bits=u16le(&fmt[14..16]);
    if !(1..=2).contains(&channels) {return Err("Minimal host supports mono or stereo WAV only".into());}
    if !(8_000..=192_000).contains(&sample_rate) {return Err("Unsupported WAV sample rate (8000..192000 Hz)".into());}
    let sample_bytes=(bits as usize).div_ceil(8);
    if !matches!((codec,bits), (1,16)|(1,24)|(1,32)|(3,32)) {
        return Err("Supported WAV formats: PCM 16/24/32-bit, IEEE float32; convert other formats first".into());
    }
    if block_align!=sample_bytes*channels || data.len()%block_align!=0 {
        return Err("Invalid WAV block alignment or truncated frame".into());
    }
    let mut samples=Vec::with_capacity(data.len()/sample_bytes);
    for bytes in data.chunks_exact(sample_bytes) {
        let value=match (codec,bits) {
            (1,16)=>i16::from_le_bytes(bytes.try_into()?) as f32 / 32768.0,
            (1,24)=>{
                let raw=i32::from(bytes[0]) | (i32::from(bytes[1]) << 8) | (i32::from(bytes[2]) << 16);
                let signed=(raw<<8)>>8;
                signed as f32/8_388_608.0
            },
            (1,32)=>i32::from_le_bytes(bytes.try_into()?) as f32/2_147_483_648.0,
            (3,32)=>f32::from_le_bytes(bytes.try_into()?),
            _=>unreachable!(),
        };
        if !value.is_finite() { return Err("WAV contains NaN or infinity".into()); }
        samples.push(value);
    }
    Ok(Audio{sample_rate,channels,samples})
}

pub fn write(path:&Path, audio:&Audio)->Result<()> {
    if audio.channels==0 || audio.channels>2 || audio.samples.len()%audio.channels!=0 {return Err("Invalid WAV channel count or sample count".into());}
    if !audio.samples.iter().all(|v|v.is_finite()) {return Err("Cannot save NaN/Inf audio".into());}
    let size=audio.samples.len().checked_mul(4).ok_or("WAV output overflow")?;
    let size32=u32::try_from(size).map_err(|_|"Output WAV larger than 4GiB; RF64 unsupported")?;
    let riff_size=size32.checked_add(36).ok_or("RIFF output too large")?;
    if let Some(parent)=path.parent() {
        if !parent.as_os_str().is_empty() { fs::create_dir_all(parent)?; }
    }
    // IEEE-float WAV carries a fact chunk with the sample-frame count.
    let frames32=u32::try_from(audio.samples.len()/audio.channels)
        .map_err(|_|"Too many WAV sample frames")?;
    let riff_size=riff_size.checked_add(12).ok_or("RIFF output too large")?;
    let mut out=Vec::with_capacity(size+56);
    out.extend_from_slice(b"RIFF");out.extend_from_slice(&riff_size.to_le_bytes());out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&(audio.channels as u16).to_le_bytes());
    out.extend_from_slice(&audio.sample_rate.to_le_bytes());
    out.extend_from_slice(&(audio.sample_rate*audio.channels as u32*4).to_le_bytes());
    out.extend_from_slice(&((audio.channels*4) as u16).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"fact");out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&frames32.to_le_bytes());
    out.extend_from_slice(b"data");out.extend_from_slice(&size32.to_le_bytes());
    for &sample in &audio.samples {out.extend_from_slice(&sample.to_le_bytes());}
    fs::write(path,out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_roundtrip() {
        let dir=std::env::temp_dir().join(format!("wap-host-wav-{}",std::process::id()));
        let a=Audio{sample_rate:48_000,channels:2,samples:vec![0.0,0.25,-0.5,1.0]};
        write(&dir,&a).unwrap();
        let b=read(&dir).unwrap();
        fs::remove_file(&dir).unwrap();
        assert_eq!((b.sample_rate,b.channels),(a.sample_rate,a.channels));
        assert_eq!(b.samples,a.samples);
    }
    #[test]
    fn rejects_non_wav() {
        let dir=std::env::temp_dir().join(format!("wap-host-bad-{}",std::process::id()));
        fs::write(&dir,b"not audio").unwrap();
        assert!(read(&dir).is_err());fs::remove_file(dir).unwrap();
    }
}
