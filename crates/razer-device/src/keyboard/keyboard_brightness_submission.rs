use super::{
    BrightnessIntent, BrightnessSubmission, FeatureTransport, KeyboardBrightness,
    KeyboardBrightnessWriteFailure, KeyboardBrightnessWriteResult, KeyboardSettingsCapability,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

const OLED_PRODUCT: u32 = 691;
const OLED_SWITCH: [u8; 3] = [3, 23, 24];

#[derive(Clone, Debug, Deserialize, Serialize)]
enum Completion {
    Value(KeyboardBrightnessWriteResult),
    Enabled(KeyboardBrightness),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedBrightness {
    product_id: u32,
    requested: BrightnessSubmission,
    switch_acknowledgement: Option<Vec<u8>>,
    completion: Option<Completion>,
}
impl AppliedBrightness {
    pub fn switch_acknowledgement(&self) -> Option<&[u8]> {
        self.switch_acknowledgement.as_deref()
    }
    pub fn reading(&self) -> Option<&KeyboardBrightness> {
        match &self.completion {
            Some(Completion::Value(result)) => Some(&result.observed),
            Some(Completion::Enabled(reading)) => Some(reading),
            None => None,
        }
    }

    pub fn check_request(
        &self,
        product_id: u32,
        requested: BrightnessSubmission,
    ) -> anyhow::Result<()> {
        requested.validate()?;
        ensure!(
            self.product_id == product_id && self.requested == requested,
            "Keyboard brightness result belongs to another submission"
        );
        if product_id == OLED_PRODUCT {
            let ack = self
                .switch_acknowledgement
                .as_ref()
                .context("Keyboard OLED switch acknowledgement is absent")?;
            let cap =
                super::capability(product_id).context("Keyboard OLED capability is absent")?;
            ensure!(
                ack.len() >= 3
                    && ack[..3]
                        == [
                            cap.active_profile,
                            cap.all_region,
                            u8::from(requested.enabled())
                        ],
                "Keyboard OLED switch acknowledgement differs"
            );
        } else {
            ensure!(
                self.switch_acknowledgement.is_none(),
                "Unexpected keyboard switch acknowledgement"
            );
        }
        match self
            .completion
            .as_ref()
            .context("Keyboard brightness readback is absent")?
        {
            Completion::Enabled(reading) => {
                ensure!(
                    product_id == OLED_PRODUCT && requested.intent() == BrightnessIntent::Enabled,
                    "Keyboard brightness skipped a required level write"
                );
                ensure!(
                    reading.percent <= 100,
                    "Keyboard brightness reading is out of range"
                );
            }
            Completion::Value(result) => {
                ensure!(
                    product_id != OLED_PRODUCT || requested.intent() == BrightnessIntent::Value,
                    "Keyboard OLED toggle unexpectedly changed its level"
                );
                ensure!(
                    result.verified
                        && result.requested_percent == requested.effective_percent()
                        && result.observed.percent == requested.effective_percent(),
                    "Keyboard brightness write was not independently confirmed"
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BrightnessApplyFailure {
    progress: AppliedBrightness,
    value_write_failure: Option<KeyboardBrightnessWriteFailure>,
}
impl BrightnessApplyFailure {
    pub fn progress(&self) -> &AppliedBrightness {
        &self.progress
    }
}
impl std::fmt::Display for BrightnessApplyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Keyboard brightness submission failed; switch_ack={}, readback={}",
            self.progress.switch_acknowledgement.is_some(),
            self.progress.reading().is_some()
        )
    }
}
impl std::error::Error for BrightnessApplyFailure {}

pub fn apply_brightness(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    requested: BrightnessSubmission,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<AppliedBrightness> {
    requested.validate()?;
    let mut progress = AppliedBrightness {
        product_id: cap.product_id,
        requested,
        switch_acknowledgement: None,
        completion: None,
    };
    let result = (|| {
        if cap.product_id == OLED_PRODUCT {
            let payload = [
                cap.active_profile,
                cap.all_region,
                u8::from(requested.enabled()),
            ];
            let reply = super::exchange(
                device,
                cap,
                OLED_SWITCH,
                &payload,
                next_transaction()?,
                &validate,
            );
            if let Some(data) = crate::feature_exchange::acknowledged_reply(&reply) {
                progress.switch_acknowledgement = Some(data.to_vec());
            }
            let reply = reply.context("Keyboard OLED switch exchange did not complete")?;
            ensure!(
                reply.len() >= 3 && reply[..3] == payload,
                "Keyboard OLED brightness switch acknowledgement differs"
            );
            validate()?;
        }
        progress.completion = Some(
            if cap.product_id == OLED_PRODUCT && requested.intent() == BrightnessIntent::Enabled {
                Completion::Enabled(super::read(device, cap, &mut next_transaction, &validate)?)
            } else {
                Completion::Value(super::apply(
                    device,
                    cap,
                    requested.effective_percent(),
                    &mut next_transaction,
                    &validate,
                )?)
            },
        );
        progress.check_request(cap.product_id, requested)?;
        validate()?;
        Ok(())
    })();
    result.map_err(|error: anyhow::Error| {
        let value_write_failure = error
            .downcast_ref::<KeyboardBrightnessWriteFailure>()
            .cloned();
        error.context(BrightnessApplyFailure {
            progress: progress.clone(),
            value_write_failure,
        })
    })?;
    Ok(progress)
}
