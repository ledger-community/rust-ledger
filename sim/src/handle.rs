//! Speculos runtime handle, provides out-of-band interaction with a simulator instance
//! via the [HTTP API](https://petstore.swagger.io/?url=https://raw.githubusercontent.com/LedgerHQ/speculos/master/speculos/api/static/swagger/swagger.json)
//! to allow button pushes, touch input, screenshots and display event observation
//! when executing integration tests.

use std::{io::Cursor, net::SocketAddr, time::Duration};

use async_trait::async_trait;
use image::{DynamicImage, ImageReader};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::debug;

use crate::{
    event::{Event, EventFilter, EventsResp},
    GenericHandle,
};

/// Interval between `/events` polls when awaiting an event
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Button enumeration
#[derive(Clone, Copy, PartialEq, Debug, Display)]
#[strum(serialize_all = "kebab-case")]
pub enum Button {
    Left,
    Right,
    Both,
}

/// Button actions
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize, Display)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Press,
    Release,
    PressAndRelease,
}

/// Button action object for serialisation and use with the HTTP API
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
struct ButtonAction {
    pub action: Action,
}

/// Touch (finger) action object for serialisation and use with the HTTP API
///
/// Optional fields are omitted when unset, leaving speculos to apply its own
/// defaults (`x2`/`y2` default to `x`/`y`, `delay` defaults to 0.1 seconds).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
struct TouchAction {
    pub action: Action,
    pub x: u16,
    pub y: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x2: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y2: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<f32>,
}

/// [Handle] trait for interacting with speculos
#[async_trait]
pub trait Handle {
    /// Get speculos HTTP address
    fn addr(&self) -> SocketAddr;

    /// Send a button action to the simulator
    async fn button(&self, button: Button, action: Action) -> anyhow::Result<()> {
        debug!("Sending button request: {}:{}", button, action);

        // Post action to HTTP API
        let r = Client::new()
            .post(format!("http://{}/button/{}", self.addr(), button))
            .json(&ButtonAction { action })
            .send()
            .await?;

        debug!("Button request complete: {}", r.status());

        Ok(())
    }

    /// Send a touch action to the simulator
    ///
    /// Only supported by touchscreen (NBGL) models, speculos rejects finger events
    /// for button-only devices.
    async fn touch(&self, x: u16, y: u16, action: Action) -> anyhow::Result<()> {
        finger(
            self.addr(),
            TouchAction {
                action,
                x,
                y,
                x2: None,
                y2: None,
                delay: None,
            },
        )
        .await
    }

    /// Tap (press and release) a point on the simulator screen
    async fn tap(&self, x: u16, y: u16) -> anyhow::Result<()> {
        self.touch(x, y, Action::PressAndRelease).await
    }

    /// Swipe from one point to another on the simulator screen
    async fn swipe(&self, from: (u16, u16), to: (u16, u16)) -> anyhow::Result<()> {
        finger(
            self.addr(),
            TouchAction {
                action: Action::PressAndRelease,
                x: from.0,
                y: from.1,
                x2: Some(to.0),
                y2: Some(to.1),
                delay: None,
            },
        )
        .await
    }

    /// Fetch a screenshot from the simulator
    async fn screenshot(&self) -> anyhow::Result<DynamicImage> {
        // Fetch screenshot from HTTP API
        let r = reqwest::get(format!("http://{}/screenshot", self.addr())).await?;

        // Read image bytes
        let b = r.bytes().await?;

        // Parse image object
        let i = ImageReader::new(Cursor::new(b))
            .with_guessed_format()?
            .decode()?;

        Ok(i)
    }

    /// Fetch display events from the simulator.
    ///
    /// With [`EventFilter::None`] this returns every event since the last
    /// [`clear_events`][Handle::clear_events], with [`EventFilter::CurrentScreen`]
    /// only those belonging to the screen currently displayed.
    async fn events(&self, filter: EventFilter) -> anyhow::Result<Vec<Event>> {
        debug!("Fetching events ({:?})", filter);

        // Fetch events from HTTP API
        let r = reqwest::get(format!("http://{}/events{}", self.addr(), filter.query())).await?;

        // Parse event list
        let e = r.json::<EventsResp>().await?;

        debug!("Received {} events", e.events.len());

        Ok(e.events)
    }

    /// Clear the accumulated event list.
    ///
    /// Note this only resets the list returned by [`EventFilter::None`], the current
    /// screen contents are unaffected.
    async fn clear_events(&self) -> anyhow::Result<()> {
        debug!("Clearing events");

        // Delete events via HTTP API
        let r = Client::new()
            .delete(format!("http://{}/events", self.addr()))
            .send()
            .await?;

        debug!("Clear events complete: {}", r.status());

        Ok(())
    }

    /// Await an event matching the provided filter, polling until `timeout` elapses.
    ///
    /// [`EventFilter::None`] is the appropriate choice for most waits as the event
    /// list accumulates, so a screen that comes and goes between polls is still
    /// matched. [`EventFilter::CurrentScreen`] can miss such transient screens.
    ///
    /// ```no_run
    /// # use std::time::Duration;
    /// # use ledger_sim::{Handle, EventFilter};
    /// # async fn example(handle: impl Handle + Sync) -> anyhow::Result<()> {
    /// let e = handle
    ///     .wait_event(EventFilter::None, Duration::from_secs(5), &|e| e.text == "Ready")
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn wait_event(
        &self,
        filter: EventFilter,
        timeout: Duration,
        f: &(dyn for<'a> Fn(&'a Event) -> bool + Send + Sync),
    ) -> anyhow::Result<Event> {
        let poll = async {
            loop {
                // Fetch the current event list
                let events = self.events(filter).await?;

                // Return the first matching event if available
                if let Some(e) = events.into_iter().find(|e| f(e)) {
                    return Ok(e);
                }

                // Otherwise wait and retry
                tokio::time::sleep(DEFAULT_POLL_INTERVAL).await;
            }
        };

        match tokio::time::timeout(timeout, poll).await {
            Ok(v) => v,
            Err(_) => Err(anyhow::anyhow!("timeout awaiting event")),
        }
    }

    /// Await an event containing the provided text, see [`wait_event`][Handle::wait_event]
    async fn wait_text(&self, text: &str, timeout: Duration) -> anyhow::Result<Event> {
        self.wait_event(EventFilter::None, timeout, &|e| e.text.contains(text))
            .await
    }
}

/// Post a [TouchAction] to the speculos HTTP API
async fn finger(addr: SocketAddr, t: TouchAction) -> anyhow::Result<()> {
    debug!("Sending touch request: {:?}", t);

    // Post action to HTTP API
    let r = Client::new()
        .post(format!("http://{addr}/finger"))
        .json(&t)
        .send()
        .await?;

    debug!("Touch request complete: {}", r.status());

    // Surface speculos rejections (invalid coordinates, non-touch model)
    r.error_for_status()?;

    Ok(())
}

impl Handle for GenericHandle {
    fn addr(&self) -> SocketAddr {
        match self {
            GenericHandle::Local(h) => h.addr(),
            GenericHandle::Docker(h) => h.addr(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Check button string encoding
    #[test]
    fn button_encoding() {
        let tests = &[
            (Button::Left, "left"),
            (Button::Right, "right"),
            (Button::Both, "both"),
        ];

        for (v, s) in tests {
            assert_eq!(&v.to_string(), s);
        }
    }

    /// Check button action encoding
    #[test]
    fn action_encoding() {
        let tests = &[
            (
                ButtonAction {
                    action: Action::Press,
                },
                r#"{"action":"press"}"#,
            ),
            (
                ButtonAction {
                    action: Action::Release,
                },
                r#"{"action":"release"}"#,
            ),
            (
                ButtonAction {
                    action: Action::PressAndRelease,
                },
                r#"{"action":"press-and-release"}"#,
            ),
        ];

        for (v, s) in tests {
            assert_eq!(&serde_json::to_string(v).unwrap(), s);
        }
    }

    /// Check touch action encoding
    #[test]
    fn touch_encoding() {
        let tests = &[
            // Simple touch, no optional keys emitted
            (
                TouchAction {
                    action: Action::Press,
                    x: 100,
                    y: 200,
                    x2: None,
                    y2: None,
                    delay: None,
                },
                r#"{"action":"press","x":100,"y":200}"#,
            ),
            // Swipe, x2/y2 present and delay still omitted
            (
                TouchAction {
                    action: Action::PressAndRelease,
                    x: 100,
                    y: 200,
                    x2: Some(100),
                    y2: Some(400),
                    delay: None,
                },
                r#"{"action":"press-and-release","x":100,"y":200,"x2":100,"y2":400}"#,
            ),
            // Delay is serialised when set
            (
                TouchAction {
                    action: Action::PressAndRelease,
                    x: 1,
                    y: 2,
                    x2: None,
                    y2: None,
                    delay: Some(0.5),
                },
                r#"{"action":"press-and-release","x":1,"y":2,"delay":0.5}"#,
            ),
        ];

        for (v, s) in tests {
            assert_eq!(&serde_json::to_string(v).unwrap(), s);
        }
    }
}
