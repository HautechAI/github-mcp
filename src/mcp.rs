use serde_json::Value;
use std::cell::Cell;

// Thread-local flag indicating whether to include rate meta in outputs for the current tools/call.
// Use non-const initializer to avoid raising MSRV; silence clippy on newer compilers.
#[allow(clippy::missing_const_for_thread_local)]
thread_local! {
    // Use const initializer to satisfy clippy::missing_const_for_thread_local (Rust 1.90).
    static INCLUDE_RATE: Cell<bool> = const { Cell::new(false) };
}

// Set the include-rate flag for the current thread (one tools/call invocation).
pub fn set_include_rate(flag: bool) {
    // Retained for compatibility; prefer IncludeRateGuard for scoped changes.
    INCLUDE_RATE.with(|c| c.set(flag));
}

// RAII guard to scope INCLUDE_RATE to a call.
// Restores the previous value when dropped, preventing leakage on early returns.
pub struct IncludeRateGuard(bool);

impl IncludeRateGuard {
    pub fn set(flag: bool) -> Self {
        let prev = INCLUDE_RATE.with(|c| {
            let p = c.get();
            c.set(flag);
            p
        });
        Self(prev)
    }
}

impl Drop for IncludeRateGuard {
    fn drop(&mut self) {
        INCLUDE_RATE.with(|c| c.set(self.0));
    }
}

fn current_include_rate() -> bool {
    INCLUDE_RATE.with(|c| c.get())
}

// Prune meta fields according to include_rate and has_more.
// - When has_more is false/missing: drop has_more and next_cursor.
// - When include_rate is false: drop rate.
// - Drop meta entirely if it becomes empty.
fn prune_meta(structured: &mut Value, include_rate: bool) {
    let Some(obj) = structured.as_object_mut() else {
        return;
    };
    let Some(meta_val) = obj.get_mut("meta") else {
        return;
    };
    let Some(meta_obj) = meta_val.as_object_mut() else {
        return;
    };

    let has_more = meta_obj
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !has_more {
        meta_obj.remove("has_more");
        meta_obj.remove("next_cursor");
    }
    if !include_rate {
        meta_obj.remove("rate");
    }

    if meta_obj.is_empty() {
        obj.remove("meta");
    }
}

// Build an MCP-compliant result envelope for tools/call outputs.
// - content: always a single text block so clients can render something.
// - structuredContent: preserves the previous structured JSON shape to minimize breakage.
// - isError: included only when true to keep payloads small.
pub fn mcp_wrap(mut structured: Value, text_opt: Option<String>, is_error: bool) -> Value {
    // Apply output shaping immediately before wrapping.
    let include_rate = current_include_rate();
    prune_meta(&mut structured, include_rate);
    let text = match text_opt {
        Some(s) => s,
        None => serde_json::to_string(&structured).unwrap_or_else(|_| "{}".to_string()),
    };
    let mut obj = serde_json::json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": structured,
    });
    if is_error {
        if let Some(map) = obj.as_object_mut() {
            map.insert("isError".to_string(), Value::Bool(true));
        }
    }
    obj
}
