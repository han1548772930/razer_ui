use super::*;
use razer_device::leviathan::Notification;
use razer_discovery::discovery::ObservedDevice;
use std::{sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},thread,time::Duration};

impl AppShell {
    pub(super) fn start_leviathan_monitor(&mut self,workspace:Entity<ProductWorkspace>,page:Entity<AudioProductWorkspace>,observed:ObservedDevice,guard:Arc<AtomicBool>,cx:&mut Context<Self>) {
        let (sender,events)=mpsc::sync_channel::<Notification>(256);
        let selected=observed.clone();let worker_guard=guard.clone();
        let worker=thread::Builder::new().name("razer-noa-event-client".into()).spawn(move ||->anyhow::Result<()> {
            ensure!(worker_guard.load(Ordering::Acquire),"Noa monitor scope canceled");
            let mut client=ServiceClient::spawn()?;client.set_request_guard(worker_guard.clone());
            let session_id="leviathan-source-sound".to_owned();
            let mut started=false;
            let result=(|| {
                let route=razer_discovery::direct::leviathan::resolve(&mut client,&selected)?;
                let reply=client.request(ServiceRequest::LeviathanEventsStart{session_id:session_id.clone(),target:route.leviathan_event_target()?})?;
                ensure!(reply["session_id"]==session_id&&reply["started"]==true,"Noa monitor start response differs");
                started=true;
                while worker_guard.load(Ordering::Acquire) {
                    let reply=client.request(ServiceRequest::LeviathanEventsPoll{session_id:session_id.clone()})?;
                    ensure!(reply["session_id"]==session_id,"Noa event response owner differs");
                    let notifications:Vec<Notification>=serde_json::from_value(reply["events"].clone())?;
                    for notification in notifications {sender.try_send(notification).context("Noa event consumer is absent or full")?;}
                    if reply["terminal"]==true {
                        if let Some(error)=reply["error"].as_str(){anyhow::bail!("Noa event stream ended: {error}");}
                        break;
                    }
                    thread::sleep(Duration::from_millis(25));
                }
                Ok(())
            })();
            // Cancellation forbids new product operations. Cooperative Stop
            // only releases this reader; Shutdown remains allowed afterwards.
            if !client.is_stopped() {
                client.set_request_guard(Arc::new(AtomicBool::new(true)));
                if started {if let Err(error)=client.request(ServiceRequest::LeviathanEventsStop{session_id}){eprintln!("Noa event stop: {error:#}");}}
                if !client.is_stopped(){let _=client.request(ServiceRequest::Shutdown);}
            }
            result
        });
        let worker=match worker {Ok(worker)=>worker,Err(error)=>{self.status=format!("声音状态监听启动失败：{error}");cx.notify();return;}};
        cx.spawn(async move |this,cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(25)).await;
                let notifications=events.try_iter().collect::<Vec<_>>();
                let current=this.update(cx,|this,cx| {
                    let current=this.devices.contains(&workspace)&&this.device_observations.iter().any(|current|
                        current.matches(workspace.read(cx).device(cx))&&current.physical_product_id()==1352
                        &&current.hid_node()==observed.hid_node()&&current.transport()==observed.transport());
                    if !current {guard.store(false,Ordering::Release);}
                    for event in notifications {page.update(cx,|page,cx|page.observe_leviathan_notification(&guard,event,cx));}
                });
                if current.is_err(){guard.store(false,Ordering::Release);break;}
                if worker.is_finished(){break;}
            }
            let result=cx.background_spawn(async move {worker.join().map_err(|_|anyhow::anyhow!("Noa event client panicked"))?}).await;
            let _=this.update(cx,|this,cx| {
                for event in events.try_iter(){page.update(cx,|page,cx|page.observe_leviathan_notification(&guard,event,cx));}
                if guard.load(Ordering::Acquire)&&let Err(error)=result {this.status=format!("声音状态监听停止：{error:#}");cx.notify();}
            });
        }).detach();
    }
}
