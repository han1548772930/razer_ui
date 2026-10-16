//! OS-owned shared WASAPI playback. Eight discrete PCM channels are passed in
//! their original order; only the OS converts to its selected output format.
use super::{Clip,Output};
use anyhow::ensure;
use windows::Win32::{Media::Audio::*,System::Com::{CLSCTX_ALL,COINIT_MULTITHREADED,CoCreateInstance,CoInitializeEx,CoUninitialize}};

struct Apartment;
impl Apartment {
    fn open()->anyhow::Result<Self>{unsafe{CoInitializeEx(None,COINIT_MULTITHREADED).ok()?;}Ok(Self)}
}
impl Drop for Apartment {fn drop(&mut self){unsafe{CoUninitialize()};}}
pub(super) struct Endpoint {
    // Field drop order releases interfaces before the apartment.
    render:IAudioRenderClient,client:IAudioClient,_device:IMMDevice,
    capacity:u32,align:usize,started:bool,_apartment:Apartment,
}
impl Endpoint {
    pub(super) fn open(clip:&Clip)->anyhow::Result<Self> {
        let apartment=Apartment::open()?;
        let enumerator:IMMDeviceEnumerator=unsafe{CoCreateInstance(&MMDeviceEnumerator,None,CLSCTX_ALL)?};
        // HTML video has no setSinkId in the mounted source. Use the same
        // system default console render endpoint; do not force a vendor sink.
        let device=unsafe{enumerator.GetDefaultAudioEndpoint(eRender,eConsole)?};
        let client:IAudioClient=unsafe{device.Activate(CLSCTX_ALL,None)?};
        let align=clip.channels().checked_mul(2).ok_or_else(||anyhow::anyhow!("PCM alignment overflow"))?;
        let mut format=[0u8;40];
        format[0..2].copy_from_slice(&0xfffeu16.to_le_bytes());
        format[2..4].copy_from_slice(&clip.channels().to_le_bytes());
        format[4..8].copy_from_slice(&clip.sample_rate().to_le_bytes());
        format[8..12].copy_from_slice(&clip.sample_rate().checked_mul(u32::from(align)).ok_or_else(||anyhow::anyhow!("PCM byte rate overflow"))?.to_le_bytes());
        format[12..14].copy_from_slice(&align.to_le_bytes());format[14..16].copy_from_slice(&16u16.to_le_bytes());
        format[16..18].copy_from_slice(&22u16.to_le_bytes());format[18..20].copy_from_slice(&16u16.to_le_bytes());
        format[20..24].copy_from_slice(&clip.channel_mask().to_le_bytes());
        // KSDATAFORMAT_SUBTYPE_PCM, little-endian GUID fields.
        format[24..].copy_from_slice(&[1,0,0,0,0,0,16,0,128,0,0,170,0,56,155,113]);
        unsafe{client.Initialize(AUDCLNT_SHAREMODE_SHARED,0x88000000,1_000_000,0,format.as_ptr().cast::<WAVEFORMATEX>(),None)?;}
        let capacity=unsafe{client.GetBufferSize()?};
        ensure!(capacity>0&&capacity<=4_194_304,"Native media render capacity exceeds limit");
        let render=unsafe{client.GetService::<IAudioRenderClient>()?};
        Ok(Self{render,client,_device:device,capacity,align:usize::from(align),started:false,_apartment:apartment})
    }
}
impl Output for Endpoint {
    fn capacity(&self)->u32 {self.capacity}
    fn padding(&self)->anyhow::Result<u32>{Ok(unsafe{self.client.GetCurrentPadding()?})}
    fn write(&mut self,pcm:&[u8],frames:u32)->anyhow::Result<()> {
        ensure!(frames<=self.capacity&&pcm.len()==frames as usize*self.align,"Native media frame count differs");
        let buffer=unsafe{self.render.GetBuffer(frames)?};
        let result=if buffer.is_null(){Err(anyhow::anyhow!("Native media render buffer is null"))}else{
            unsafe{std::ptr::copy_nonoverlapping(pcm.as_ptr(),buffer,pcm.len());}Ok(())
        };
        // Release every acquired packet, including invalid-pointer failures.
        let cleanup=unsafe{self.render.ReleaseBuffer(frames,if result.is_ok(){0}else{2})}.map_err(anyhow::Error::from);
        result.and(cleanup)
    }
    fn start(&mut self)->anyhow::Result<()> {
        if !self.started {unsafe{self.client.Start()?;}self.started=true;}Ok(())
    }
    fn stop(&mut self)->anyhow::Result<()> {
        if self.started {unsafe{self.client.Stop()?;}self.started=false;}Ok(())
    }
    fn reset(&mut self)->anyhow::Result<()> {unsafe{self.client.Reset()?;}Ok(())}
}
impl Drop for Endpoint {fn drop(&mut self){if let Err(error)=self.stop(){eprintln!("Native media Stop: {error:#}");}}}
