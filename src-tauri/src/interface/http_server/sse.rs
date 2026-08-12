use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::StreamExt;
use futures::stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::shared::event_bus::EventHandler;

use super::state::HttpServerState;

const RING_CAPACITY: usize = 100;
const CHANNEL_CAPACITY: usize = 256;
const KEEPALIVE_SECS: u64 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseJobEvent {
    pub id: u64,
    pub event_type: String,
    pub data: serde_json::Value,
}

pub struct SseBroadcaster {
    next_id: AtomicU64,
    tx: broadcast::Sender<SseJobEvent>,
    ring: RwLock<VecDeque<SseJobEvent>>,
}

impl SseBroadcaster {
    pub fn new() -> Arc<Self> {
        let (tx, _rx) = broadcast::channel(CHANNEL_CAPACITY);
        Arc::new(Self {
            next_id: AtomicU64::new(1),
            tx,
            ring: RwLock::new(VecDeque::with_capacity(RING_CAPACITY)),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SseJobEvent> {
        self.tx.subscribe()
    }

    pub fn replay_since(&self, last_event_id: u64) -> Vec<SseJobEvent> {
        let ring = self.ring.read().unwrap();
        ring.iter()
            .filter(|e| e.id > last_event_id)
            .cloned()
            .collect()
    }

    fn push(&self, event: SseJobEvent) {
        {
            let mut ring = self.ring.write().unwrap();
            if ring.len() == RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(event.clone());
        }
        let _ = self.tx.send(event);
    }
}

impl EventHandler for SseBroadcaster {
    fn handle(&self, event_type: &str, payload: &str) {
        let data = serde_json::from_str::<serde_json::Value>(payload)
            .unwrap_or_else(|_| serde_json::Value::String(payload.to_string()));
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.push(SseJobEvent {
            id,
            event_type: event_type.to_string(),
            data,
        });
    }
}

// ==================== HTTP handler ====================

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    pub token: String,
}

pub async fn sse_stream(
    State(state): State<HttpServerState>,
    Query(q): Query<EventsQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    // Verify token (query, không phải header — EventSource giới hạn).
    if state.token_manager.verify_token(&q.token).is_none() {
        tracing::warn!("SSE auth rejected (token=***)");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let Some(broadcaster) = state.sse_broadcaster.clone() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    let last_id = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let replay = broadcaster.replay_since(last_id);
    let rx = broadcaster.subscribe();
    let live = tokio_stream::wrappers::BroadcastStream::new(rx);

    let replay_stream = stream::iter(replay).map(Ok::<_, Infallible>);
    let live_stream = live.filter_map(|res| async {
        match res {
            Ok(ev) => Some(Ok::<_, Infallible>(ev)),
            Err(_) => None,
        }
    });

    let events = replay_stream
        .chain(live_stream)
        .map(|res| res.map(sse_event_from));

    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(KEEPALIVE_SECS))
            .text("ping"),
    ))
}

fn sse_event_from(ev: SseJobEvent) -> Event {
    let payload = serde_json::to_string(&ev.data).unwrap_or_else(|_| "{}".into());
    Event::default()
        .id(ev.id.to_string())
        .event(ev.event_type)
        .data(payload)
}

// ==================== Log scrub ====================

pub fn scrub_token_query(uri: &str) -> String {
    // Replace token=... với token=*** để log không lộ.
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"([?&]token=)[^&]+").unwrap());
    re.replace_all(uri, "$1***").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_masks_token() {
        let uri = "/api/v1/events?token=abcd1234&foo=bar";
        assert_eq!(scrub_token_query(uri), "/api/v1/events?token=***&foo=bar");
    }

    #[test]
    fn scrub_masks_middle_token() {
        let uri = "/api/v1/events?foo=bar&token=deadbeef";
        assert_eq!(scrub_token_query(uri), "/api/v1/events?foo=bar&token=***");
    }

    #[test]
    fn scrub_leaves_no_token_alone() {
        assert_eq!(scrub_token_query("/api/v1/ping"), "/api/v1/ping");
    }

    #[test]
    fn broadcaster_handles_event() {
        let b = SseBroadcaster::new();
        b.handle("PrintJobCreated", r#"{"job_id":"x","status":"PENDING"}"#);
        let replay = b.replay_since(0);
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].event_type, "PrintJobCreated");
        assert_eq!(replay[0].id, 1);
    }

    #[test]
    fn ring_evicts_oldest_after_capacity() {
        let b = SseBroadcaster::new();
        for i in 0..(RING_CAPACITY + 10) {
            b.handle("Test", &format!(r#"{{"seq":{}}}"#, i));
        }
        let replay = b.replay_since(0);
        assert_eq!(replay.len(), RING_CAPACITY);
        // First remaining id should be event 11 (indices 10..=109 kept).
        assert_eq!(replay[0].id, 11);
    }

    #[test]
    fn replay_filters_by_last_id() {
        let b = SseBroadcaster::new();
        for _ in 0..5 {
            b.handle("Test", r#"{"a":1}"#);
        }
        let replay = b.replay_since(3);
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].id, 4);
    }
}
