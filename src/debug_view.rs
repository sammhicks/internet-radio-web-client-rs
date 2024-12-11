use dioxus::prelude::*;

use rradio_messages::{CurrentStation, Track, TrackTags};

use crate::ConnectionState;

use super::{FastEqRc, PlayerState};

#[component]
fn CurrentTrackTagsView(current_track_tags: FastEqRc<Option<TrackTags>>) -> Element {
    match current_track_tags.as_ref() {
        Some(TrackTags {
            title,
            organisation,
            artist,
            album,
            genre,
            image,
            comment,
        }) => rsx! {
            dd { "" }
            dd { "Title: {title:?}" }
            dd { "Organisation: {organisation:?}" }
            dd { "Artist: {artist:?}" }
            dd { "Album: {album:?}" }
            dd { "Genre: {genre:?}" }
            dd { "Image: {image:?}" }
            dd { "Comment: {comment:?}" }
        },
        None => rsx! { dd { "None" } },
    }
}

#[component]
fn TrackView(track: Track, is_current: bool) -> Element {
    let Track {
        title,
        album,
        artist,
        url,
        is_notification,
    } = track;

    let class = if is_current { "current-track" } else { "" };

    rsx! {
        dt { class: "{class}", "Track" }
        dd { class: "{class}", "Title: {title:?}" }
        dd { class: "{class}", "Album: {album:?}" }
        dd { class: "{class}", "Artist: {artist:?}" }
        dd { class: "{class}", "Url: {url:?}" }
        dd { class: "{class}", "Is Notification: {is_notification:?}" }
    }
}

#[component]
fn CurrentStationView(
    current_station: FastEqRc<CurrentStation>,
    current_track_index: usize,
) -> Element {
    match current_station.as_ref() {
        CurrentStation::NoStation => rsx! { dd { "None" } },
        CurrentStation::FailedToPlayStation { error } => {
            rsx! { dd { "Failed to play station: {error}" } }
        }
        CurrentStation::PlayingStation {
            index,
            source_type,
            title,
            tracks,
        } => {
            let tracks = tracks
                .as_deref()
                .unwrap_or_default()
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    rsx! { TrackView { key: "{index}", track: track.clone(), is_current: index == current_track_index, } }
                });

            rsx!(
                dd { "Index: {index:?}" }
                dd { "Source Type: {source_type:?}" }
                dd { "Title: {title:?}" }
                dd {
                    "Tracks: "
                    dl {
                        dd {
                            dl { {tracks} }
                        }
                    }
                }
            )
        }
    }
}

#[component]
pub fn DebugView(connection_state: Signal<ConnectionState>, player_state: PlayerState) -> Element {
    if let ConnectionState::Connecting = connection_state() {
        return rsx! {};
    }

    let PlayerState {
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
        latest_error,
    } = player_state;

    rsx! {
        dl {
            dt { "Pipeline State: {pipeline_state:?}" }
            dt { "Pause Before Playing: {pause_before_playing:?}" }
            dt { "Current Track Index: {current_track_index:?}" }
            dt { "Is Muted: {is_muted:?}" }
            dt { "Volume: {volume:?}" }
            dt { "Buffering: {buffering:?}" }
            dt { "Track Duration: {track_duration:?}" }
            dt { "Track Position: {track_position:?}" }
            dt { "Ping Times: {ping_times:?}" }
            dt { "Current Track Tags" }
            CurrentTrackTagsView { current_track_tags }
            dt { "Current Station" }
            CurrentStationView { current_station, current_track_index }
            dt { "Latest Error: {latest_error:?}" }
        }
    }
}
