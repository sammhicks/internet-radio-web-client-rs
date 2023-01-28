use dioxus::prelude::*;

use rradio_messages::Track;

use crate::{
    handle_input,
    track_position_slider::{TrackPositionSlider, TrackPositionText},
    FastEqRc, PlayerState,
};

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

#[allow(non_snake_case)]
#[inline_props]
pub fn View(cx: Scope, player_state: PlayerState) -> Element {
    tracing::debug!(?player_state, "PlayerStateView");

    let commands = use_coroutine_handle::<rradio_messages::Command>(&cx).expect("Commands");

    let volume_min = rradio_messages::VOLUME_MIN;
    let volume_max = rradio_messages::VOLUME_ZERO_DB;

    let current_track = player_state
        .current_station
        .as_ref()
        .as_ref()
        .and_then(|station| station.tracks.as_deref())
        .and_then(|tracks| tracks.get(player_state.current_track_index))
        .map(|current_track| rsx! { CurrentTrack { track: current_track.clone(), tags: player_state.current_track_tags.clone() } });

    let track_position =
        TrackPositionText::new(&player_state.track_position, &player_state.track_duration);

    cx.render(rsx! {
        fieldset {
            id: "current-track-container",
            legend { "Current Track" }
            current_track
        }
        Station { station: player_state.current_station.clone(), current_track_index: player_state.current_track_index }
        TrackPositionSlider { track_position: track_position }
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
