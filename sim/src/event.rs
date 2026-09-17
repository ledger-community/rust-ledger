//! Speculos display events, fetched via the
//! [HTTP API](https://petstore.swagger.io/?url=https://raw.githubusercontent.com/LedgerHQ/speculos/master/speculos/api/static/swagger/swagger.json)
//! to allow integration tests to observe what the simulated device is displaying.

use serde::{Deserialize, Serialize};

/// A text event emitted by the simulated device display
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Displayed text
    pub text: String,
    /// Horizontal position
    pub x: i32,
    /// Vertical position
    pub y: i32,
    /// Text width
    #[serde(default)]
    pub w: i32,
    /// Text height
    #[serde(default)]
    pub h: i32,
    /// Screen clear flag
    #[serde(default)]
    pub clear: bool,
}

/// Filter for event queries, see [`Handle::events`][crate::Handle::events]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub enum EventFilter {
    /// All events since the last [`Handle::clear_events`][crate::Handle::clear_events]
    #[default]
    None,
    /// Only events belonging to the current screen
    CurrentScreen,
}

impl EventFilter {
    /// Query string suffix for use with the `/events` endpoint
    pub(crate) fn query(&self) -> &'static str {
        match self {
            EventFilter::None => "",
            EventFilter::CurrentScreen => "?currentscreenonly=true",
        }
    }
}

/// Wrapper for the `/events` HTTP response
#[derive(Clone, PartialEq, Debug, Deserialize)]
pub(crate) struct EventsResp {
    pub events: Vec<Event>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Check event decoding
    #[test]
    fn event_decoding() {
        let s = r#"{"events":[{"text":"Application","x":35,"y":3,"w":10,"h":8,"clear":false}]}"#;

        let r: EventsResp = serde_json::from_str(s).unwrap();

        assert_eq!(
            r.events,
            vec![Event {
                text: "Application".to_string(),
                x: 35,
                y: 3,
                w: 10,
                h: 8,
                clear: false,
            }]
        );
    }

    /// Older speculos versions omit `w`, `h` and `clear`
    #[test]
    fn event_decoding_legacy() {
        let s = r#"{"events":[{"text":"is ready","x":44,"y":17}]}"#;

        let r: EventsResp = serde_json::from_str(s).unwrap();

        assert_eq!(
            r.events,
            vec![Event {
                text: "is ready".to_string(),
                x: 44,
                y: 17,
                w: 0,
                h: 0,
                clear: false,
            }]
        );
    }

    /// Check event filter query encoding
    #[test]
    fn filter_encoding() {
        assert_eq!(EventFilter::default(), EventFilter::None);

        assert_eq!(EventFilter::None.query(), "");
        assert_eq!(
            EventFilter::CurrentScreen.query(),
            "?currentscreenonly=true"
        );
    }
}
