//! Deliberately minimal, offline-only demonstration of the public WAP ABI.
//! No device drivers, host graph, scheduler, DAW or proprietary engine code.
mod wav;

use std::{error::Error, path::{Path,PathBuf}, env};
use wyldtone_plugin_packager::package::load_wap;
use wyldtone_plugin_runtime::wasm_abi::WasmPlugin;

type Result<T>=std::result::Result<T,Box<dyn Error>>;

fn usage() {
    eprintln!("Usage:\n  wap-host inspect PLUGIN.wap\n  wap-host render PLUGIN.wap --wav input.wav --out output.wav [--block 256] [--tail 0.5] [--param id=0.5]...\n  wap-host render PLUGIN.wap --test-tone 110 --duration 2 --rate 48000 --out output.wav [--block 256] [--param id=0.5]...\n\n--param values are normalized (0..1). WAV output is IEEE float32. Offline processing only.");
}

struct Options {
    plugin:PathBuf, input:Option<PathBuf>, tone:Option<f64>,
    output:PathBuf, duration:f64, rate:u32, block:usize, tail:f64,
    params:Vec<(String,f32)>,
}

fn parse(args:&[String])->Result<Options> {
    let plugin=PathBuf::from(args.first().ok_or("Missing WAP filename")?);
    let mut o=Options{plugin,input:None,tone:None,output:PathBuf::new(),duration:2.0,rate:48_000,block:256,tail:0.0,params:Vec::new()};
    let mut i=1;
    while i<args.len() {
        let flag=&args[i];
        i+=1;
        let value=args.get(i).ok_or_else(||format!("Missing value for {flag}"))?;
        match flag.as_str() {
            "--wav"=>o.input=Some(PathBuf::from(value)),
            "--test-tone"=>o.tone=Some(value.parse()?),
            "--out"=>o.output=PathBuf::from(value),
            "--duration"=>o.duration=value.parse()?,
            "--rate"=>o.rate=value.parse()?,
            "--block"=>o.block=value.parse()?,
            "--tail"=>o.tail=value.parse()?,
            "--param"=>{
                let (id,v)=value.split_once('=').ok_or("Expected --param id=normalized_value")?;
                let normalized:f32=v.parse()?;
                if id.is_empty() || !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {return Err("Parameters require id=number in 0..1".into());}
                o.params.push((id.into(),normalized));
            },
            _=>return Err(format!("Unknown argument {flag}").into()),
        }
        i+=1;
    }
    if o.output.as_os_str().is_empty() {return Err("--out is required".into());}
    if o.input.is_some()==o.tone.is_some(){return Err("Choose exactly one of --wav or --test-tone".into());}
    if !(16..=2048).contains(&o.block) {return Err("--block must be 16..=2048 frames".into());}
    if !(8_000..=192_000).contains(&o.rate){return Err("--rate must be 8000..192000 Hz".into());}
    if !(0.01..=30.0).contains(&o.duration) || !o.duration.is_finite() {return Err("--duration must be 0.01..30 seconds".into());}
    if !(0.0..=10.0).contains(&o.tail) || !o.tail.is_finite(){return Err("--tail must be 0..10 seconds".into());}
    if let Some(tone)=o.tone {if !tone.is_finite() || !(20.0..=20_000.0).contains(&tone){return Err("--test-tone frequency must be 20..20000 Hz".into());}}
    if o.output == o.plugin || o.input.as_ref()==Some(&o.output) {return Err("Output must not overwrite the input file or plugin".into());}
    Ok(o)
}

fn render(args:&[String])->Result<()> {
    let opts=parse(args)?;
    if std::fs::metadata(&opts.plugin)?.len()>64*1024*1024 {return Err("WAP package exceeds 64 MiB host limit".into());}
    let wap=load_wap(&opts.plugin)?;
    let engine=WasmPlugin::create_engine()?;
    let mut plugin=WasmPlugin::from_bytes(&engine,&wap.wasm)?;
    let mut params=plugin.load_parameters()?;
    wap.manifest.validate_against(&params)?;
    let buses=plugin.bus_layout_channels()?;
    let (input_layout,output_layout)=buses.unwrap_or((vec![1],vec![1]));
    if input_layout.len()!=1 || output_layout.len()!=1 || input_layout[0]!=output_layout[0] || !(1..=2).contains(&input_layout[0]) {
        return Err("Minimal host supports a single symmetric mono or stereo bus only (no sidechain)".into());
    }
    let channels=input_layout[0];
    let mut audio=if let Some(path)=&opts.input {wav::read(path)?} else {
        let frames=(opts.rate as f64*opts.duration).round() as usize;
        let freq=opts.tone.ok_or("Test tone frequency missing")?;
        let mut samples=Vec::with_capacity(frames*channels);
        for frame in 0..frames {
            let value=(0.25*(2.0*std::f64::consts::PI*freq*(frame as f64)/opts.rate as f64).sin()) as f32;
            for _ in 0..channels { samples.push(value); }
        }
        wav::Audio{sample_rate:opts.rate,channels,samples}
    };
    if audio.channels!=channels {
        return Err(format!("WAV contains {} channel(s), but this plugin declares {}. Convert the WAV first.",audio.channels,channels).into());
    }
    let total_input_frames=audio.samples.len()/channels;
    let tail_frames=(audio.sample_rate as f64*opts.tail).round() as usize;
    let total_frames=total_input_frames.checked_add(tail_frames).ok_or("Sample count overflow")?;
    if total_frames>20_000_000 {return Err("Maximum output is 20 million frames".into());}
    plugin.prepare(audio.sample_rate,channels,opts.block)?;
    plugin.initialize_parameter_defaults(&mut params);
    for (id,value) in opts.params {
        let idx=params.iter().position(|p|p.id==id).ok_or_else(||format!("Unknown parameter id: {id}"))?;
        plugin.plugin_parameter_set.call(&mut plugin.store,(idx as i32,value))?;
    }
    let mut output=Vec::with_capacity(total_frames*channels);
    let has_buses=plugin.plugin_process_buses.clone();
    let offset=usize::try_from(plugin.buffer_ptr).map_err(|_|"Negative plugin buffer pointer")?;
    let mut frame=0usize;
    while frame<total_frames {
        let n=(total_frames-frame).min(opts.block);
        let count=n*channels;
        let buffer_floats=if has_buses.is_some(){count*2}else{count};
        if buffer_floats>16_384 {return Err("Block exceeds WAP shared buffer (16384 floats)".into());}
        let bytes=buffer_floats.checked_mul(4).ok_or("WAP buffer overflow")?;
        let end=offset.checked_add(bytes).ok_or("WAP buffer pointer overflow")?;
        let available=plugin.memory.data_size(&plugin.store);
        if end>available {plugin.memory.grow(&mut plugin.store,((end-available).div_ceil(65_536)) as u64)?;}
        {
            let mem=plugin.memory.data_mut(&mut plugin.store);
            mem[offset..end].fill(0);
            for k in 0..count {
                let pos=frame*channels+k;
                let sample=audio.samples.get(pos).copied().unwrap_or(0.0);
                mem[offset+k*4..offset+k*4+4].copy_from_slice(&sample.to_le_bytes());
            }
        }
        if let Some(ref func)=has_buses {
            func.call(&mut plugin.store,(plugin.buffer_ptr,n as i32))?;
        } else {
            plugin.plugin_process.call(&mut plugin.store,(plugin.buffer_ptr,count as i32))?;
        }
        let mem=plugin.memory.data(&plugin.store);
        let out_start=if has_buses.is_some(){offset+count*4}else{offset};
        for chunk in mem[out_start..out_start+count*4].chunks_exact(4) {
            let value=f32::from_le_bytes(chunk.try_into()?);
            if !value.is_finite(){return Err("Plugin produced non-finite audio".into());}
            output.push(value);
        }
        frame+=n;
    }
    audio.samples=output;
    wav::write(&opts.output,&audio)?;
    println!("Rendered {} frames at {} Hz ({} ch) with {} -> {}",total_frames,audio.sample_rate,channels,wap.manifest.plugin.name,opts.output.display());
    Ok(())
}

fn inspect(path:&Path)->Result<()> {
    if std::fs::metadata(path)?.len()>64*1024*1024 {return Err("WAP package exceeds 64 MiB host limit".into());}
    let wap=load_wap(path)?;
    println!("{} {} by {}",wap.manifest.plugin.name,wap.manifest.plugin.version,wap.manifest.plugin.manufacturer);
    println!("ID: {} | Manifest version: {} | Parameters: {}",wap.manifest.plugin.id,wap.manifest.format_version,wap.manifest.parameters.len());
    for p in wap.manifest.parameters {println!("  {} ({})",p.id,p.name);}
    Ok(())
}
fn run()->Result<()> {
    let args:Vec<String>=env::args().skip(1).collect();
    match args.as_slice(){
        [cmd,wap] if cmd=="inspect" => inspect(Path::new(wap)),
        [cmd,rest @ ..] if cmd=="render" => render(rest),
        _ => {usage();Err("Invalid command".into())}
    }
}
fn main(){if let Err(e)=run(){eprintln!("Error: {e}"); std::process::exit(1);}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_output_is_rejected(){
        assert!(parse(&["x.wap".into(),"--test-tone".into(),"110".into()]).is_err());
    }
    #[test]
    fn multiple_sources_are_rejected(){
        assert!(parse(&["x.wap".into(),"--wav".into(),"x.wav".into(),"--test-tone".into(),"110".into(),"--out".into(),"y.wav".into()]).is_err());
    }
}
