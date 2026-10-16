//! Retained Noa interrupt reader. Poll transfers received events without
//! blocking the service dispatcher; Stop/Drop close every interrupt handle.
use anyhow::{Context as _,ensure};
use razer_device::leviathan::{self,EventTarget,Notification};
#[cfg(not(windows))]
use razer_device::receiver_pairing::HardwareEvents;
use serde_json::{Value,json};
use std::{sync::{Arc,atomic::{AtomicBool,Ordering},mpsc::{self,Receiver}},thread::{self,JoinHandle},time::Duration};

struct Active {id:String,canceled:Arc<AtomicBool>,events:Receiver<Notification>,worker:JoinHandle<anyhow::Result<()>>}
#[derive(Default)]
pub(super) struct Controller {active:Option<Active>}
impl Controller {
    pub(super) fn start(&mut self,id:String,target:EventTarget)->anyhow::Result<Value> {
        ensure!(!id.is_empty()&&id.len()<=128&&self.active.is_none(),"Noa event owner is invalid or already active");
        let canceled=Arc::new(AtomicBool::new(false));let signal=canceled.clone();
        let (sender,events)=mpsc::sync_channel(256);
        let worker=thread::Builder::new().name("razer-noa-events".into()).spawn(move || {
            #[cfg(windows)] let mut source=super::native::leviathan_event_source(&target)?;
            #[cfg(not(windows))] let mut source=collection_source(target)?;
            let result=(|| {
                source.begin()?;
                while !signal.load(Ordering::Acquire) {
                    if let Some(bytes)=source.receive(Duration::from_millis(50))?
                        && let Some(event)=leviathan::notification(&bytes)? {
                        sender.try_send(event).context("Noa event consumer is absent or full")?;
                    }
                }
                Ok(())
            })();
            let cleanup=source.finish();
            result.and(cleanup)
        })?;
        self.active=Some(Active{id:id.clone(),canceled,events,worker});
        Ok(json!({"session_id":id,"started":true}))
    }
    pub(super) fn poll(&mut self,id:&str)->anyhow::Result<Value> {
        let active=self.active.as_ref().context("Noa event owner is absent")?;
        ensure!(active.id==id,"Noa event owner differs");
        let mut events=active.events.try_iter().collect::<Vec<_>>();
        let terminal=active.worker.is_finished();
        let error=if terminal {
            let active=self.active.take().unwrap();
            let result=active.worker.join().map_err(|_|anyhow::anyhow!("Noa event worker panicked"))?;
            events.extend(active.events.try_iter());result.err().map(|error|format!("{error:#}"))
        }else{None};
        Ok(json!({"session_id":id,"events":events,"terminal":terminal,"error":error}))
    }
    pub(super) fn stop(&mut self,id:&str)->anyhow::Result<Value> {
        if let Some(active)=&self.active {ensure!(active.id==id,"Noa event stop owner differs");}
        if let Some(active)=self.active.take(){
            active.canceled.store(true,Ordering::Release);
            active.worker.join().map_err(|_|anyhow::anyhow!("Noa event cleanup worker panicked"))??;
        }
        Ok(json!({"session_id":id,"stopped":true}))
    }
}
impl Drop for Controller {
    fn drop(&mut self){if let Some(active)=self.active.take(){active.canceled.store(true,Ordering::Release);let _=active.worker.join();}}
}

#[cfg(not(windows))]
fn collection_source(target:EventTarget)->anyhow::Result<Box<dyn HardwareEvents>> {
    use razer_device::backend::{HidNode,InterruptTransport};
    use razer_hid::with_backend;
    struct Events {node:HidNode,reader:Option<Box<dyn InterruptTransport>>}
    impl HardwareEvents for Events {
        fn begin(&mut self)->anyhow::Result<()> {
            ensure!(self.node.vendor_id==5426&&self.node.product_id==1352,"Noa event collection differs");
            self.reader=Some(with_backend(|backend|backend.open_interrupt(&self.node))?);Ok(())
        }
        fn receive(&mut self,timeout:Duration)->anyhow::Result<Option<Vec<u8>>> {
            let nodes=with_backend(|backend|backend.enumerate())?;
            ensure!(nodes.iter().filter(|node|**node==self.node).count()==1,"Noa event collection changed");
            let reader=self.reader.as_ref().context("Noa event reader is absent")?;
            let length=reader.report_lengths().input.values().max().copied().context("Noa collection has no Input report")?;
            ensure!(length<=100,"Noa input exceeds source native read capacity");
            let mut bytes=vec![0;100];let count=reader.read(&mut bytes,u32::try_from(timeout.as_millis())?)?;
            ensure!(count<=bytes.len(),"Noa interrupt count differs");bytes.truncate(count);
            Ok((count>0).then_some(bytes))
        }
        fn finish(&mut self)->anyhow::Result<()>{self.reader=None;Ok(())}
    }
    match target {
        EventTarget::Collection(node)=>Ok(Box::new(Events{node,reader:None})),
        EventTarget::WindowsContainer(_)=>anyhow::bail!("Windows ContainerId requires its native identity adapter"),
    }
}
