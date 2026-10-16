//! Current lighting_driver 1.9.14.0 `protocol::Rzp25NewChroma` replacement.
//!
//! Native factory RVA 0x2edd0, translator 0x7feb0, checksum 0x7fc30.
//! See docs/re/lighting-native-current.md. Reports include the HID report ID;
//! the transport owns platform IO and delay scheduling. No vendor DLL is loaded.

use anyhow::{Result, bail};

/// Registration fields consumed by the native Rzp25NewChroma translator.
#[derive(Clone, Copy, Debug)]
pub struct Registration {
    pub category: u8,
    pub report_id: u8,
    pub row_delay_ms: u8,
}

/// The normalized native RGB matrix (frame bytes are ordered by row).
pub struct RgbFrame<'a> {
    pub first_row: u8,
    pub last_row: u8,
    pub first_column: u8,
    pub last_column: u8,
    pub region_id: u8,
    pub profile_id: u8,
    pub rgb: &'a [u8],
}

/// Actual write and wait operations, implemented by the device transport owner.
/// A callback invocation alone is not proof that the HID write succeeded.
pub trait FrameWriter {
    fn write_report(&mut self, report: &[u8; 91]) -> Result<()>;
    fn wait_between_rows(&mut self, milliseconds: u8) -> Result<()>;
}

/// Source keeps this counter on the shared protocol translator, starting at 1.
pub struct NewChroma {
    transaction_counter: u8,
}

impl Default for NewChroma {
    fn default() -> Self {
        Self {
            transaction_counter: 1,
        }
    }
}

impl NewChroma {
    /// Send rows in source order and stop at the first actual transport failure.
    ///
    /// Native logs malformed payloads and can return true without writing. This
    /// implementation reports them explicitly; it never invents a device save.
    pub fn write_frame(
        &mut self,
        registration: Registration,
        frame: RgbFrame<'_>,
        writer: &mut impl FrameWriter,
    ) -> Result<()> {
        if frame.first_row > frame.last_row || frame.first_column > frame.last_column {
            bail!("lighting RGB matrix bounds are inverted");
        }
        let columns = usize::from(frame.last_column - frame.first_column) + 1;
        let row_bytes = columns * 3;
        if row_bytes > 77 {
            bail!("lighting RGB row exceeds the native 81-byte payload");
        }
        let rows = usize::from(frame.last_row - frame.first_row) + 1;
        if frame.rgb.len() != rows * row_bytes {
            bail!("lighting RGB frame length does not match its matrix");
        }
        for row_offset in 0..rows {
            let row = frame.first_row + row_offset as u8;
            let mut report = [0u8; 91];
            report[0] = registration.report_id;
            report[2] = (self.transaction_counter & 0x1f) | registration.category.wrapping_shl(5);
            self.transaction_counter = self.transaction_counter.wrapping_add(1);
            report[6] = (5 + row_bytes) as u8;
            report[7] = 0x0f;
            report[8] = 0x03;
            report[9] = frame.profile_id;
            report[10] = frame.region_id;
            report[11] = row;
            report[12] = frame.first_column;
            report[13] = frame.last_column;
            let start = row_offset * row_bytes;
            report[14..14 + row_bytes].copy_from_slice(&frame.rgb[start..start + row_bytes]);
            // RVA 0x7fc30 XORs report bytes 3..91 with checksum initially zero.
            report[89] = report[3..].iter().fold(0, |sum, byte| sum ^ byte);
            writer.write_report(&report)?;
            if row_offset + 1 < rows {
                writer.wait_between_rows(registration.row_delay_ms)?;
            }
        }
        Ok(())
    }
}
