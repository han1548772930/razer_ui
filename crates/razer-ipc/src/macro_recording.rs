//! Recorder suppression receipts are observations of native callbacks.
//! A false source result remains data; transport and cleanup failures are separate.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct SuspensionReceipt {
    suspended: bool,
}

impl SuspensionReceipt {
    pub fn new(suspended: bool) -> Self {
        Self { suspended }
    }

    pub fn suspended(&self) -> bool {
        self.suspended
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ActionReply {
    result: bool,
    reason: String,
}

impl ActionReply {
    pub fn new(result: bool, reason: String) -> Self {
        Self { result, reason }
    }

    pub fn result(&self) -> bool {
        self.result
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct RestoreReceipt {
    suspended: bool,
    mapping: ActionReply,
    global_shortcuts: ActionReply,
}

impl RestoreReceipt {
    pub fn new(suspended: bool, mapping: ActionReply, global_shortcuts: ActionReply) -> Self {
        Self {
            suspended,
            mapping,
            global_shortcuts,
        }
    }

    pub fn suspended(&self) -> bool {
        self.suspended
    }

    pub fn mapping(&self) -> &ActionReply {
        &self.mapping
    }

    pub fn global_shortcuts(&self) -> &ActionReply {
        &self.global_shortcuts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_false_and_reason_are_preserved_in_the_wire_receipt() {
        let expected = json!({
            "suspended":true,
            "mapping":{"result":false,"reason":"mapping rejected"},
            "global_shortcuts":{"result":true,"reason":""}
        });
        let actual = RestoreReceipt::new(
            true,
            ActionReply::new(false, "mapping rejected".into()),
            ActionReply::new(true, String::new()),
        );
        assert_eq!(serde_json::to_value(&actual).unwrap(), expected);
        let observed: RestoreReceipt = serde_json::from_value(expected).unwrap();
        assert!(observed.suspended());
        assert!(!observed.mapping().result());
        assert_eq!(observed.mapping().reason(), "mapping rejected");
        assert!(observed.global_shortcuts().result());
    }

    #[test]
    fn suppression_and_restoration_require_complete_typed_observations() {
        for invalid in [
            json!({}),
            json!({"suspended":null}),
            json!({"suspended":"true"}),
        ] {
            assert!(serde_json::from_value::<SuspensionReceipt>(invalid).is_err());
        }
        for invalid in [
            json!({"global_shortcuts":{"result":true}}),
            json!({"suspended":false,"mapping":{"result":true,"reason":""},"global_shortcuts":{"result":true}}),
            json!({"suspended":false,"mapping":{"result":true,"reason":""},"global_shortcuts":{"result":"true","reason":""}}),
            json!({"suspended":false,"mapping":{"result":true,"reason":null},"global_shortcuts":{"result":true,"reason":""}}),
        ] {
            assert!(serde_json::from_value::<RestoreReceipt>(invalid).is_err());
        }
    }
}
