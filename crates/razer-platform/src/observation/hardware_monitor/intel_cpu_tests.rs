use super::*;
use std::collections::BTreeMap;
struct Fake { registers: BTreeMap<u32,u64>, calls: usize, fail: Option<usize>, counters: [u32; 2], step: [u64;2], writes: Vec<(u32,u64)> }
impl Fake { fn new() -> Self { Self { registers: BTreeMap::from([(0x38f,0x1200000020),(0x38d,0x34000000ab),(0x1a2,100<<16),(0x1b1,(1<<31)|(39<<16))]), calls:0, fail:None, counters:[0,0],step:[200,100],writes:Vec::new() } } }
impl Transport for Fake {
    fn ioctl(&mut self, code: u32, input: &[u8], output: &mut [u8]) -> anyhow::Result<usize> {
        self.calls += 1;
        let register = u32::from_le_bytes(input[..4].try_into().unwrap());
        if code == 0x9c402444 {
            let value = (u64::from(u32::from_le_bytes(input[4..8].try_into().unwrap()))<<32)|u64::from(u32::from_le_bytes(input[8..12].try_into().unwrap()));
            self.registers.insert(register,value); self.writes.push((register,value));
        }
        ensure!(self.fail != Some(self.calls), "injected transport failure");
        match code {
            0x9c402440 => {
                let value = if register == 0x30a || register == 0x30b { let index=usize::from(register==0x30b); self.counters[index]+=1; u64::from(self.counters[index]) * self.step[index] } else { *self.registers.get(&register).unwrap() };
                output.copy_from_slice(&value.to_le_bytes());
            },
            0x9c402444 => output.fill(0),
            0x9c402608 => { output[..4].copy_from_slice(&0_u32.to_le_bytes()); output[4..].copy_from_slice(&3_000_000_000_u32.to_le_bytes()); },
            _=>panic!("unexpected IOCTL"),
        }
        Ok(output.len())
    }
}
fn restored(driver: Driver<Fake>) { let fake=driver.into_transport(); assert_eq!(fake.registers[&0x38f],0x1200000020); assert_eq!(fake.registers[&0x38d],0x34000000ab); }
#[test]
fn current_package_temperature_uses_valid_bit_and_measured_target() {
    let mut driver=Driver::new(Fake::new()); assert_eq!(temperature(&mut driver).unwrap(),61.0);
    let mut fake=driver.into_transport(); fake.registers.insert(0x1b1,39<<16); assert!(temperature(&mut Driver::new(fake)).is_err());
}
#[test]
fn fixed_counters_produce_actual_ratio_then_restore_both_original_controls() {
    let mut driver=Driver::new(Fake::new()); assert_eq!(fixed_counter_clock(&mut driver,8, |_| Ok(())).unwrap(),6000.0); restored(driver);
}
#[test]
fn every_measurement_transport_failure_restores_modified_controls() {
    // Calls 1..11 cover both saved reads, enable writes/readbacks, TSC and
    // counter observations. Failed writes may still have reached hardware.
    for failure in 1..=11 { let mut fake=Fake::new(); fake.fail=Some(failure); let mut driver=Driver::new(fake); assert!(fixed_counter_clock(&mut driver,4, |_| Ok(())).is_err(), "failure {failure}"); restored(driver); }
}
#[test]
fn failed_or_panicking_timer_restores_the_actual_original_controls() {
    for panic in [false,true] { let mut driver=Driver::new(Fake::new()); assert!(fixed_counter_clock(&mut driver,4, |_| { if panic { panic!("timer panic") } else { anyhow::bail!("timer read failed") } }).is_err()); restored(driver); }
}

#[test]
fn fixed_counter_ratio_preserves_high_dwords_from_complete_assembly() {
    let mut fake=Fake::new();fake.step=[0x2_0000_0000,0x1_0000_0000];
    let mut driver=Driver::new(fake);assert_eq!(fixed_counter_clock(&mut driver,4, |_| Ok(())).unwrap(),6000.0);restored(driver);
}
