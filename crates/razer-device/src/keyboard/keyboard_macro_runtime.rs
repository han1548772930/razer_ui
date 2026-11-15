//! A single keyboard initialization lease owns its macro commands, source
//! catalog projection, queued imports and partial-write receipts.
use super::{Addition, Requirements, Runtime};
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    keyboard_firmware_storage,
    keyboard_onboard_profiles::{CompleteReading, MacroDetail, Reading},
    onboard_macro::{self, KeyboardMacroMemory, MacroFailurePhase, MacroWriteReport, OnboardMacro},
    onboard_macro_codec::CompiledMacro,
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
use std::{
    collections::{BTreeSet, VecDeque},
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
pub enum ImportEvent {
    Macros(Vec<onboard_macro::import::Document>),
    Profiles,
}

#[derive(Clone, Debug, Serialize)]
pub struct MacroEntry {
    pub id: u16,
    pub guid: String,
    pub name: String,
    pub allocated_size: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandReceipt {
    pub deleting: bool,
    pub report: MacroWriteReport,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeviceReceipt {
    pub commands: Vec<CommandReceipt>,
    pub firmware_writes: Vec<keyboard_firmware_storage::WriteReport>,
    pub firmware_errors: Vec<String>,
    /// Source obmJson projection, never an observed device catalog.
    pub projected_catalog: Vec<MacroEntry>,
    pub failed_macros: Vec<(String, String)>,
    pub standby_delete_errors: Vec<String>,
    pub imports: VecDeque<ImportEvent>,
}

pub struct DeviceRuntime<'a, N, G, R, W> {
    device: &'a dyn FeatureTransport,
    cap: &'a DeviceReadCapability,
    next: N,
    guard: G,
    random: R,
    wait: W,
    memory: Option<KeyboardMacroMemory>,
    standby: BTreeSet<u16>,
    /// Only original observations and successfully acknowledged mutations may
    /// feed reconstruction; the source catalog above may include failed writes.
    restoration: CompleteReading,
    restoration_valid: bool,
    pub receipt: DeviceReceipt,
}

impl<'a, N, G, R, W> DeviceRuntime<'a, N, G, R, W>
where
    N: FnMut() -> Result<u8>,
    G: Fn() -> Result<()>,
    R: FnMut() -> Result<u16>,
    W: FnMut(Duration) -> Result<()>,
{
    pub fn new(
        device: &'a dyn FeatureTransport,
        cap: &'a DeviceReadCapability,
        reading: &CompleteReading,
        requirements: &Requirements,
        standby: BTreeSet<u16>,
        next: N,
        guard: G,
        random: R,
        wait: W,
    ) -> Result<Self> {
        reading.snapshot.validate()?;
        ensure!(
            cap.product_id == reading.snapshot.product_id,
            "Macro lease has a foreign observation"
        );
        OnboardMacro::for_current_keyboard(device, cap)?;
        ensure!(
            standby
                .iter()
                .all(|id| reading.snapshot.macros.inventory.ids.contains(id)
                    && !requirements.locked_device_ids.contains(id)),
            "Standby selection is absent or protected"
        );
        ensure!(
            !matches!(cap.product_id, 739 | 747) || standby.is_empty(),
            "This keyboard runner has no standby macro classification"
        );
        let memory = if matches!(cap.product_id, 579 | 600 | 602 | 697) {
            None
        } else {
            Some(KeyboardMacroMemory::observe(
                &reading.snapshot.macros,
                reading.snapshot.inventory.slots.len(),
            )?)
        };
        let projected_catalog = reading
            .snapshot
            .macros
            .macros
            .iter()
            .map(|entry| MacroEntry {
                id: entry.metadata.macro_id,
                guid: entry.metadata.guid.clone(),
                name: entry.metadata.name.clone(),
                allocated_size: entry.allocated_size_with_name,
            })
            .collect();
        Ok(Self {
            device,
            cap,
            next,
            guard,
            random,
            wait,
            memory,
            standby,
            restoration: reading.clone(),
            restoration_valid: true,
            receipt: DeviceReceipt {
                projected_catalog,
                ..Default::default()
            },
        })
    }

    fn interrupted(&self, error: &anyhow::Error) -> bool {
        crate::backend::exchange_interrupted(error) || (self.guard)().is_err()
    }

    fn acknowledge_delete(&mut self, id: u16, report: &MacroWriteReport) -> Result<()> {
        let catalog = &mut self.restoration.snapshot.macros;
        catalog.macros.retain(|entry| entry.metadata.macro_id != id);
        catalog.inventory.ids.retain(|old| *old != id);
        catalog.inventory.count = u16::try_from(catalog.inventory.ids.len())?;
        catalog.inventory.pages.clear();
        if let Some(storage) = &report.storage_after {
            catalog.inventory.storage = storage.clone();
        }
        self.restoration
            .import_details
            .macros
            .retain(|entry| entry.macro_id != id);
        Ok(())
    }

    fn acknowledge_upload(
        &mut self,
        id: u16,
        request: &onboard_macro::MacroWriteRequest,
    ) -> Result<()> {
        let metadata =
            onboard_macro::decode_metadata(id, onboard_macro::encode_metadata(request)?)?;
        let size = u32::try_from(request.data.len())?;
        let catalog = &mut self.restoration.snapshot.macros;
        catalog.macros.retain(|entry| entry.metadata.macro_id != id);
        catalog.inventory.ids.retain(|old| *old != id);
        catalog.macros.push(onboard_macro::MacroSummary {
            metadata: metadata.clone(),
            data_size: size,
            allocated_size_with_name: onboard_macro::required_memory(request.data.len())?,
        });
        catalog.inventory.ids.push(id);
        catalog.inventory.count = u16::try_from(catalog.inventory.ids.len())?;
        catalog.inventory.pages.clear();
        self.restoration
            .import_details
            .macros
            .retain(|entry| entry.macro_id != id);
        self.restoration.import_details.macros.push(MacroDetail {
            macro_id: id,
            content: Reading::Observed(onboard_macro::MacroReading {
                metadata,
                data: request.data.clone(),
                data_size: size,
                data_blocks: vec![],
            }),
        });
        Ok(())
    }

    fn add_once(&mut self, compiled: &CompiledMacro) -> Result<()> {
        (self.guard)()?;
        let ids = self
            .receipt
            .projected_catalog
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        // The source allocator retries random collisions. Cancellation is
        // checked on every attempt, without substituting a deterministic ID.
        let id = loop {
            (self.guard)()?;
            let candidate = (self.random)()? & 0x7fff;
            if !ids.contains(&candidate) {
                break candidate;
            }
        };
        let request = compiled.request();
        let local_resolved;
        let acknowledged;
        let result = if let Some(memory) = &mut self.memory {
            let macros = OnboardMacro::for_current_keyboard(self.device, self.cap)?;
            let result = macros.add_calculated_keyboard_macro(
                id,
                request,
                memory,
                &mut self.next,
                &self.guard,
            );
            let (report, error, resolved, capacity) = match result {
                Ok(report) => (report, None, true, false),
                Err(error) => {
                    let capacity = error.phase == MacroFailurePhase::InsufficientStorage;
                    let resolved = matches!(
                        error.phase,
                        MacroFailurePhase::Allocate
                            | MacroFailurePhase::Metadata
                            | MacroFailurePhase::Data
                    ) && error.progress.storage_after.is_some();
                    (error.progress.clone(), Some(error), resolved, capacity)
                }
            };
            local_resolved = report.local_write_returned;
            acknowledged = report.acknowledged();
            if let Some(storage) = &report.storage_after {
                self.restoration.snapshot.macros.inventory.storage = storage.clone();
            }
            self.receipt.commands.push(CommandReceipt {
                deleting: false,
                report,
                error: error.as_ref().map(ToString::to_string),
            });
            if capacity {
                Err(anyhow::Error::new(CapacityError))
            } else if resolved {
                Ok(())
            } else {
                Err(error.context("Macro upload did not resolve")?.into_error())
            }
        } else {
            let result = keyboard_firmware_storage::write_macro_with_restore_guard(
                self.device,
                self.cap,
                &self.restoration,
                id,
                request,
                self.restoration_valid,
                &mut self.next,
                &self.guard,
                &mut self.wait,
            );
            let (report, error) = match result {
                Ok(report) => (report, None),
                Err(error) => (error.progress.clone(), Some(error)),
            };
            let required = onboard_macro::required_memory(request.data.len())?;
            let capacity = report.storage_before.as_ref().is_some_and(|storage| {
                u64::from(required)
                    > u64::from(storage.available_memory) + u64::from(storage.recyclable_memory)
            }) || report
                .storage_after_rebuild
                .as_ref()
                .is_some_and(|storage| required > storage.available_memory);
            local_resolved = report
                .upload
                .as_ref()
                .is_some_and(|upload| upload.report.local_write_returned);
            acknowledged = report
                .upload
                .as_ref()
                .is_some_and(|upload| upload.report.acknowledged());
            if let Some(backup) = &report.reconstruction {
                if report.format_confirmed()
                    && report
                        .macro_restoration
                        .iter()
                        .all(|entry| entry.report.acknowledged())
                {
                    for saved in &backup.macros {
                        self.acknowledge_upload(
                            saved.reading.metadata.macro_id,
                            &saved.rebuilt.request,
                        )?;
                    }
                } else {
                    self.restoration_valid = false;
                }
            }
            if let Some(storage) = &report.storage_after {
                self.restoration.snapshot.macros.inventory.storage = storage.clone();
            }
            if let Some(error) = &error {
                self.receipt.firmware_errors.push(error.to_string());
            }
            if report.device_state_may_have_changed() && !report.confirmed() {
                self.restoration_valid = false;
            }
            self.receipt.firmware_writes.push(report);
            if capacity {
                Err(anyhow::Error::new(CapacityError))
            } else {
                error.map_or(Ok(()), |error| Err(error.into_error()))
            }
        };
        if local_resolved {
            self.receipt.projected_catalog.push(MacroEntry {
                id,
                guid: request.guid.clone(),
                name: request.name.clone(),
                allocated_size: onboard_macro::required_memory(request.data.len())?,
            });
            if acknowledged {
                self.acknowledge_upload(id, request)?;
            } else {
                self.restoration_valid = false;
            }
        } else if self.receipt.commands.last().is_some_and(|entry| {
            !entry.deleting && entry.report.steps.iter().any(|step| step.attempted)
        }) {
            self.restoration_valid = false;
        }
        result
    }
}

#[derive(Debug)]
struct CapacityError;
impl std::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Insufficient keyboard macro capacity")
    }
}
impl std::error::Error for CapacityError {}

impl<N, G, R, W> Runtime for DeviceRuntime<'_, N, G, R, W>
where
    N: FnMut() -> Result<u8>,
    G: Fn() -> Result<()>,
    R: FnMut() -> Result<u16>,
    W: FnMut(Duration) -> Result<()>,
{
    fn delete_macro(&mut self, id: u16) -> Result<()> {
        (self.guard)()?;
        let Some(entry) = self
            .receipt
            .projected_catalog
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
        else {
            return Ok(());
        };
        let macros = OnboardMacro::for_current_keyboard(self.device, self.cap)?;
        match macros.delete_keyboard_local(id, &mut self.next, &self.guard) {
            Ok(report) => {
                self.acknowledge_delete(id, &report)?;
                if let Some(memory) = &mut self.memory {
                    memory.released(entry.allocated_size)?;
                }
                self.receipt
                    .projected_catalog
                    .retain(|entry| entry.id != id);
                self.receipt.commands.push(CommandReceipt {
                    deleting: true,
                    report,
                    error: None,
                });
                Ok(())
            }
            Err(error) => {
                let resolved_failure = error.phase == MacroFailurePhase::Delete
                    && crate::feature_exchange::returned_status(error.error()).is_some()
                    && !self.interrupted(error.error());
                if error.device_state_may_have_changed() {
                    self.restoration_valid = false;
                }
                self.receipt.commands.push(CommandReceipt {
                    deleting: true,
                    report: error.progress.clone(),
                    error: Some(error.to_string()),
                });
                if resolved_failure {
                    Ok(())
                } else {
                    Err(error.into_error())
                }
            }
        }
    }

    fn add_macro(&mut self, compiled: &CompiledMacro, kind: Addition) -> Result<()> {
        let mut result = self.add_once(compiled);
        if kind != Addition::NewWithStandbyCleanup
            || !result
                .as_ref()
                .is_err_and(|error| error.is::<CapacityError>())
        {
            return result;
        }
        let candidates = self
            .receipt
            .projected_catalog
            .iter()
            .filter(|entry| entry.id != 0 && self.standby.contains(&entry.id))
            .map(|entry| entry.id)
            .collect::<Vec<_>>();
        for id in candidates {
            (self.guard)()?;
            if let Err(error) = self.delete_macro(id) {
                if self.interrupted(&error) {
                    return Err(error);
                }
                self.receipt
                    .standby_delete_errors
                    .push(format!("{error:#}"));
            }
            result = self.add_once(compiled);
            if !result
                .as_ref()
                .is_err_and(|error| error.is::<CapacityError>())
            {
                return result;
            }
        }
        result
    }

    fn enqueue_macro_import(
        &mut self,
        documents: Vec<onboard_macro::import::Document>,
    ) -> Result<()> {
        (self.guard)()?;
        self.receipt
            .imports
            .push_back(ImportEvent::Macros(documents));
        Ok(())
    }
    fn enqueue_profile_import(&mut self) -> Result<()> {
        (self.guard)()?;
        self.receipt.imports.push_back(ImportEvent::Profiles);
        Ok(())
    }
    fn record_failed_macro(&mut self, guid: &str, name: &str) {
        if let Some(old) = self
            .receipt
            .failed_macros
            .iter_mut()
            .find(|entry| entry.0 == guid)
        {
            old.1 = name.to_owned();
        } else {
            self.receipt
                .failed_macros
                .push((guid.to_owned(), name.to_owned()));
        }
    }
}
