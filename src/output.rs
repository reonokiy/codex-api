//! Codex emits output items separately and may leave terminal response.output empty.
//! Assemble only the public API view; the native protocol remains untouched.
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
#[derive(Default)]
pub struct OutputAccumulator {
    lanes: HashMap<String, BTreeMap<u64, Value>>,
}
impl OutputAccumulator {
    /// Returns true when the terminal event was enriched.
    pub fn observe(&mut self, event: &mut Value) -> bool {
        let lane = event
            .get("stream_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        match event["type"].as_str() {
            Some("response.created") => {
                self.lanes.remove(&lane);
            }
            Some("response.output_item.done") => {
                if let (Some(index), Some(item)) = (
                    event["output_index"].as_u64(),
                    event.get("item").filter(|v| v.is_object()),
                ) {
                    self.lanes
                        .entry(lane)
                        .or_default()
                        .insert(index, item.clone());
                }
            }
            Some("response.completed" | "response.failed" | "response.incomplete") => {
                let items = self.lanes.remove(&lane).unwrap_or_default();
                if !items.is_empty()
                    && event["response"].is_object()
                    && event["response"]
                        .get("output")
                        .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
                {
                    event["response"]["output"] = Value::Array(items.into_values().collect());
                    return true;
                }
            }
            _ => {}
        }
        false
    }
}
