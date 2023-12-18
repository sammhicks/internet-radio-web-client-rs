use std::{fmt, time::Duration};

use dioxus::prelude::*;

use crate::{handle_input, DisplayDuration};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TrackPositionText {
    NoTrack,
    Track {
        position: Duration,
        duration: Duration,
    },
}

impl TrackPositionText {
    pub fn new(&track_position: &Option<Duration>, &track_duration: &Option<Duration>) -> Self {
        track_position
            .zip(track_duration)
            .map_or(TrackPositionText::NoTrack, |(position, duration)| {
                TrackPositionText::Track { position, duration }
            })
    }
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

#[component]
pub fn TrackPositionSlider(cx: Scope, track_position: TrackPositionText) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>(cx).expect("Commands");

    let TrackPositionSliderValues {
        disabled,
        position,
        duration,
    } = TrackPositionSliderValues::from(*track_position);

    cx.render(rsx! {
        input {
            "type": "range",
            disabled: "{disabled}",
            min: "0",
            max: "{duration}",
            value: "{position}",
            onchange: move |ev| handle_input(|new_position_secs| rradio_messages::Command::SeekTo(Duration::from_secs(new_position_secs)), &ev.value, commands),
        }
    })
}
