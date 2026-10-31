//! Connection-owned serial reads from the current GenericCamera vtables.
//! Pro 1.0.36.1 reads A0 02 chunks 0..2; base/X 1.0.35.0 read C0 01
//! chunks 1..3 and 02 00 chunks 1..4 through their existing ISP owners.
use anyhow::{Result, ensure};

use crate::camera_extensions::{
    CameraExtensionCommand, CameraExtensionExchange, CameraExtensionPhase,
    CameraExtensionTransport, run_exchange,
};
use crate::camera_focus_family::{
    CameraFamilyFocusAttempt, CameraFamilyFocusFailure, CameraFamilyFocusPhase,
    CameraFamilyFocusTransport, execute_camera_family_isp,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialExchange {
    Pro {
        exchange: CameraExtensionExchange,
        failure: Option<(CameraExtensionPhase, String)>,
    },
    Family {
        topology_nodes: Option<u32>,
        attempts: Vec<CameraFamilyFocusAttempt>,
        completed: bool,
        failure: Option<CameraFamilyFocusFailure>,
    },
}

impl SerialExchange {
    fn response(&self) -> Option<[u8; 8]> {
        match self {
            Self::Pro { exchange, failure: None } if exchange.acknowledged => exchange.response,
            Self::Family { attempts, completed: true, failure: None, .. } => {
                attempts.last().filter(|attempt| attempt.acknowledged).and_then(|attempt| attempt.response)
            }
            _ => None,
        }
    }

    fn interruption(&self) -> Option<&str> {
        match self {
            Self::Pro { failure: Some((CameraExtensionPhase::Validation | CameraExtensionPhase::Wait, message)), .. } => Some(message),
            Self::Family { failure: Some(CameraFamilyFocusFailure { phase: CameraFamilyFocusPhase::Validation | CameraFamilyFocusPhase::Wait, message, .. }), .. } => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerialChunk {
    attempt: u8,
    command: [u8; 8],
    exchange: SerialExchange,
}

impl SerialChunk {
    pub fn attempt(&self) -> u8 { self.attempt }
    pub fn command(&self) -> &[u8; 8] { &self.command }
    pub fn exchange(&self) -> &SerialExchange { &self.exchange }
}

/// Failed chunk buffers remain diagnostic. Only acknowledged chunks contribute
/// to the source's serial string; partial strings remain visible to validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraSerialProgress {
    product_id: u32,
    chunks: Vec<SerialChunk>,
    serial: Option<String>,
    reused_observation: bool,
    error: Option<String>,
}

impl CameraSerialProgress {
    fn new(product_id: u32) -> Self {
        Self { product_id, chunks: Vec::new(), serial: None, reused_observation: false, error: None }
    }
    pub fn product_id(&self) -> u32 { self.product_id }
    pub fn chunks(&self) -> &[SerialChunk] { &self.chunks }
    pub fn serial(&self) -> Option<&str> { self.serial.as_deref() }
    pub fn reused_observation(&self) -> bool { self.reused_observation }
    pub fn error(&self) -> Option<&str> { self.error.as_deref() }
}

/// The owning live camera session creates and drops this state. No persisted
/// serial or caller-supplied default can populate its observation.
pub struct CameraSerialState {
    product_id: u32,
    observed_bytes: Option<Vec<u8>>,
}

trait SerialReader {
    fn exchange(&mut self, command: [u8; 8], validate: &impl Fn() -> Result<()>) -> SerialExchange;
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()>;
}

struct ProReader<'a, T>(&'a mut T);
impl<T: CameraExtensionTransport> SerialReader for ProReader<'_, T> {
    fn exchange(&mut self, command: [u8; 8], validate: &impl Fn() -> Result<()>) -> SerialExchange {
        let mut exchange = CameraExtensionExchange::new(CameraExtensionCommand::new(&command, false, 0x52070));
        let failure = run_exchange(self.0, &mut exchange, validate).err();
        SerialExchange::Pro { exchange, failure }
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> { self.0.wait_ms(milliseconds) }
}

struct FamilyReader<'a, T> { transport: &'a mut T, product_id: u32 }
impl<T: CameraFamilyFocusTransport> SerialReader for FamilyReader<'_, T> {
    fn exchange(&mut self, command: [u8; 8], validate: &impl Fn() -> Result<()>) -> SerialExchange {
        let progress = execute_camera_family_isp(self.transport, self.product_id, command, validate);
        SerialExchange::Family {
            topology_nodes: progress.topology_nodes, attempts: progress.attempts,
            completed: progress.completed, failure: progress.failure,
        }
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> { self.transport.wait_ms(milliseconds) }
}

impl CameraSerialState {
    pub fn new(product_id: u32) -> Result<Self> {
        ensure!(matches!(product_id, 3594..=3596), "camera serial protocol is not audited for this product");
        Ok(Self { product_id, observed_bytes: None })
    }

    pub fn read_pro(&mut self, transport: &mut impl CameraExtensionTransport, validate: &impl Fn() -> Result<()>) -> CameraSerialProgress {
        if self.product_id != 3594 {
            let mut progress = CameraSerialProgress::new(self.product_id);
            progress.error = Some("camera serial transport does not match the live product".into());
            return progress;
        }
        self.read(&mut ProReader(transport), validate)
    }

    pub fn read_family(&mut self, transport: &mut impl CameraFamilyFocusTransport, validate: &impl Fn() -> Result<()>) -> CameraSerialProgress {
        if !matches!(self.product_id, 3595 | 3596) {
            let mut progress = CameraSerialProgress::new(self.product_id);
            progress.error = Some("camera serial transport does not match the live product".into());
            return progress;
        }
        let product_id = self.product_id;
        self.read(&mut FamilyReader { transport, product_id }, validate)
    }

    fn read(&mut self, transport: &mut impl SerialReader, validate: &impl Fn() -> Result<()>) -> CameraSerialProgress {
        let mut progress = CameraSerialProgress::new(self.product_id);
        let result = (|| -> Result<String> {
            validate()?;
            if let Some(bytes) = &self.observed_bytes {
                progress.reused_observation = true;
                return serial_text(bytes);
            }
            // The manager retries a false virtual getter ten times, sleeping
            // 300 ms after each failed attempt, including the final attempt.
            for attempt in 0..10 {
                let mut bytes = Vec::new();
                let (prefix, start, count) = match self.product_id {
                    3594 => ([0xa0, 2], 0, 3),
                    3595 => ([0xc0, 1], 1, 3),
                    3596 => ([2, 0], 1, 4),
                    _ => unreachable!("constructor validates the serial product"),
                };
                for part in start..start + count {
                    validate()?;
                    let command = [prefix[0], prefix[1], part, 0, 0, 0, 0, 0];
                    let exchange = transport.exchange(command, validate);
                    if let Some(response) = exchange.response() {
                        bytes.extend_from_slice(&response[3..8]);
                    }
                    let interruption = exchange.interruption().map(str::to_owned);
                    progress.chunks.push(SerialChunk { attempt, command, exchange });
                    if let Some(error) = interruption { anyhow::bail!(error); }
                    validate()?;
                    transport.wait_ms(10)?;
                    validate()?;
                }
                if !bytes.is_empty() {
                    let serial = serial_text(&bytes);
                    self.observed_bytes = Some(bytes);
                    return serial;
                }
                transport.wait_ms(300)?;
                validate()?;
            }
            anyhow::bail!("camera serial getter received no acknowledged serial chunks after ten attempts")
        })();
        match result {
            Ok(serial) => progress.serial = Some(serial),
            Err(error) => progress.error = Some(format!("{error:#}")),
        }
        progress
    }
}

fn serial_text(bytes: &[u8]) -> Result<String> {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    Ok(std::str::from_utf8(&bytes[..end])?.to_owned())
}

#[cfg(test)]
#[path = "camera_serial_tests.rs"]
mod tests;
