#![recursion_limit = "512"]

use std::{borrow::Cow, time::Duration};

use anyhow::Result;
use yew::{
    format::MsgPack,
    html,
    services::{
        websocket::{WebSocketStatus, WebSocketTask},
        WebSocketService,
    },
    Component, ComponentLink, Html, ShouldRender,
};

use rradio_messages::{ArcStr, Command, PipelineState, PlayerStateDiff, Station, TrackTags};

mod player;
mod podcasts;

struct DurationDisplay(Duration);

impl std::fmt::Display for DurationDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_secs = self.0.as_secs();

        let total_mins = total_secs / 60;
        let secs = total_secs % 60;

        let hours = total_mins / 60;
        let mins = total_mins % 60;

        if hours > 0 {
            write!(f, "{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            write!(f, "{:02}:{:02}", mins, secs)
        }
    }
}

trait CreatesOptionalCallback<M> {
    fn result_callback<F, IN>(&self, function: F) -> yew::Callback<IN>
    where
        F: Fn(IN) -> Result<M> + 'static;
}

impl<COMP: yew::Component, M: Into<COMP::Message>> CreatesOptionalCallback<M>
    for yew::ComponentLink<COMP>
{
    fn result_callback<F, IN>(&self, function: F) -> yew::Callback<IN>
    where
        F: Fn(IN) -> Result<M> + 'static,
    {
        let scope = self.clone();
        let closure = move |input| match function(input) {
            Ok(output) => scope.send_message(output),
            Err(err) => log::error!("{:#}", err),
        };
        closure.into()
    }
}

fn update_value<T>(current_value: &mut T, diff_value: Option<T>) {
    if let Some(new_value) = diff_value {
        *current_value = new_value;
    }
}

fn update_option<T>(current_value: &mut Option<T>, diff_value: rradio_messages::OptionDiff<T>) {
    match diff_value {
        rradio_messages::OptionDiff::NoChange => (),
        rradio_messages::OptionDiff::ChangedToNone => {
            *current_value = None;
        }
        rradio_messages::OptionDiff::ChangedToSome(value) => {
            *current_value = Some(value);
        }
    }
}

#[allow(clippy::large_enum_variant)]
enum ConnectionState {
    NotConnected,
    IncompatibleVersion(ArcStr),
    HasConnection {
        task: WebSocketTask,
        is_connected: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct PlayerState {
    pub pipeline_state: PipelineState,
    pub current_station: Option<Station>,
    pub current_track_index: usize,
    pub current_track_tags: Option<TrackTags>,
    pub volume: i32,
    pub buffering: u8,
    pub track_duration: Option<Duration>,
    pub track_position: Option<Duration>,
}

impl PlayerState {
    fn update(&mut self, diff: PlayerStateDiff) {
        update_value(&mut self.pipeline_state, diff.pipeline_state);
        update_option(&mut self.current_station, diff.current_station);
        update_value(&mut self.current_track_index, diff.current_track_index);
        update_option(&mut self.current_track_tags, diff.current_track_tags);
        update_value(&mut self.volume, diff.volume);
        update_value(&mut self.buffering, diff.buffering);
        update_option(&mut self.track_duration, diff.track_duration);
        update_option(&mut self.track_position, diff.track_position);
    }
}

enum Msg {
    WebsocketMessageReceived(MsgPack<Result<rradio_messages::Event>>),
    WebsocketStatusChanged(WebSocketStatus),
    SendCommand(Command),
}

enum CurrentView {
    Player,
    Podcasts,
}

struct AppState {
    link: ComponentLink<Self>,
    connection: ConnectionState,
    player_state: PlayerState,
    current_view: CurrentView,
}

impl Component for AppState {
    type Message = Msg;
    type Properties = ();

    fn create(_props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let host = yew::utils::host().unwrap();
        let host = host.split_once(":").map_or(host.as_str(), |(host, _)| host);
        let url = format!("ws://{}:8000", host);
        log::info!("Connecting to {}", url);
        let connection = match WebSocketService::connect_binary(
            &url,
            link.callback(Msg::WebsocketMessageReceived),
            link.callback(Msg::WebsocketStatusChanged),
        ) {
            Ok(task) => ConnectionState::HasConnection {
                task,
                is_connected: false,
            },
            Err(err) => {
                log::error!("Could not connect to server: {:#}", err);
                ConnectionState::NotConnected
            }
        };

        let path = yew::utils::document()
            .location()
            .and_then(|location| {
                location
                    .pathname()
                    .map_err(|err| log::error!("No pathname: {:?}", err.as_string()))
                    .ok()
            })
            .unwrap_or_default();

        let current_view = match path.as_str() {
            "/podcasts" => CurrentView::Podcasts,
            _ => CurrentView::Player,
        };

        Self {
            link,
            connection,
            player_state: PlayerState::default(),
            current_view,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::WebsocketMessageReceived(MsgPack(msg)) => match msg {
                Ok(rradio_messages::Event::ProtocolVersion(version)) => {
                    if version != rradio_messages::VERSION {
                        log::error!(
                            "Bad message version. Mine: {}. Theirs: {}",
                            rradio_messages::VERSION,
                            version
                        );
                        self.connection = ConnectionState::IncompatibleVersion(version);
                        true
                    } else {
                        false
                    }
                }
                Ok(rradio_messages::Event::PlayerStateChanged(diff)) => {
                    // log::info!("State Changes: {:?}", diff);
                    self.player_state.update(diff);
                    true
                }
                Ok(rradio_messages::Event::LogMessage(rradio_messages::LogMessage::Error(
                    message,
                ))) => {
                    log::error!("{}", message);
                    false
                }
                Err(err) => {
                    log::error!("Bad Websocket message: {:#}", err);
                    self.connection = ConnectionState::NotConnected;
                    true
                }
            },
            Msg::WebsocketStatusChanged(status) => match status {
                WebSocketStatus::Opened => {
                    log::info!("Websocket connection");
                    if let ConnectionState::HasConnection { is_connected, .. } =
                        &mut self.connection
                    {
                        *is_connected = true;
                    }

                    true
                }
                WebSocketStatus::Closed => {
                    log::info!("Websocket closed");
                    self.connection = ConnectionState::NotConnected;
                    true
                }
                WebSocketStatus::Error => {
                    log::error!("Websocket Error");
                    self.connection = ConnectionState::NotConnected;
                    true
                }
            },
            Msg::SendCommand(command) => {
                if let ConnectionState::HasConnection {
                    task,
                    is_connected: true,
                } = &mut self.connection
                {
                    task.send_binary(MsgPack(&command))
                }
                false
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        let connection_state = match &self.connection {
            ConnectionState::NotConnected => Some(Cow::Borrowed("Not Connected")),
            ConnectionState::IncompatibleVersion(version) => Some(Cow::Owned(format!(
                "Incompatible Version: Client = {}, Server = {}",
                rradio_messages::VERSION,
                version
            ))),
            ConnectionState::HasConnection {
                is_connected: false,
                ..
            } => Some(Cow::Borrowed("Connecting")),
            ConnectionState::HasConnection {
                is_connected: true, ..
            } => None,
        };

        let connection_header_visible = connection_state.as_ref().map(|_| "visible");
        let connection_state = connection_state.unwrap_or_default();

        let send_command = self.link.callback(Msg::SendCommand);
        let play_url = self
            .link
            .callback(|url: String| Msg::SendCommand(Command::PlayUrl(url)));

        let current_view = match self.current_view {
            CurrentView::Player => {
                html! { <player::Player player_state=self.player_state.clone() send_command=send_command /> }
            }
            CurrentView::Podcasts => {
                html! { <podcasts::Podcasts play_url=play_url /> }
            }
        };

        html! {
            <body class="app">
                <header class=yew::classes!(connection_header_visible)><output>{connection_state}</output></header>
                {current_view}
            </body>
        }
    }
}

pub fn main() {
    wasm_logger::init(wasm_logger::Config::default());

    yew::App::<AppState>::new().mount_as_body();
}
