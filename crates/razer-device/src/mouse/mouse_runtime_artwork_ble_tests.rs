//! Wire boundaries from host 4.0.896's `electron/modules/noble/strategy/protocol30.js`
//! (`write`, bytes 1597..3506) and `electron/modules/noble/constants.js`, retained
//! in `.analysis/inputs/host-current/asar/e7a524aca177/`.
//! Feature inputs follow `rzDevice30.77250cbc2a3f134d767c.js` in the current
//! PID226 snapshot `mouse-mw-226-snapshot-738e48c31af52973b33c`.
use super::write_ble_command;
use anyhow::ensure;
use std::cell::Cell;

fn packets(frame: &[u8]) -> Vec<Vec<u8>> {
    let mut sent = vec![];
    write_ble_command(frame, &|| Ok(()), |fragment| {
        sent.push(fragment.to_vec());
        Ok(())
    })
    .unwrap();
    sent
}

#[test]
fn protocol30_write_keeps_8_9_28_and_29_byte_wire_boundaries() {
    assert_eq!(
        packets(&[3, 0, 0, 0, 1, 134, 0, 0]),
        [vec![3, 0, 0, 0, 1, 134, 0, 0]]
    );
    assert_eq!(
        packets(&[4, 1, 0, 0, 5, 2, 0, 0, 128]),
        [vec![4, 1, 0, 0, 5, 2, 0, 0], vec![128]]
    );
    assert_eq!(
        packets(&[
            5, 20, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139,
            140, 141, 142, 143, 144, 145, 146, 147,
        ]),
        [
            vec![5, 20, 0, 0, 11, 4, 1, 0],
            vec![
                128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
                144, 145, 146, 147,
            ],
        ]
    );
    assert_eq!(
        packets(&[
            6, 21, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139,
            140, 141, 142, 143, 144, 145, 146, 147, 148,
        ]),
        [
            vec![6, 21, 0, 0, 11, 4, 1, 0],
            vec![
                128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
                144, 145, 146, 147,
            ],
            vec![148],
        ]
    );
}

#[test]
fn protocol30_write_splits_current_power_rotation_and_dpi_frames() {
    use crate::{mouse_current_dpi, mouse_current_power, mouse_rotation};

    // Current rzDevice30.setTimeToSleep(300),
    // setProximitySensorAngleTune(1, 1, 10) and setDPIStages(1, 2, 3, records)
    // use these independent wire bytes.
    let power =
        mouse_current_power::ble::frame(7, mouse_current_power::ble::SET_SLEEP, &[44, 1]).unwrap();
    assert_eq!(packets(&power), [vec![7, 2, 0, 0, 5, 4, 0, 0], vec![44, 1]]);
    let rotation = mouse_rotation::ble::frame(
        8,
        mouse_rotation::ble::SET,
        mouse_rotation::ble::PROFILE,
        &[1, 10],
    )
    .unwrap();
    assert_eq!(
        packets(&rotation),
        [vec![8, 2, 0, 0, 15, 20, 1, 0], vec![1, 10]]
    );
    let dpi = mouse_current_dpi::ble::frame(
        9,
        mouse_current_dpi::ble::SET_STAGES,
        1,
        &[
            2, 3, 0, 144, 1, 244, 1, 0, 0, 1, 88, 2, 188, 2, 0, 0, 2, 32, 3, 132, 3, 0, 0, 0,
        ],
    )
    .unwrap();
    assert_eq!(
        packets(&dpi),
        [
            vec![9, 24, 0, 0, 11, 4, 1, 0],
            vec![
                2, 3, 0, 144, 1, 244, 1, 0, 0, 1, 88, 2, 188, 2, 0, 0, 2, 32, 3, 132
            ],
            vec![3, 0, 0, 0],
        ]
    );
}

#[test]
fn protocol30_write_stops_at_the_first_failed_header_or_data_fragment() {
    let frame = [
        6, 21, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
        141, 142, 143, 144, 145, 146, 147, 148,
    ];
    for failure_at in 0..3 {
        let mut calls = 0;
        let result = write_ble_command(&frame, &|| Ok(()), |_| {
            let position = calls;
            calls += 1;
            ensure!(position != failure_at, "simulated GATT write failure");
            Ok(())
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "simulated GATT write failure"
        );
        assert_eq!(calls, failure_at + 1);
    }
}

#[test]
fn protocol30_write_rechecks_cancellation_before_each_fragment() {
    let frame = [
        6, 21, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
        141, 142, 143, 144, 145, 146, 147, 148,
    ];
    for cancel_after in 0..3 {
        let calls = Cell::new(0);
        let result = write_ble_command(
            &frame,
            &|| {
                ensure!(calls.get() < cancel_after, "owner canceled");
                Ok(())
            },
            |_| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err().to_string(), "owner canceled");
        assert_eq!(calls.get(), cancel_after);
    }
}

#[test]
fn protocol30_write_rejects_truncated_headers_and_invalid_transactions_before_io() {
    for frame in [
        &[][..],
        &[3, 0, 0, 0, 1, 134, 0][..],
        &[31, 0, 0, 0, 1, 134, 0, 0][..],
    ] {
        let result = write_ble_command(frame, &|| Ok(()), |_| {
            panic!("invalid Protocol30 command reached the transport")
        });
        assert_eq!(
            result.unwrap_err().to_string(),
            "Protocol30 command header/transaction differs"
        );
    }
}
