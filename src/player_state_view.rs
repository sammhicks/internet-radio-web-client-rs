use std::{fmt, time::Duration};

use dioxus::prelude::*;

use rradio_messages::Track;

use crate::{DisplayDuration, FastEqRc, PlayerState};

fn handle_input<T, F>(f: F, value: &str, commands: &CoroutineHandle<rradio_messages::Command>)
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

#[allow(non_snake_case)]
#[inline_props]
fn CurrentTrack(
    cx: Scope,
    track: rradio_messages::Track,
    tags: FastEqRc<Option<rradio_messages::TrackTags>>,
) -> Element<'a> {
    tracing::debug!(?track, ?tags, "CurrentTrack");

    let tags = tags.as_ref().as_ref();

    let title = tags
        .and_then(|tags| tags.title.as_deref())
        .or(track.title.as_deref())
        .unwrap_or_default();
    let artist = tags
        .and_then(|tags| tags.artist.as_deref())
        .or(track.artist.as_deref())
        .unwrap_or_default();
    let album = tags
        .and_then(|tags| tags.album.as_deref())
        .or(track.album.as_deref())
        .unwrap_or_default();
    let genre = tags
        .and_then(|tags| tags.genre.as_deref())
        .unwrap_or_default();

    let image = tags
        .and_then(|tags| tags.image.as_deref())
        .unwrap_or_default();

    cx.render(rsx! {
        div {
            id: "current-track",
            style: r#"background-image: url("{image}")"#,
            div {
                id: "current-track-tags",
                div { "{title}" }
                div { "{artist}" }
                div { "{album}" }
                div { "{genre}" }
            }
        }
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn PlaylistTrack<'a>(
    cx: Scope<'a>,
    track_index: usize,
    track: &'a rradio_messages::Track,
    is_current_track: bool,
) -> Element<'a> {
    tracing::debug!(?track_index, ?track, ?is_current_track, "PlaylistTrack");

    let commands = use_coroutine_handle::<rradio_messages::Command>(&cx).expect("Commands");

    let contents = if track.is_notification {
        rsx! { "<Notification>" }
    } else {
        match (&track.title, &track.artist) {
            (Some(title), Some(artist)) => rsx! { "{title} - {artist}" },
            (Some(title), None) => rsx! { "{title}" },
            (None, _) => track.url.rsplit_once('/').map_or_else(
                || rsx! { "{track.url}" },
                |(_, name)| match urlencoding::decode(name) {
                    Ok(name) => rsx! { "{name}" },
                    Err(_) => rsx! { "{name}" },
                },
            ),
        }
    };

    let class_name = if *is_current_track {
        "current-track"
    } else {
        ""
    };

    let track_index = *track_index;

    cx.render(rsx! {
        div {
            class: "{class_name}",
            onclick: move |_| commands.send(rradio_messages::Command::NthItem(track_index)),
            contents
        }
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn Station(
    cx: Scope,
    station: FastEqRc<Option<rradio_messages::Station>>,
    current_track_index: usize,
) -> Element {
    tracing::debug!(?station, ?current_track_index, "Station");

    match station.as_ref() {
        Some(station) => {
            let rradio_messages::Station {
                index,
                source_type,
                title,
                tracks,
            } = station;

            let legend = match index {
                Some(index) => rsx! { "Station {index}" },
                None => rsx! { "Station" },
            };

            let title = match &title {
                Some(title) => rsx! { "{title}" },
                None => rsx! { "{source_type}" },
            };

            let tracks = tracks
                .as_ref()
                .map_or::<&[Track], _>(&[], |tracks| tracks.as_ref())
                .iter()
                .enumerate()
                .map(|(track_index, track)| {
                    let is_current_track = track_index == *current_track_index;
                    rsx! { PlaylistTrack { key: "Track{track_index}", track_index: track_index, track: track, is_current_track: is_current_track } }
                });

            cx.render(rsx! {
                fieldset {
                    id: "current-station",
                    legend { legend }
                    div { id: "current-station-title", title }
                    tracks
                }
            })
        }
        None => cx.render(rsx! {
            fieldset {
                id: "current-station",
                legend { "No Station" }
            }
        }),
    }
}

#[derive(Clone, Copy)]
enum TrackPositionText {
    NoTrack,
    Track {
        position: Duration,
        duration: Duration,
    },
}

impl fmt::Display for TrackPositionText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrackPositionText::NoTrack => write!(f, "--"),
            TrackPositionText::Track { position, duration } => write!(
                f,
                "{} - {}",
                DisplayDuration(position),
                DisplayDuration(duration)
            ),
        }
    }
}

struct TrackPositionSliderValues {
    disabled: bool,
    position: u64,
    duration: u64,
}

impl From<TrackPositionText> for TrackPositionSliderValues {
    fn from(track_position_text: TrackPositionText) -> Self {
        match track_position_text {
            TrackPositionText::NoTrack => TrackPositionSliderValues {
                disabled: true,
                position: 0,
                duration: 100,
            },
            TrackPositionText::Track { position, duration } => TrackPositionSliderValues {
                disabled: false,
                position: position.as_secs(),
                duration: duration.as_secs(),
            },
        }
    }
}

#[allow(non_snake_case)]
#[inline_props]
pub fn View(cx: Scope, player_state: PlayerState) -> Element {
    tracing::debug!(?player_state, "PlayerStateView");

    let commands = use_coroutine_handle::<rradio_messages::Command>(&cx).expect("Commands");

    let track_position = player_state
        .track_position
        .zip(player_state.track_duration)
        .map_or(TrackPositionText::NoTrack, |(position, duration)| {
            TrackPositionText::Track { position, duration }
        });

    let track_position_slider = {
        let TrackPositionSliderValues {
            disabled,
            position,
            duration,
        } = track_position.into();

        rsx! {
            input {
                "type": "range",
                disabled: "{disabled}",
                min: "0",
                max: "{duration}",
                value: "{position}",
                onchange: move |ev| handle_input(|new_position_secs| rradio_messages::Command::SeekTo(Duration::from_secs(new_position_secs)), &ev.value, commands),
            }
        }
    };

    let volume_min = rradio_messages::VOLUME_MIN;
    let volume_max = rradio_messages::VOLUME_ZERO_DB;

    let current_track = player_state
        .current_station
        .as_ref()
        .as_ref()
        .and_then(|station| station.tracks.as_deref())
        .and_then(|tracks| tracks.get(player_state.current_track_index))
        .map(|current_track| rsx! { CurrentTrack { track: current_track.clone(), tags: player_state.current_track_tags.clone() } });

    cx.render(rsx! {
        fieldset {
            id: "current-track-container",
            legend { "Current Track" }
            current_track
        }
        Station { station: player_state.current_station.clone(), current_track_index: player_state.current_track_index }
        track_position_slider
        footer {
            div {
                class: "expand center-single-child",
                div {
                    id: "time",
                    output { "{track_position}" }
                }
            }
            div {
                id: "controls",
                button { onclick: move |_| commands.send(rradio_messages::Command::SmartPreviousItem), "⏪" }
                button { onclick: move |_| commands.send(rradio_messages::Command::PlayPause), "⏯️" }
                button { onclick: move |_| commands.send(rradio_messages::Command::NextItem), "⏩" }
            }
            div {
                class: "expand center-single-child",
                id: "volume",
                "🔉"
                input {
                    "type": "range",
                    min: "{volume_min}",
                    max: "{volume_max}",
                    value: "{player_state.volume}",
                    oninput: move |ev| handle_input(rradio_messages::Command::SetVolume, &ev.value, commands)
                }
                "🔊"
            }
        }
    })
}
