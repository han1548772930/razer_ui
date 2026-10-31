//! Validate the recorder transport envelope before its lifecycle consumes it.
use super::Callback;
use razer_platform::macro_recorder::Mode;
use serde::Deserialize;
use serde_json::Value;

pub(super) enum Event {
    Started { mode: Mode, callback: Callback },
    Items { bytes: usize, callback: Callback },
    Stopped { mode: Mode },
    Failed(String),
}

#[derive(Deserialize)]
struct Batch {
    overflow: bool,
    events: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Envelope {
    Started { mode: Mode },
    Item,
    Stopped { mode: Mode },
    Error { error: String },
}

pub(super) fn parse(source: Value) -> Result<Vec<Event>, String> {
    let batch: Batch = serde_json::from_value(source)
        .map_err(|error| format!("原生录制事件响应格式错误：{error}"))?;
    if batch.overflow {
        return Err("原生录制队列溢出，未提交不完整录制".into());
    }
    batch.events.into_iter().map(parse_event).collect()
}

fn parse_event(source: Value) -> Result<Event, String> {
    let envelope: Envelope = serde_json::from_value(source.clone())
        .map_err(|error| format!("原生录制事件格式错误：{error}"))?;
    Ok(match envelope {
        Envelope::Started { mode } => Event::Started {
            mode,
            callback: Callback::parse(&source)?.ok_or("录制 started 回调未解析")?,
        },
        Envelope::Item => Event::Items {
            bytes: source.to_string().len(),
            callback: Callback::parse(&source)?.ok_or("录制 item 回调未解析")?,
        },
        Envelope::Stopped { mode } => Event::Stopped { mode },
        Envelope::Error { error } => Event::Failed(error),
    })
}
