//! OS display order, names and active state for SystemCommon GPU selection.
use crate::hardware_monitor::GraphicsReading;
use std::{mem::size_of, ptr};
use windows_sys::Win32::Graphics::Gdi::{DISPLAY_DEVICEW, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP, EnumDisplayDevicesW};
fn text(value:&[u16])->String {String::from_utf16_lossy(&value[..value.iter().position(|&v|v==0).unwrap_or(value.len())])}
fn identity(text:&str,key:&str)->Option<u32>{let at=text.find(key)?+key.len();u32::from_str_radix(text.get(at..at+4)?,16).ok()}
pub(super) fn annotate(graphics:&mut [GraphicsReading],diagnostics:&mut Vec<String>) {
    let mut displays=Vec::new();
    for index in 0..4096 {
        let mut device=DISPLAY_DEVICEW::default();device.cb=size_of::<DISPLAY_DEVICEW>() as u32;
        // SAFETY: correctly sized owned output and null parent enumerate adapters.
        if unsafe{EnumDisplayDevicesW(ptr::null(),index,&mut device,0)}==0 {break;}
        let id=text(&device.DeviceID).to_ascii_uppercase();
        if let (Some(vendor),Some(product))=(identity(&id,"VEN_"),identity(&id,"DEV_")){displays.push((vendor,product,text(&device.DeviceString),device.StateFlags&DISPLAY_DEVICE_ATTACHED_TO_DESKTOP!=0,index));}
    }
    // Identical PCI IDs alone cannot disambiguate two physical adapters. Their
    // status remains unobserved instead of borrowing another adapter's state.
    let mut counts=std::collections::BTreeMap::new();
    for gpu in graphics.iter() { *counts.entry((gpu.vendor_id,gpu.device_id)).or_insert(0_usize)+=1; }
    for gpu in graphics.iter_mut() {
        let matches:Vec<_>=displays.iter().filter(|entry|entry.0==gpu.vendor_id && entry.1==gpu.device_id).collect();
        if matches.len()==1 && counts[&(gpu.vendor_id,gpu.device_id)]==1 {gpu.name=matches[0].2.clone();gpu.enabled=Some(matches[0].3);}
        else if matches.len()>1 || counts[&(gpu.vendor_id,gpu.device_id)]>1 {gpu.enabled=None;diagnostics.push(format!("Display identity for PCI {:04x}:{:04x} is ambiguous",gpu.vendor_id,gpu.device_id));}
    }
    graphics.sort_by_key(|gpu|displays.iter().find(|entry|entry.0==gpu.vendor_id&&entry.1==gpu.device_id).map_or(u32::MAX,|entry|entry.4));
}
