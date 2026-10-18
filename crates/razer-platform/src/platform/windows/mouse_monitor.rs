//! The native RawInput window and registration never leave their I/O thread.
use crate::mouse_monitor::{Event, Sample};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, mem::size_of, ptr, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread::{self, JoinHandle}, time::Duration};
use windows_sys::Win32::{Foundation::*, System::LibraryLoader::GetModuleHandleW,
    UI::{Input::*, WindowsAndMessaging::*}};

pub(crate) struct Owner {
    stop: mpsc::Sender<()>,
    events: mpsc::Receiver<Result<Event, String>>,
    overflow: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<()>>>,
    cleanup_failure: Option<String>,
}
impl Owner {
    pub(crate) fn spawn() -> Result<Self> {
        let (stop, stopped)=mpsc::channel();
        let (events, receiver)=mpsc::sync_channel(8192);
        let overflow=Arc::new(AtomicBool::new(false));
        let failed=overflow.clone();
        let (ready, initialized)=mpsc::sync_channel(1);
        let worker=thread::Builder::new().name("razer-mouse-monitor".into()).spawn(move || {
            let mut window=match Window::open() {
                Ok(window)=>window,
                Err(error)=>{let _=ready.send(Err(format!("{error:#}"))); return Err(error);}
            };
            let _=ready.send(Ok(()));
            let mut devices=BTreeMap::new();
            loop {
                match stopped.try_recv() {
                    Ok(())|Err(mpsc::TryRecvError::Disconnected)=>break,
                    Err(mpsc::TryRecvError::Empty)=>{}
                }
                let mut message=MSG::default();
                // Bound each pump so high-rate movement cannot starve stop.
                for _ in 0..256 {
                    if unsafe{PeekMessageW(&mut message,window.hwnd,0,0,PM_REMOVE)}==0 {break;}
                    let result=match message.message {
                        WM_INPUT=>input(message.lParam as HRAWINPUT,&mut devices),
                        WM_INPUT_DEVICE_CHANGE if message.wParam==GIDC_REMOVAL as usize=>Ok(devices.remove(&(message.lParam as usize)).map(Event::Removed)),
                        WM_INPUT_DEVICE_CHANGE if message.wParam==GIDC_ARRIVAL as usize=>{
                            devices.remove(&(message.lParam as usize));
                            Ok(None)
                        }
                        _=>Ok(None),
                    };
                    match result {
                        Ok(Some(event))=>{if events.try_send(Ok(event)).is_err(){failed.store(true,Ordering::Release);}}
                        Err(error)=>{if events.try_send(Err(format!("{error:#}"))).is_err(){failed.store(true,Ordering::Release);}}
                        Ok(None)=>{}
                    }
                    // DefWindowProc releases foreground raw input resources.
                    unsafe{TranslateMessage(&message); DispatchMessageW(&message);}
                }
                match stopped.recv_timeout(Duration::from_millis(1)) {
                    Ok(())|Err(mpsc::RecvTimeoutError::Disconnected)=>break,
                    Err(mpsc::RecvTimeoutError::Timeout)=>{}
                }
            }
            window.close()
        })?;
        match initialized.recv().context("Mouse monitor initialization disconnected")? {
            Ok(())=>Ok(Self{stop,events:receiver,overflow,worker:Some(worker),cleanup_failure:None}),
            Err(error)=>{let _=worker.join(); anyhow::bail!(error);}
        }
    }
    pub(crate) fn events(&self)->Result<Vec<Event>> {
        self.drain(false)
    }
    pub(crate) fn final_events(&self)->Result<Vec<Event>> {
        ensure!(self.worker.is_none(),"Mouse monitor is still running");
        self.drain(true)
    }
    fn drain(&self,stopped:bool)->Result<Vec<Event>> {
        ensure!(!self.overflow.load(Ordering::Acquire),"Mouse input monitor queue overflowed");
        let mut events=Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(Ok(event))=>events.push(event),
                Ok(Err(error))=>anyhow::bail!(error),
                Err(mpsc::TryRecvError::Empty)=>return Ok(events),
                Err(mpsc::TryRecvError::Disconnected) if stopped=>return Ok(events),
                Err(mpsc::TryRecvError::Disconnected)=>anyhow::bail!("Mouse input monitor worker disconnected"),
            }
        }
    }
    pub(crate) fn shutdown(&mut self)->Result<()> {
        if let Some(worker)=self.worker.take() {
            let _=self.stop.send(());
            let result=worker.join().map_err(|_|anyhow::anyhow!("Mouse monitor cleanup panicked")).and_then(|result|result);
            if let Err(error)=result {self.cleanup_failure=Some(format!("{error:#}"));}
        }
        if let Some(error)=&self.cleanup_failure {anyhow::bail!("{error}");}
        Ok(())
    }
}
impl Drop for Owner { fn drop(&mut self){if let Err(error)=self.shutdown(){eprintln!("Mouse monitor cleanup: {error:#}");}} }

struct Window { hwnd:HWND, module:HMODULE, class:Vec<u16>, registered:bool }
impl Window {
    fn open()->Result<Self> {
        let module=unsafe{GetModuleHandleW(ptr::null())};
        ensure!(!module.is_null(),"GetModuleHandleW: {}",std::io::Error::last_os_error());
        let class="RazerLocalMouseInputMonitor\0".encode_utf16().collect::<Vec<_>>();
        let definition=WNDCLASSW{lpfnWndProc:Some(DefWindowProcW),hInstance:module,lpszClassName:class.as_ptr(),..Default::default()};
        ensure!(unsafe{RegisterClassW(&definition)}!=0,"Register mouse monitor class: {}",std::io::Error::last_os_error());
        let hwnd=unsafe{CreateWindowExW(0,class.as_ptr(),class.as_ptr(),0,0,0,0,0,HWND_MESSAGE,ptr::null_mut(),module,ptr::null())};
        if hwnd.is_null(){let error=std::io::Error::last_os_error(); unsafe{UnregisterClassW(class.as_ptr(),module);}; return Err(error).context("Create mouse input window");}
        let mut window=Self{hwnd,module,class,registered:false};
        let device=RAWINPUTDEVICE{usUsagePage:1,usUsage:2,dwFlags:RIDEV_INPUTSINK|RIDEV_DEVNOTIFY,hwndTarget:hwnd};
        if unsafe{RegisterRawInputDevices(&device,1,size_of::<RAWINPUTDEVICE>() as u32)}==0 {
            let error=std::io::Error::last_os_error();
            let cleanup=window.close();
            anyhow::bail!("Register mouse RawInput: {error}; cleanup: {cleanup:?}");
        }
        window.registered=true;
        Ok(window)
    }
    fn close(&mut self)->Result<()> {
        let mut errors=Vec::new();
        if self.registered {
            let device=RAWINPUTDEVICE{usUsagePage:1,usUsage:2,dwFlags:RIDEV_REMOVE,hwndTarget:ptr::null_mut()};
            if unsafe{RegisterRawInputDevices(&device,1,size_of::<RAWINPUTDEVICE>() as u32)}==0 {errors.push(format!("Unregister mouse RawInput: {}",std::io::Error::last_os_error()));}
            else {self.registered=false;}
        }
        if !self.hwnd.is_null() {
            if unsafe{DestroyWindow(self.hwnd)}==0 {errors.push(format!("Destroy mouse input window: {}",std::io::Error::last_os_error()));}
            else {self.hwnd=ptr::null_mut();}
        }
        if self.hwnd.is_null() && !self.class.is_empty() {
            if unsafe{UnregisterClassW(self.class.as_ptr(),self.module)}==0 {errors.push(format!("Unregister mouse input class: {}",std::io::Error::last_os_error()));}
            else {self.class.clear();}
        }
        ensure!(errors.is_empty(),"{}",errors.join("; ")); Ok(())
    }
}
impl Drop for Window {fn drop(&mut self){let _=self.close();}}

fn device_path(handle:HANDLE)->Result<String> {
    let mut length=0;
    ensure!(unsafe{GetRawInputDeviceInfoW(handle,RIDI_DEVICENAME,ptr::null_mut(),&mut length)}==0,"Read RawInput name length: {}",std::io::Error::last_os_error());
    ensure!((1..=32768).contains(&length),"Invalid RawInput device name length");
    let mut path=vec![0u16;length as usize+1];
    let copied=unsafe{GetRawInputDeviceInfoW(handle,RIDI_DEVICENAME,path.as_mut_ptr().cast(),&mut length)};
    ensure!(copied!=u32::MAX && (copied as usize)<path.len(),"Read RawInput device name: {}",std::io::Error::last_os_error());
    let end=path.iter().position(|value|*value==0).context("Unterminated RawInput device name")?;
    let path=String::from_utf16(&path[..end])?;
    ensure!(!path.is_empty(),"Empty RawInput device name");
    Ok(path)
}
fn input(handle:HRAWINPUT,devices:&mut BTreeMap<usize,String>)->Result<Option<Event>> {
    let mut length=0;
    ensure!(unsafe{GetRawInputData(handle,RID_INPUT,ptr::null_mut(),&mut length,size_of::<RAWINPUTHEADER>() as u32)}!=u32::MAX,"Read RawInput size: {}",std::io::Error::last_os_error());
    ensure!((size_of::<RAWINPUTHEADER>()..=1024*1024).contains(&(length as usize)),"Invalid RawInput byte count");
    let mut storage=vec![0u64;(length as usize).div_ceil(size_of::<u64>())];
    let copied=unsafe{GetRawInputData(handle,RID_INPUT,storage.as_mut_ptr().cast(),&mut length,size_of::<RAWINPUTHEADER>() as u32)};
    ensure!(copied!=u32::MAX && copied as usize<=storage.len()*size_of::<u64>() && copied as usize>=size_of::<RAWINPUTHEADER>(),"Read RawInput body failed or changed length");
    // Storage is eight-byte aligned and every accessed field is within copied.
    let header=unsafe{ptr::read(storage.as_ptr().cast::<RAWINPUTHEADER>())};
    if header.dwType!=RIM_TYPEMOUSE || header.hDevice.is_null(){return Ok(None);}
    ensure!(copied as usize>=size_of::<RAWINPUTHEADER>()+size_of::<RAWMOUSE>(),"Truncated RawInput mouse record");
    let mouse=unsafe{ptr::read(storage.as_ptr().cast::<u8>().add(size_of::<RAWINPUTHEADER>()).cast::<RAWMOUSE>())};
    let buttons=unsafe{mouse.Anonymous.Anonymous};
    if mouse.lLastX==0 && mouse.lLastY==0 && buttons.usButtonFlags==0 && buttons.usButtonData==0 {return Ok(None);}
    let path=match devices.get(&(header.hDevice as usize)) {Some(path)=>path.clone(),None=>{
        let path=device_path(header.hDevice)?; devices.insert(header.hDevice as usize,path.clone()); path
    }};
    let tick=crate::event_clock::tick()?;
    Ok(Some(Event::Input(Sample{path,x:mouse.lLastX,y:mouse.lLastY,button_flags:buttons.usButtonFlags,
        button_data:buttons.usButtonData,modifiers:super::global_shortcuts::side_modifiers(),time_tick:tick})))
}
