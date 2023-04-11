#![allow(clippy::never_loop)]

use std::{fmt, time::Duration};

use anyhow::Context;
use dioxus::prelude::*;
use futures_util::{FutureExt, SinkExt, StreamExt};
use gloo_storage::Storage;

use rradio_messages::ArcStr;

mod fast_eq_rc;
use fast_eq_rc::FastEqRc;

mod update_from_diff;
use update_from_diff::UpdateFromDiff;

mod debug_view;
mod player_state_view;
mod podcasts_view;
mod track_position_slider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    PlayerState,
    Podcasts,
    Debug,
}

impl AppView {
    fn classname(self) -> &'static str {
        match self {
            AppView::PlayerState => "player-state",
            AppView::Podcasts => "podcasts",
            AppView::Debug => "debug",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    ConnectionError(ArcStr),
}

impl ConnectionState {
    pub fn handle_closed(
        connection_state: UseState<ConnectionState>,
    ) -> impl Fn(anyhow::Result<()>) {
        move |result: anyhow::Result<()>| {
            connection_state.set(match result {
                Ok(()) => Self::Disconnected,
                Err(err) => Self::ConnectionError(rradio_messages::arcstr::format!("{:#}", err)),
            });
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerState {
    pub pipeline_state: rradio_messages::PipelineState,
    pub current_station: FastEqRc<Option<rradio_messages::Station>>,
    pub pause_before_playing: Option<Duration>,
    pub current_track_index: usize,
    pub current_track_tags: FastEqRc<Option<rradio_messages::TrackTags>>,
    pub is_muted: bool,
    pub volume: i32,
    pub buffering: u8,
    pub track_duration: Option<Duration>,
    pub track_position: Option<Duration>,
    pub ping_times: rradio_messages::PingTimes,
}

impl UpdateFromDiff<rradio_messages::PlayerStateDiff> for PlayerState {
    fn update_from_diff(&mut self, diff: rradio_messages::PlayerStateDiff) {
        let rradio_messages::PlayerStateDiff {
            pipeline_state,
            current_station,
            pause_before_playing,
            current_track_index,
            current_track_tags,
            is_muted,
            volume,
            buffering,
            track_duration,
            track_position,
            ping_times,
        } = diff;

        self.pipeline_state.update_from_diff(pipeline_state);
        self.current_station.update_from_diff(current_station);
        self.pause_before_playing
            .update_from_diff(pause_before_playing);
        self.current_track_index
            .update_from_diff(current_track_index);
        self.current_track_tags.update_from_diff(current_track_tags);
        self.is_muted.update_from_diff(is_muted);
        self.volume.update_from_diff(volume);
        self.buffering.update_from_diff(buffering);
        self.track_duration.update_from_diff(track_duration);
        self.track_position.update_from_diff(track_position);
        self.ping_times.update_from_diff(ping_times);
    }
}

struct DisplayDuration(Duration);

impl fmt::Display for DisplayDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();

        write!(f, "{:02}:{:02}", secs / 60, secs % 60)
    }
}

fn handle_input<T, F>(f: F, value: &str, commands: &Coroutine<rradio_messages::Command>)
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
    F: Fn(T) -> rradio_messages::Command,
{
    match T::from_str(value) {
        Ok(value) => commands.send(f(value)),
        Err(err) => {
            tracing::warn!("Failed to handle input value {value:?}: {err}")
        }
    }
}
enum AppCommand {
    Command(rradio_messages::Command),
    Event(Result<gloo_net::websocket::Message, gloo_net::websocket::WebSocketError>),
}

#[allow(non_snake_case)]
#[inline_props]
fn ConnectionStateView(cx: Scope, connection_state: ConnectionState) -> Element {
    let connection_state_view = match connection_state {
        ConnectionState::Connecting => Some(rsx! { "Connecting..." }),
        ConnectionState::Connected => None,
        ConnectionState::Disconnected => Some(rsx! { "RRadio has terminated" }),
        ConnectionState::ConnectionError(err) => Some(rsx! { "{err}" }),
    }
    .and_then(|message| {
        cx.render(rsx! {
            header {
                id: "connection-message",
                output { message }
            }
        })
    });

    cx.render(rsx! { connection_state_view })
}

#[allow(non_snake_case)]
#[inline_props]
fn Root(cx: Scope, app_view: AppView) -> Element {
    let (connection_state, use_connection_state) =
        use_state(&cx, || ConnectionState::Connecting).split();
    let (player_state, use_player_state) = use_state(&cx, PlayerState::default).split();

    use_coroutine::<rradio_messages::Command, _, _>(&cx, {
        let player_state_store = use_player_state.clone();
        |mut commands| {
            {
                let use_connection_state = use_connection_state.clone();
                async move {
                    let host = gloo_storage::LocalStorage::raw()
                        .get_item("RRADIO_SERVER")
                        .expect("unreachable: get_item does not throw an exception")
                        .map_or_else(
                            || {
                                web_sys::window()
                                    .context("No Window!")?
                                    .location()
                                    .host()
                                    .map_err(|err| anyhow::anyhow!("No hostname: {:?}", err))
                            },
                            Ok,
                        )?;

                    let api_url = format!("ws://{host}/api");

                    let mut is_first_connection_attempt = true;

                    loop {
                        let result = async {
                            let (mut websocket_tx, websocket_rx) =
                                gloo_net::websocket::futures::WebSocket::open_with_protocol(
                                    &api_url,
                                    rradio_messages::API_VERSION_HEADER.trim(),
                                )
                                .map_err(|err| {
                                    anyhow::anyhow!("Failed to open websocket: {err:?}")
                                })?
                                .split();

                            is_first_connection_attempt = false;
                            use_connection_state.set(ConnectionState::Connected);

                            let app_commands = futures_util::stream::select(
                                (&mut commands).map(AppCommand::Command),
                                websocket_rx.map(AppCommand::Event),
                            );

                            futures_util::pin_mut!(app_commands);

                            while let Some(app_command) = app_commands.next().await {
                                match app_command {
                                    AppCommand::Command(rradio_command) => {
                                        let mut buffer = Vec::new();
                                        rradio_command
                                            .encode(&mut buffer)
                                            .context("Failed to encode Command")?;

                                        websocket_tx
                                            .send(gloo_net::websocket::Message::Bytes(buffer))
                                            .await
                                            .map_err(|err| {
                                                anyhow::anyhow!(
                                                    "Failed to send websocket message: {err}"
                                                )
                                            })?;
                                    }
                                    AppCommand::Event(Err(
                                        gloo_net::websocket::WebSocketError::ConnectionClose(_),
                                    )) => break,
                                    AppCommand::Event(rradio_event) => {
                                        match rradio_event.map_err(|err| {
                                            anyhow::anyhow!(
                                                "Failed to receive websocket message: {err}"
                                            )
                                        })? {
                                            gloo_net::websocket::Message::Text(message) => {
                                                tracing::warn!("Ignoring text message: {message:?}")
                                            }
                                            gloo_net::websocket::Message::Bytes(mut buffer) => {
                                                match rradio_messages::Event::decode(&mut buffer)
                                                    .context("Failed to decode Event")?
                                                {
                                                    rradio_messages::Event::PlayerStateChanged(
                                                        diff,
                                                    ) => {
                                                        player_state_store.with_mut(
                                                            |current_player_state| {
                                                                current_player_state
                                                                    .update_from_diff(diff);
                                                            },
                                                        );
                                                    }
                                                    rradio_messages::Event::LogMessage(
                                                        rradio_messages::LogMessage::Error(error),
                                                    ) => tracing::error!("error: {error:#}"),
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            anyhow::Ok(())
                        }
                        .await;

                        match result {
                            Ok(()) => return Ok(()),
                            Err(err) if is_first_connection_attempt => return Err(err),
                            Err(err) => {
                                use_connection_state.set(ConnectionState::ConnectionError(
                                    rradio_messages::arcstr::format!("{:#}", err),
                                ));
                            }
                        }

                        // Wait and then try to reconnect
                        gloo_timers::future::sleep(std::time::Duration::from_secs(3)).await;
                    }
                }
            }
            .map(ConnectionState::handle_closed(use_connection_state.clone()))
        }
    });

    let app = match app_view {
        AppView::PlayerState => {
            rsx! { player_state_view::View { player_state: player_state.clone() } }
        }
        AppView::Podcasts => rsx! { podcasts_view::View { player_state: player_state.clone() } },
        AppView::Debug => {
            rsx! { debug_view::View { connection_state: connection_state.clone(), player_state: player_state.clone() } }
        }
    };

    cx.render(rsx! {
        ConnectionStateView { connection_state: connection_state.clone() }
        nav {
            a { href: "?player", "Player" },
            a { href: "?podcasts", "Podcasts" }
            a { href: "?debug", "Debug" }
        }
        app
    })
}

fn main() {
    console_error_panic_hook::set_once();

    let max_level = gloo_storage::LocalStorage::raw()
        .get("RRADIO_LOGGING")
        .unwrap()
        .map_or(tracing::Level::INFO, |level| {
            level.parse().expect("Logging level")
        });

    tracing_wasm::set_as_global_default_with_config(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_max_level(max_level)
            .build(),
    );

    let app_view = match gloo_utils::window()
        .location()
        .search()
        .expect("search")
        .as_str()
    {
        "?podcast" | "?podcasts" => AppView::Podcasts,
        "?debug" => AppView::Debug,
        _ => AppView::PlayerState,
    };

    let root_element = "app";

    let main = gloo_utils::document()
        .get_element_by_id(root_element)
        .expect(r#"no element "app""#);

    main.set_class_name(app_view.classname());
    main.set_inner_html("");

    dioxus_web::launch_with_props(
        Root,
        RootProps { app_view },
        dioxus_web::Config::new().rootname(root_element),
    );
}
