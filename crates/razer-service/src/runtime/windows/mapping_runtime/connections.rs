//! One physical filter-driver owner shared by device and capture registrations.
use super::{capture, driver, identity};
use anyhow::{Context, Result, ensure};
use razer_device::mapping_native_session::{NativeCapabilitySet, NativeDeviceKind, NativeDiscoveryQuery, NativeGuid, NativeProductIdentity, hooks::RedirectKind};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key { vendor:u16, product:u16, container:Uuid }
impl Key {
    fn new(vendor:u32,product:u32,container:&str)->Result<Self> {
        let key=Self{vendor:vendor.try_into()?,product:product.try_into()?,container:Uuid::parse_str(container.trim_matches(['{','}']))?};
        ensure!(!key.container.is_nil(),"Mapping target has no observed container");Ok(key)
    }
    fn query(&self)->NativeDiscoveryQuery {
        let(a,b,c,d)=self.container.as_fields();
        NativeDiscoveryQuery{container:NativeGuid::new(a,b,c,*d),
            product:NativeProductIdentity::new(self.vendor,self.product,NativeDeviceKind::Unknown),
            verified_capabilities:NativeCapabilitySet::empty()}
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Lease { Device {scope:Uuid,caller:Uuid}, Capture {scope:Uuid,guid:Uuid}, Philips {scope:Uuid,guid:Uuid} }
impl Lease {pub fn scope(&self)->Uuid {match self{Self::Device{scope,..}|Self::Capture{scope,..}|Self::Philips{scope,..}=>*scope}}}
struct Redirect { holders:BTreeSet<Lease>, confirmed:Option<bool> }
struct Entry { driver:driver::Owner, leases:BTreeSet<Lease>, redirects:BTreeMap<u8,Redirect>, notifications:BTreeSet<Lease> }
#[derive(Default)]
pub(super) struct Registry { entries:BTreeMap<Key,Entry>, lease_keys:BTreeMap<Lease,Key> }
pub(super) struct Input {pub leases:Vec<Lease>,pub result:Result<driver::Packet,String>}
fn redirect_id(kind:RedirectKind)->u8 {match kind{RedirectKind::Keyboard=>1,RedirectKind::Mouse=>2,RedirectKind::MouseMove=>4}}
fn redirect_kind(id:u8)->RedirectKind {match id{1=>RedirectKind::Keyboard,2=>RedirectKind::Mouse,4=>RedirectKind::MouseMove,_=>unreachable!()}}
impl Registry {
    fn acquire(&mut self,key:Key,lease:Lease)->Result<driver::Route> {
        ensure!(!self.lease_keys.contains_key(&lease),"Mapping registration already owns a driver lease");
        if !self.entries.contains_key(&key) {
            let observed=super::super::mapping_native_session::discover_mapping_native_routes(key.query())?;
            ensure!(observed.enumeration_complete && observed.failures.is_empty(),"Mapping filter-driver enumeration incomplete: {:?}",observed.failures);
            // 119de6 appends in SetupDi index order; acbc0 uses sentinel.next.
            // Sorting paths or selecting a different HID interface changes the source route.
            let route=observed.routes.into_iter().next().context("No current mapping filter-driver route matches this container/PID")?;
            let driver=driver::Owner::open(route)?;
            self.entries.insert(key.clone(),Entry{driver,leases:BTreeSet::new(),redirects:BTreeMap::new(),notifications:BTreeSet::new()});
        }
        let entry=self.entries.get_mut(&key).unwrap();
        entry.leases.insert(lease.clone());
        self.lease_keys.insert(lease,key);
        Ok(entry.driver.route())
    }
    pub fn device(&mut self,scope:Uuid,caller:Uuid,target:&identity::Target)->Result<driver::Route> {
        self.acquire(Key::new(target.info.vendor_id,target.info.product_id,&target.info.container_id)?,Lease::Device{scope,caller})
    }
    pub fn capture(&mut self,scope:Uuid,target:&capture::Target)->Result<driver::Route> {
        self.acquire(Key::new(target.vendor_id,target.product_id,&target.container_id)?,Lease::Capture{scope,guid:target.guid})
    }
    pub fn philips(&mut self,scope:Uuid,target:&capture::Target)->Result<driver::Route> {
        self.acquire(Key::new(target.vendor_id,target.product_id,&target.container_id)?,Lease::Philips{scope,guid:target.guid})
    }
    fn entry(&mut self,lease:&Lease)->Result<&mut Entry> {
        let key=self.lease_keys.get(lease).context("Mapping driver registration changed")?;
        self.entries.get_mut(key).context("Mapping driver owner is absent")
    }
    pub fn redirect(&mut self,lease:&Lease,kind:RedirectKind,enabled:bool)->Result<()> {
        let entry=self.entry(lease)?;let id=redirect_id(kind);
        let state=entry.redirects.entry(id).or_insert_with(||Redirect{holders:BTreeSet::new(),confirmed:Some(false)});
        if enabled {
            state.holders.insert(lease.clone());
            if state.confirmed!=Some(true) {
                state.confirmed=None;
                entry.driver.redirect(kind,true)?;
                state.confirmed=Some(true);
            }
        } else if state.holders.contains(lease) {
            if state.holders.len()==1 {
                state.confirmed=None;
                entry.driver.redirect(kind,false)?;
                state.confirmed=Some(false);
            }
            state.holders.remove(lease);
        }
        Ok(())
    }
    pub fn notifications(&mut self,lease:&Lease,enabled:bool)->Result<()> {
        let entry=self.entry(lease)?;
        if enabled {
            entry.notifications.insert(lease.clone());
        } else if entry.notifications.contains(lease) {
            entry.notifications.remove(lease);
        }
        Ok(())
    }
    pub fn inputs(&mut self)->Vec<Input> {
        let mut results=Vec::new();
        for entry in self.entries.values_mut() {
            let events=entry.driver.inputs().unwrap_or_else(|error|vec![Err(format!("{error:#}"))]);
            for result in events {results.push(Input{leases:entry.leases.iter().cloned().collect(),result});}
        }
        results
    }
    /// The caller drains and joins device output tasks before releasing their
    /// lease. Captures can release independently while another lease remains.
    pub fn release(&mut self,lease:&Lease)->Result<()> {
        let Some(key)=self.lease_keys.get(lease).cloned() else{return Ok(());};
        let entry=self.entries.get_mut(&key).context("Mapping driver owner is absent")?;
        let redirects=entry.redirects.iter().filter_map(|(id,state)|state.holders.contains(lease).then_some(*id)).collect::<Vec<_>>();
        let mut errors=Vec::new();
        for id in redirects.into_iter().rev() {
            if let Err(error)=self.redirect(lease,redirect_kind(id),false){errors.push(format!("{error:#}"));}
        }
        if let Err(error)=self.notifications(lease,false){errors.push(format!("{error:#}"));}
        ensure!(errors.is_empty(),"{}",errors.join("; "));
        let entry=self.entries.get_mut(&key).unwrap();
        if entry.leases.len()==1 {
            let result=entry.driver.shutdown();
            self.entries.remove(&key);self.lease_keys.remove(lease);
            result
        } else {entry.leases.remove(lease);self.lease_keys.remove(lease);Ok(())}
    }
    pub fn leases_for(&self,scope:Uuid)->Vec<Lease> {self.lease_keys.keys().filter(|lease|lease.scope()==scope).cloned().collect()}
    pub fn shutdown(&mut self)->Result<()> {
        let mut errors=Vec::new();
        for (_,mut entry) in std::mem::take(&mut self.entries) {if let Err(error)=entry.driver.shutdown(){errors.push(format!("{error:#}"));}}
        self.lease_keys.clear();ensure!(errors.is_empty(),"{}",errors.join("; "));Ok(())
    }
}
