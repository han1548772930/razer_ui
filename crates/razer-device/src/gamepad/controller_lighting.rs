//! Mounted controller lighting commands from current MW 70784, 65430 and
//! rzDevice25Sage. The transport result records acknowledgement separately
//! from observed data and from the source's best-effort PC-mode initialization.
use crate::{backend::FeatureTransport, controller_feature};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct LightingCapability {
    product_id: u32,
}
pub fn capability(product_id: u32) -> Option<LightingCapability> {
    matches!(product_id, 2636 | 4115).then_some(LightingCapability { product_id })
}
impl LightingCapability {
    pub fn claim_interface(self) -> i32 {
        match self.product_id {
            2636 => 1,
            4115 => 2,
            _ => unreachable!("Capability is constructed only from source products"),
        }
    }
    pub fn report_id(self) -> u8 {
        4
    }
    pub fn delay(self) -> Duration {
        Duration::from_millis(10)
    }
    pub fn accepts(self, physical_id: u32) -> bool {
        physical_id == self.product_id
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpectrumRate {
    Slow,
    Medium,
    Fast,
}
impl TryFrom<u8> for SpectrumRate {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> anyhow::Result<Self> {
        match value {
            1 => Ok(Self::Fast),
            2 => Ok(Self::Medium),
            3 => Ok(Self::Slow),
            _ => anyhow::bail!("Source controller spectrum rate must be 1, 2 or 3"),
        }
    }
}
impl SpectrumRate {
    pub fn source_value(self) -> u8 {
        match self {
            Self::Slow => 3,
            Self::Medium => 2,
            Self::Fast => 1,
        }
    }
    fn wire(self) -> u8 {
        match self {
            Self::Slow => 1,
            Self::Medium => 0,
            Self::Fast => 2,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControllerEffect {
    Off,
    Spectrum {
        rate: SpectrumRate,
    },
    Static {
        index: u8,
        custom_colors: Vec<[u8; 3]>,
    },
}
impl ControllerEffect {
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Self::Static {
            index,
            custom_colors,
        } = self
        {
            ensure!(
                *index < 16 && custom_colors.len() <= 8,
                "Controller static palette exceeds its source limits"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LightingOperation {
    Read {
        profile_slot: u8,
    },
    Set {
        profile_slot: u8,
        effect: ControllerEffect,
    },
    Initialize {
        profile_slot: u8,
        effect: ControllerEffect,
    },
    ReadBrightness,
    Brightness {
        value: u8,
    },
}
impl LightingOperation {
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Set { effect, .. } | Self::Initialize { effect, .. } => effect.validate(),
            Self::Brightness { value } => {
                ensure!(*value <= 100, "Controller brightness exceeds 100 percent");
                Ok(())
            }
            _ => Ok(()),
        }
    }
    pub fn is_read(&self) -> bool {
        matches!(self, Self::Read { .. } | Self::ReadBrightness)
    }
    pub fn validate_product(&self, product_id: u32) -> anyhow::Result<()> {
        ensure!(
            capability(product_id).is_some(),
            "Product does not mount the source lighting capability"
        );
        if matches!(
            self,
            Self::Read { .. } | Self::Set { .. } | Self::Initialize { .. }
        ) {
            ensure!(
                product_id == 2636,
                "Controller preset effects are mounted only by this source Sage branch"
            );
        }
        self.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ObservedEffect {
    pub profile_slot: u8,
    pub region_id: u8,
    pub effect_id: u8,
    pub flags: u8,
    pub rate: u8,
    pub color_count: u8,
    /// Preserve all returned bytes; source updateProfile iterates the returned
    /// colorArray rather than inventing a getter projection from a SET request.
    pub color_bytes: Vec<u8>,
}
impl ObservedEffect {
    fn parse(data: &[u8]) -> anyhow::Result<Self> {
        ensure!(data.len() >= 6, "Truncated controller lighting observation");
        ensure!(
            usize::from(data[5]) * 3 <= data.len() - 6,
            "Controller returned an incomplete color palette"
        );
        Ok(Self {
            profile_slot: data[0],
            region_id: data[1],
            effect_id: data[2],
            flags: data[3],
            rate: data[4],
            color_count: data[5],
            color_bytes: data[6..].to_vec(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ObservedBrightness {
    pub profile_slot: u8,
    pub region_id: u8,
    pub value: u8,
}
impl ObservedBrightness {
    fn parse(data: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            data.len() >= 3,
            "Truncated controller brightness observation"
        );
        Ok(Self {
            profile_slot: data[0],
            region_id: data[1],
            value: (u16::from(data[2]) * 100).div_ceil(255) as u8,
        })
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct InitializationReport {
    pub attempts: u8,
    pub observed_option: Option<u8>,
    pub correction_acknowledged: bool,
    pub failures: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
pub struct LightingResult {
    pub effect: Option<ObservedEffect>,
    pub brightness: Option<ObservedBrightness>,
    pub initialization: Option<InitializationReport>,
    pub acknowledged_writes: u32,
}

pub struct LightingProtocol<
    'a,
    T: FeatureTransport + ?Sized,
    W: FnMut(Duration) -> anyhow::Result<()>,
> {
    transport: &'a T,
    source: LightingCapability,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    LightingProtocol<'a, T, W>
{
    pub fn new(transport: &'a T, source: LightingCapability, wait: W) -> Self {
        Self {
            transport,
            source,
            wait,
            transaction: 0,
        }
    }
    fn exchange(&mut self, header: [u8; 3], data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let transaction = self.transaction;
        self.transaction = (transaction + 1) % 31;
        let request =
            controller_feature::report(self.source.report_id(), transaction, header, data)?;
        controller_feature::exchange(
            self.transport,
            &request,
            self.source.delay(),
            &mut self.wait,
        )
    }
    fn set(&mut self, profile: u8, effect: &ControllerEffect) -> anyhow::Result<()> {
        let mut data = vec![profile, 0, 0, 0, 0, 0];
        match effect {
            ControllerEffect::Off => {}
            ControllerEffect::Spectrum { rate } => {
                data[2] = 3;
                data[4] = rate.wire();
            }
            ControllerEffect::Static {
                index,
                custom_colors,
            } => {
                data[2] = 13;
                data[3] = *index;
                data[4] = 1;
                data[5] = u8::try_from(custom_colors.len())?;
                data.extend(custom_colors.iter().flatten().copied());
            }
        }
        self.exchange([u8::try_from(data.len())?, 15, 2], &data)?;
        Ok(())
    }
    fn brightness(&mut self) -> anyhow::Result<ObservedBrightness> {
        // getBrightness(region0) first asks for the actual region list. Its
        // first observed region is used; an empty successful list keeps 0.
        let region = match self.exchange([80, 15, 128], &[]) {
            Ok(regions) => regions.first().copied().unwrap_or(0),
            Err(_) => {
                // getBrightness's wrapper catches a failed region query and
                // selects region1. The owner must still propagate cancellation.
                (self.wait)(Duration::ZERO)?;
                1
            }
        };
        let data = self.exchange([3, 15, 132], &[1, region])?;
        ObservedBrightness::parse(&data)
    }
    pub fn execute(&mut self, operation: &LightingOperation) -> anyhow::Result<LightingResult> {
        operation.validate_product(self.source.product_id)?;
        let mut result = LightingResult::default();
        match operation {
            LightingOperation::Read { profile_slot } => {
                let data = self.exchange([80, 15, 130], &[*profile_slot, 0])?;
                let mut effect = ObservedEffect::parse(&data)?;
                if effect.effect_id == 13 {
                    effect = ObservedEffect::parse(
                        &self.exchange([5, 15, 130], &[*profile_slot, 0, 13, 0, 1])?,
                    )?;
                }
                result.effect = Some(effect);
            }
            LightingOperation::Set {
                profile_slot,
                effect,
            } => {
                self.set(*profile_slot, effect)?;
                result.acknowledged_writes += 1;
            }
            LightingOperation::Initialize {
                profile_slot,
                effect,
            } => {
                let mut initialized = InitializationReport::default();
                for _ in 0..3 {
                    (self.wait)(Duration::ZERO)?;
                    initialized.attempts += 1;
                    let attempt = (|| -> anyhow::Result<()> {
                        let data = self.exchange([1, 0, 214], &[0])?;
                        let option = *data
                            .first()
                            .context("Controller returned no PC-mode LED option")?;
                        initialized.observed_option = Some(option);
                        if option != 0 {
                            self.exchange([1, 0, 86], &[0])?;
                            initialized.correction_acknowledged = true;
                            result.acknowledged_writes += 1;
                        }
                        Ok(())
                    })();
                    if attempt.is_ok() {
                        break;
                    }
                    initialized
                        .failures
                        .push(format!("{:#}", attempt.unwrap_err()));
                    // Check cancellation outside the caught native failure,
                    // preserving the source retry delay without swallowing it.
                    (self.wait)(Duration::from_millis(100))?;
                }
                result.initialization = Some(initialized);
                self.set(*profile_slot, effect)?;
                result.acknowledged_writes += 1;
            }
            LightingOperation::ReadBrightness => result.brightness = Some(self.brightness()?),
            LightingOperation::Brightness { value } => {
                let observed = self.brightness()?;
                // Source compares requested region0 with the observed getter
                // region. A different region sends to region0 even if equal.
                if observed.region_id != 0 || observed.value != *value {
                    let wire = (u16::from(*value) * 255 / 100) as u8;
                    self.exchange([3, 15, 4], &[1, 0, wire])?;
                    result.acknowledged_writes += 1;
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "controller_lighting_tests.rs"]
mod tests;
