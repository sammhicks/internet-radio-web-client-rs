use std::borrow::Cow;

use anyhow::Context;
use yew::{html, Callback, Component, ComponentLink, Html, Properties, ShouldRender};

use rradio_messages::{arcstr, ArcStr, Command, Station, Track, TrackTags};

use crate::{CreatesOptionalCallback, DurationDisplay};

#[derive(Clone, Properties)]
pub struct Props {
    pub player_state: super::PlayerState,
    pub send_command: Callback<rradio_messages::Command>,
}

pub enum Msg {
    SendCommand(rradio_messages::Command),
}

fn show_track(track: &Track) -> ArcStr {
    if track.is_notification {
        arcstr::literal!("<Notification>")
    } else {
        match &track.title {
            Some(title) => arcstr::format!("{}: {}", title, track.url),
            None => track.url.clone(),
        }
    }
}

fn show_station(station: &Station, current_track_index: usize) -> Html {
    let station_description = match (&station.index, &station.title) {
        (Some(index), Some(title)) => format!("{} - {}", index, title),
        (Some(index), None) => index.to_string(),
        (None, Some(title)) => title.to_string(),
        (None, None) => String::from("Unknown"),
    };

    let tracks = station
        .tracks
        .iter()
        .enumerate()
        .map(|(track_index, track)| {
            let li_class = if track_index == current_track_index {
                Some("selected")
            } else {
                None
            };
            html!(<li class=li_class>{show_track(track)}</li>)
        })
        .collect::<Html>();

    html! {
        <>
            <label>{"Current Station:"}<output>{station_description}</output></label>
            <label>{"Tracks"}<ol>{tracks}</ol></label>
        </>
    }
}

fn show_track_tag(name: &'static str, tag: &Option<ArcStr>) -> Html {
    tag.as_ref().map_or_else(
        || html!(),
        |tag| html!(<li><label>{name} {":"} <output>{tag}</output></label></li>),
    )
}

fn show_track_tags(track_tags: &TrackTags) -> Html {
    let image = match &track_tags.image {
        Some(url) => {
            html!(<li><label>{"Image:"}<img class="album-art" src=url.to_string() /></label></li>)
        }
        None => html!(),
    };

    html! {
        <label>{"Track Tags:"}
            <ul>
                {show_track_tag("Title", &track_tags.title)}
                {show_track_tag("Organisation", &track_tags.organisation)}
                {show_track_tag("Artist", &track_tags.artist)}
                {show_track_tag("Album", &track_tags.album)}
                {show_track_tag("Genre", &track_tags.genre)}
                {image}
                {show_track_tag("Comment", &track_tags.comment)}
            </ul>
        </label>
    }
}

pub struct Player {
    props: Props,
    link: ComponentLink<Self>,
}

impl Component for Player {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self { props, link }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SendCommand(command) => self.props.send_command.emit(command),
        }
        false
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let station = self
            .props
            .player_state
            .current_station
            .as_ref()
            .map_or_else(
                || html!(<label>{"No Station"}</label>),
                |station| show_station(station, self.props.player_state.current_track_index),
            );

        let track_tags = self
            .props
            .player_state
            .current_track_tags
            .as_ref()
            .map_or_else(|| html!(<label>{"No Track"}</label>), show_track_tags);

        let on_volume_change = self.link.result_callback(|data: yew::InputData| {
            Ok(Msg::SendCommand(Command::SetVolume(
                data.value.parse().context("Invalid volume")?,
            )))
        });

        let position = if let Some((position, duration)) = self
            .props
            .player_state
            .track_position
            .zip(self.props.player_state.track_duration)
        {
            Cow::Owned(format!(
                "{} - {}",
                DurationDisplay(position),
                DurationDisplay(duration)
            ))
        } else {
            Cow::Borrowed("Infinite Stream")
        };

        html! {
            <>
                <label>{"Pipeline State:"}<output>{self.props.player_state.pipeline_state}</output></label>
                {station}
                {track_tags}
                <label>{"Volume:"}<input type="range" min=rradio_messages::VOLUME_MIN.to_string() max=rradio_messages::VOLUME_MAX.to_string() value={self.props.player_state.volume.to_string()} oninput=on_volume_change/></label>
                <div>
                    <button onclick=self.link.callback(|_| Msg::SendCommand(Command::PreviousItem))>{"⏮"}</button>
                    <button onclick=self.link.callback(|_| Msg::SendCommand(Command::PlayPause))>{"⏯"}</button>
                    <button onclick=self.link.callback(|_| Msg::SendCommand(Command::NextItem))>{"⏭"}</button>
                </div>
                <label>{"Position:"}<output>{position}</output></label>
                <label>{"Buffering:"}<progress value=self.props.player_state.buffering.to_string() min=0 max=100> {self.props.player_state.buffering}{"%"} </progress></label>
            </>
        }
    }
}
