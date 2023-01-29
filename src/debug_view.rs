use dioxus::prelude::*;

use rradio_messages::{Station, Track, TrackTags};

use crate::ConnectionState;

use super::{FastEqRc, PlayerState};

#[allow(non_snake_case)]
#[inline_props]
fn CurrentTrackTagsView(cx: Scope, current_track_tags: FastEqRc<Option<TrackTags>>) -> Element {
    cx.render(match current_track_tags.as_ref() {
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
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn TrackView<'a>(cx: Scope<'a>, track: &'a Track, is_current: bool) -> Element<'a> {
    let Track {
        title,
        album,
        artist,
        url,
        is_notification,
    } = track;

    let class = if *is_current { "current-track" } else { "" };

    cx.render(rsx! {
        dt { class: "{class}", "Track" }
        dd { class: "{class}", "Title: {title:?}" }
        dd { class: "{class}", "Album: {album:?}" }
        dd { class: "{class}", "Artist: {artist:?}" }
        dd { class: "{class}", "Url: {url:?}" }
        dd { class: "{class}", "Is Notification: {is_notification:?}" }
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn CurrentStationView(
    cx: Scope,
    current_station: FastEqRc<Option<Station>>,
    current_track_index: usize,
) -> Element {
    cx.render(match current_station.as_ref() {
        Some(Station {
            index,
            source_type,
            title,
            tracks,
        }) => {
            let tracks = match tracks {
                Some(tracks) => {
                    let tracks = tracks.iter().enumerate().map(|(index, track)| {
                        let is_current = index == *current_track_index;
                        rsx! { TrackView { key: "{index}", track: track, is_current: is_current, } }
                    });

                    rsx! {
                        "Some ("
                        dl {
                            dd {
                                dl { tracks}
                            }
                        }
                        ")"
                    }
                }
                None => rsx! { "None" },
            };

            rsx! {
                dd { "Index: {index:?}" }
                dd { "Source Type: {source_type:?}" }
                dd { "Title: {title:?}" }
                dd { "Tracks: " tracks }
            }
        }
        None => rsx! { dd { "None" } },
    })
}

#[allow(non_snake_case)]
#[inline_props]
pub fn View(cx: Scope, connection_state: ConnectionState, player_state: PlayerState) -> Element {
    if let ConnectionState::Connecting = connection_state {
        return None;
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
    } = player_state;

    cx.render(rsx! {
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
            CurrentTrackTagsView { current_track_tags: current_track_tags.clone() }
            dt { "Current Station" }
            CurrentStationView { current_station: current_station.clone(), current_track_index: *current_track_index }
        }
    })
}
