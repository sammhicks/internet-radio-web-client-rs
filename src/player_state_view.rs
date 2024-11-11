use dioxus::prelude::*;

use crate::{
    handle_input,
    track_position_slider::{TrackPositionSlider, TrackPositionText},
    FastEqRc, PlayerState,
};

#[component]
fn CurrentTrackView(
    track: rradio_messages::Track,
    tags: FastEqRc<Option<rradio_messages::TrackTags>>,
) -> Element {
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

    rsx! {
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
    }
}

#[component]
fn PlaylistTrackView(
    track_index: usize,
    track: rradio_messages::Track,
    is_current_track: bool,
) -> Element {
    tracing::debug!(?track_index, ?track, ?is_current_track, "PlaylistTrack");

    let commands = use_coroutine_handle::<rradio_messages::Command>();

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

    let class_name = if is_current_track {
        "current-track"
    } else {
        ""
    };

    let track_index = track_index;

    rsx! {
        div {
            class: "{class_name}",
            onclick: move |_| commands.send(rradio_messages::Command::NthItem(track_index)),
            {contents}
        }
    }
}

#[component]
fn StationView(
    current_station: FastEqRc<rradio_messages::CurrentStation>,
    current_track_index: usize,
) -> Element {
    tracing::debug!(?current_station, ?current_track_index, "Station");

    match current_station.as_ref() {
        rradio_messages::CurrentStation::NoStation => {
            return rsx! {
                fieldset {
                    id: "current-station",
                    legend { "No Station" }
                }
            }
        }
        rradio_messages::CurrentStation::FailedToPlayStation { error } => {
            return rsx! {
                fieldset {
                    id: "current-station",
                    legend { "Failed to Play Station" }
                    "{error}"
                }
            }
        }
        rradio_messages::CurrentStation::PlayingStation {
            index,
            source_type,
            title,
            tracks,
        } => {
            let legend = match index {
                Some(index) => rsx! { "Station {index}" },
                None => rsx! { "Station" },
            };

            let title = match &title {
                Some(title) => rsx! { "{title}" },
                None => rsx! { "{source_type}" },
            };

            let tracks = tracks.as_deref()
                .unwrap_or_default()
                .iter()
                .cloned()
                .enumerate()
                .map(|(track_index, track)| {
                    let is_current_track = track_index == current_track_index;
                    rsx! { PlaylistTrackView { key: "Track{track_index}", track_index, track, is_current_track } }
                });

            rsx! {
                fieldset {
                    id: "current-station",
                    legend { {legend} }
                    div { id: "current-station-title", {title} }
                    {tracks}
                }
            }
        }
    }
}

#[component]
pub fn view(player_state: PlayerState) -> Element {
    tracing::debug!(?player_state, "PlayerStateView");

    let commands = use_coroutine_handle::<rradio_messages::Command>();

    let volume_min = rradio_messages::VOLUME_MIN;
    let volume_max = rradio_messages::VOLUME_ZERO_DB;

    let current_track = match player_state.current_station.as_ref() {
        rradio_messages::CurrentStation::PlayingStation { tracks: Some(tracks), .. } => {
            tracks.get(player_state.current_track_index).map(|current_track| rsx! { CurrentTrackView { track: current_track.clone(), tags: player_state.current_track_tags.clone() } })
        },
        _ => None,
    };

    let track_position =
        TrackPositionText::new(&player_state.track_position, &player_state.track_duration);

    rsx! {
        fieldset {
            id: "current-track-container",
            legend { "Current Track" }
            {current_track}
        }
        StationView { current_station: player_state.current_station.clone(), current_track_index: player_state.current_track_index }
        TrackPositionSlider { track_position }
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
                    oninput: move |ev| handle_input(rradio_messages::Command::SetVolume, &ev.value(), &commands)
                }
                "🔊"
            }
        }
    }
}
