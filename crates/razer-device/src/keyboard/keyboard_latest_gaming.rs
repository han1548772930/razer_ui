//! PID756 entry to the shared source firmware gaming compiler.
use crate::mapping_engine::EngineContext;
use anyhow::{Result, ensure};
use serde_json::Value;

pub fn apply(
    document: &Value,
    running_games: Option<usize>,
    compose_f4: bool,
    context: &mut EngineContext,
) -> Result<Value> {
    ensure!(
        document["productId"] == 756,
        "Latest gaming caller requires logical product 756"
    );
    crate::keyboard_firmware_gaming::apply(document, running_games, compose_f4, context)
}
