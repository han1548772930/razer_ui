//! Current keyboard initialization's macro phase. Profile reconciliation owns
//! the assigned/unassigned profile selection and supplies its typed GUID sets.
//! Device commands and queued local-import events keep their own receipts.
use crate::{
    keyboard_onboard_profiles::CompleteReading,
    onboard_macro::{self, MacroSummary},
    onboard_macro_codec::{CompiledMacro, LocalDocument},
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

#[path = "keyboard_macro_runtime.rs"]
pub mod device;
#[cfg(test)]
#[path = "keyboard_macro_reconciliation_tests.rs"]
mod tests;

#[derive(Clone, Debug, Default)]
pub struct Requirements {
    /// Source q: assigned profiles' stored mappings plus unassigned profiles'
    /// direct macro mappings, in first-occurrence order.
    pub referenced_guids: Vec<String>,
    /// Source Z: assigned profiles' referenced GUIDs absent from device storage.
    pub new_device_guids: Vec<String>,
    /// Raw OBM mapping macroGroup.id already contains the catalog GUID string.
    /// This is independent of each macro's numeric firmware storage ID.
    pub hardware_reference_guids: BTreeSet<String>,
    pub locked_device_ids: BTreeSet<u16>,
    /// The profile owner has an actual source import event to enqueue here.
    /// Product756 enqueues its event earlier and must supply false.
    pub profile_import_pending: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Addition {
    Replacement,
    New,
    NewWithStandbyCleanup,
}

/// One source task's concrete device owner and event queue implement this
/// boundary. A resolved method is not itself proof of device confirmation:
/// the owner must retain rejected ACKs and partial writes in its device receipt.
pub trait Runtime {
    fn delete_macro(&mut self, macro_id: u16) -> Result<()>;
    fn add_macro(&mut self, compiled: &CompiledMacro, kind: Addition) -> Result<()>;
    fn enqueue_macro_import(
        &mut self,
        documents: Vec<onboard_macro::import::Document>,
    ) -> Result<()>;
    fn enqueue_profile_import(&mut self) -> Result<()>;
    fn record_failed_macro(&mut self, guid: &str, name: &str);
}

#[derive(Clone, Debug, Serialize)]
pub enum Step {
    SkippedBlankHash { macro_id: u16 },
    Compared { macro_id: u16, differs: bool },
    DeletedForReplacement { macro_id: u16 },
    ReplacementSubmitted { old_macro_id: u16, guid: String },
    UnsupportedReplacement { old_macro_id: u16 },
    MacroImportEnqueued { guids: Vec<String> },
    ProfileImportEnqueued,
    ObsoleteDeleted { macro_id: u16 },
    NewSubmitted { guid: String },
    UnsupportedNew { guid: String },
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub steps: Vec<Step>,
    /// Source per-replacement/per-add catches retain task.error even if the
    /// outer source wrapper subsequently reaches its completed assignment.
    pub caught_errors: Vec<String>,
    pub phase_exhausted: bool,
}

#[derive(Debug)]
pub struct Failure {
    pub progress: Report,
    error: anyhow::Error,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Keyboard macro reconciliation failed: {:#}", self.error)
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

fn catch_operation(
    result: Result<()>,
    report: &mut Report,
    guard: &impl Fn() -> Result<()>,
) -> Result<()> {
    if let Err(error) = result {
        if crate::backend::exchange_interrupted(&error) || guard().is_err() {
            return Err(error);
        }
        report.caught_errors.push(format!("{error:#}"));
    }
    Ok(())
}

fn differs(
    product: u32,
    summary: &MacroSummary,
    compiled: Option<&CompiledMacro>,
    reading: &CompleteReading,
) -> Result<bool> {
    if !matches!(product, 739 | 747) {
        return Ok(summary.metadata.hash
            != compiled.map_or("", |compiled| compiled.request().hash.as_str()));
    }
    let Some(compiled) = compiled else {
        return Ok(true);
    };
    let hardware = reading
        .import_details
        .macros
        .iter()
        .find(|detail| detail.macro_id == summary.metadata.macro_id)
        .and_then(|detail| detail.content.observed())
        .context("Full-content macro comparison has no observed device data")?;
    ensure!(
        hardware.metadata == summary.metadata,
        "Macro detail belongs to another observation"
    );
    hardware.validate_keyboard_content(product)?;
    let decoded = hardware.keyboard_projection(product)?;
    Ok(onboard_macro::decode::differs(
        compiled.comparison_items(),
        &decoded.items,
    ))
}

/// Run after the profile owner's source-ordered difference preparation. Local
/// imports are queued in the source order; their persistence is a later task.
/// This function never substitutes local documents for observed device data.
pub fn reconcile(
    reading: &CompleteReading,
    requirements: &Requirements,
    documents: &[LocalDocument],
    runtime: &mut impl Runtime,
    guard: &impl Fn() -> Result<()>,
) -> std::result::Result<Report, Failure> {
    let mut report = Report::default();
    let result = (|| -> Result<()> {
        guard()?;
        reading.snapshot.validate()?;
        let product = reading.snapshot.product_id;
        ensure!(
            product != 756 || !requirements.profile_import_pending,
            "This product imports prepared profiles before the macro phase"
        );
        let catalog = &reading.snapshot.macros;
        ensure!(
            requirements
                .locked_device_ids
                .iter()
                .all(|id| catalog.inventory.ids.contains(id)),
            "Protected macro ID is absent from observed storage"
        );
        let referenced = requirements
            .referenced_guids
            .iter()
            .collect::<BTreeSet<_>>();
        let local_guids = documents
            .iter()
            .map(LocalDocument::guid)
            .collect::<BTreeSet<_>>();
        for old in &catalog.macros {
            if !referenced.contains(&old.metadata.guid) {
                continue;
            }
            guard()?;
            if !matches!(product, 739 | 747) && old.metadata.hash.is_empty() {
                report.steps.push(Step::SkippedBlankHash {
                    macro_id: old.metadata.macro_id,
                });
                continue;
            }
            let Some(document) = documents
                .iter()
                .find(|document| document.guid() == old.metadata.guid)
            else {
                continue;
            };
            // Conversion itself is outside the source replacement catch.
            let compiled = document.compile(product)?;
            let changed = differs(product, old, compiled.as_ref(), reading)?;
            report.steps.push(Step::Compared {
                macro_id: old.metadata.macro_id,
                differs: changed,
            });
            if !changed {
                continue;
            }
            let operation = (|| -> Result<()> {
                runtime.delete_macro(old.metadata.macro_id)?;
                report.steps.push(Step::DeletedForReplacement {
                    macro_id: old.metadata.macro_id,
                });
                guard()?;
                if let Some(compiled) = compiled {
                    runtime.add_macro(&compiled, Addition::Replacement)?;
                    report.steps.push(Step::ReplacementSubmitted {
                        old_macro_id: old.metadata.macro_id,
                        guid: document.guid().to_owned(),
                    });
                } else {
                    // Source addMacro's absent required fields return without
                    // an allocation after the preceding deletion has run.
                    report.steps.push(Step::UnsupportedReplacement {
                        old_macro_id: old.metadata.macro_id,
                    });
                }
                Ok(())
            })();
            catch_operation(operation, &mut report, guard)?;
        }
        let mut imports = Vec::new();
        for old in &catalog.macros {
            if !requirements
                .hardware_reference_guids
                .contains(&old.metadata.guid)
                || local_guids.contains(old.metadata.guid.as_str())
            {
                continue;
            }
            guard()?;
            let hardware = reading
                .import_details
                .macros
                .iter()
                .find(|detail| detail.macro_id == old.metadata.macro_id)
                .and_then(|detail| detail.content.observed())
                .context("Imported macro has no observed content")?;
            ensure!(
                hardware.metadata == old.metadata,
                "Imported macro belongs to another observation"
            );
            imports.push(onboard_macro::import::from_reading(product, hardware)?);
        }
        if !imports.is_empty() {
            let guids = imports
                .iter()
                .map(|document| document.guid().to_owned())
                .collect();
            guard()?;
            runtime.enqueue_macro_import(imports)?;
            report.steps.push(Step::MacroImportEnqueued { guids });
        }
        if requirements.profile_import_pending {
            guard()?;
            runtime.enqueue_profile_import()?;
            report.steps.push(Step::ProfileImportEnqueued);
        }
        for old in &catalog.macros {
            if referenced.contains(&old.metadata.guid)
                || requirements
                    .locked_device_ids
                    .contains(&old.metadata.macro_id)
            {
                continue;
            }
            guard()?;
            runtime.delete_macro(old.metadata.macro_id)?;
            report.steps.push(Step::ObsoleteDeleted {
                macro_id: old.metadata.macro_id,
            });
        }
        for guid in &requirements.new_device_guids {
            guard()?;
            let Some(document) = documents
                .iter()
                .rev()
                .find(|document| document.guid() == guid && document.supports_onboard())
            else {
                continue;
            };
            let operation = (|| -> Result<()> {
                let Some(compiled) = document.compile(product)? else {
                    report
                        .steps
                        .push(Step::UnsupportedNew { guid: guid.clone() });
                    return Ok(());
                };
                runtime.add_macro(
                    &compiled,
                    if matches!(product, 739 | 747) {
                        Addition::New
                    } else {
                        Addition::NewWithStandbyCleanup
                    },
                )?;
                report.steps.push(Step::NewSubmitted { guid: guid.clone() });
                Ok(())
            })();
            if operation.is_err() && !matches!(product, 739 | 747) {
                runtime.record_failed_macro(document.guid(), document.name());
            }
            catch_operation(operation, &mut report, guard)?;
        }
        guard()?;
        report.phase_exhausted = true;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(report),
        Err(error) => Err(Failure {
            progress: report,
            error,
        }),
    }
}
