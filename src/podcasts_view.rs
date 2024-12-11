use std::str::FromStr;

use anyhow::Context;
use dioxus::prelude::*;

use gloo_storage::Storage;

use crate::{
    track_position_slider::{TrackPositionSlider, TrackPositionText},
    PlayerState,
};

use super::FastEqRc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Podcast {
    title: String,
    url: String,
}

impl Podcast {
    async fn fetch(url: &str) -> anyhow::Result<rss::Channel> {
        let response = gloo_net::http::Request::get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch {url}"))?;

        if response.status() != 200 {
            anyhow::bail!(
                "Failed to fetch {}: Error {}: {}",
                url,
                response.status(),
                response.status_text()
            );
        }

        rss::Channel::from_str(&response.text().await?)
            .with_context(|| format!("Failed to parse RSS from {url:?}"))
    }
}

struct Podcasts;

impl Podcasts {
    const STORAGE_KEY: &'static str = "RRADIO_PODCASTS";

    fn load() -> Vec<Podcast> {
        match gloo_storage::LocalStorage::get(Self::STORAGE_KEY) {
            Ok(podcasts) => podcasts,
            Err(gloo_storage::errors::StorageError::KeyNotFound(_)) => Vec::new(),
            Err(err) => {
                tracing::error!("Failed to load {}: {}", Self::STORAGE_KEY, err);
                Vec::new()
            }
        }
    }
}

trait SavePodcastsExt {
    fn save(&self);
}

impl SavePodcastsExt for [Podcast] {
    fn save(&self) {
        if let Err(err) = gloo_storage::LocalStorage::set(Podcasts::STORAGE_KEY, self) {
            tracing::error!("Failed to save podcasts_list: {}", err);
        }
    }
}

#[component]
fn FetchedPodcastItemView(playlist_title: String, item: rss::Item) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>();

    let rss_title = item.title.as_deref().unwrap_or("No Title");
    let description = item
        .description
        .map(|description| rsx! { p { "{description}" } });

    let link = match item.enclosure {
        Some(enclosure) => {
            let track_title = item.title.clone().unwrap_or_else(|| playlist_title.clone());

            let play_track = move |_| {
                commands.send(rradio_messages::Command::SetPlaylist {
                    title: playlist_title.clone(),
                    tracks: vec![rradio_messages::SetPlaylistTrack {
                        title: track_title.clone(),
                        url: enclosure.url.clone(),
                    }],
                });
            };
            rsx! {
                div {
                    button {
                        "type": "button",
                        onclick: play_track,
                        "Stream"
                    }
                }
            }
        }
        None => rsx! { "Nothing to Stream!" },
    };

    rsx! {
        h2 { "{rss_title}" }
        {link}
        {description}
        hr { }
    }
}

#[component]
fn FetchedPodcastView(fetched_podcast: FastEqRc<rss::Channel>) -> Element {
    let playlist_title = fetched_podcast.title();
    let description = fetched_podcast.description();

    let items = fetched_podcast
        .as_ref()
        .items()
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, item)| rsx! { FetchedPodcastItemView { key: "{index}", playlist_title, item } });

    rsx! {
        h1 { "{playlist_title}" }
        p { em { "{description}" } }
        {items}
    }
}

#[component]
pub fn PodcastsView(player_state: PlayerState) -> Element {
    enum FetchedPodcast {
        NoPodcasts,
        FetchingPodcast { title: String },
        Podcast(FastEqRc<rss::Channel>),
        Error(anyhow::Error),
    }

    let commands = use_coroutine_handle::<rradio_messages::Command>();

    let mut new_podcast = use_signal(String::new);
    let mut new_podcast_error = use_signal(String::new);

    let mut podcasts = use_signal(Podcasts::load);
    let mut selected_podcast_index = use_signal(|| 0_usize);

    let mut fetched_podcast = use_signal(|| FetchedPodcast::NoPodcasts);

    let _ = use_resource(move || async move {
        if podcasts.is_empty() {
            fetched_podcast.set(FetchedPodcast::NoPodcasts);
            return;
        }

        let Some(podcast) = podcasts.get(selected_podcast_index()) else {
            fetched_podcast.set(FetchedPodcast::Error(anyhow::Error::msg(
                "Podcast index out of range",
            )));
            return;
        };

        fetched_podcast.set(FetchedPodcast::FetchingPodcast {
            title: podcast.title.clone(),
        });

        fetched_podcast.set(
            Podcast::fetch(&podcast.url)
                .await
                .map_or_else(FetchedPodcast::Error, |podcast| {
                    FetchedPodcast::Podcast(FastEqRc::new(podcast))
                }),
        );
    });

    let fetched_podcast = match &*fetched_podcast.read() {
        FetchedPodcast::NoPodcasts => rsx! {},
        FetchedPodcast::FetchingPodcast { title } => rsx! { div { "Loading {title}..." } },
        FetchedPodcast::Podcast(fetched_podcast) => {
            rsx! { FetchedPodcastView { fetched_podcast: fetched_podcast.clone() } }
        }
        FetchedPodcast::Error(err) => rsx! { div { "{err:#}" } },
    };

    let mut remove_current_podcast = move || {
        let mut podcasts = podcasts.write();

        if podcasts
            .get(selected_podcast_index())
            .is_some_and(|selected_podcast| {
                gloo_dialogs::confirm(&format!(
                    "Are you sure you want to remove {}?",
                    selected_podcast.title
                ))
            })
        {
            podcasts.remove(selected_podcast_index());
            podcasts.save();
            selected_podcast_index.set(0);
        }
    };

    let remove_podcast = rsx! {
        div {
            button {
                "type": "button",
                onclick: move |_| remove_current_podcast(),
                "Remove Podcast"
            }
        }
    };

    let podcast_options = podcasts.iter().enumerate().map(|(index, option)| {
        let is_selected = selected_podcast_index() == index;
        rsx! {
            option {
                key: "{index}",
                selected: "{is_selected}",
                value: "{index}",
                "{option.title}"
            }
        }
    });

    let podcast_options = std::iter::once({
        let key = "none-selected";
        let disabled = if podcasts.get(selected_podcast_index()).is_none() {
            "disabled"
        } else {
            ""
        };
        let selected = if podcasts.get(selected_podcast_index()).is_none() {
            "selected"
        } else {
            ""
        };
        rsx! {
            option {
                key: "{key}",
                disabled: "{disabled}",
                selected: "{selected}",

            }
        }
    })
    .chain(podcast_options);

    let add_podcast = {
        move |_| {
            spawn(async move {
                let url = new_podcast.take();

                match Podcast::fetch(&url).await {
                    Ok(podcast) => {
                        let mut current_podcasts = podcasts.write();
                        current_podcasts.push(Podcast {
                            title: podcast.title,
                            url: url.clone(),
                        });

                        current_podcasts.sort_by(|a, b| {
                            use std::cmp::Ordering;

                            let mut a = a.title.chars().flat_map(char::to_lowercase);
                            let mut b = b.title.chars().flat_map(char::to_lowercase);

                            loop {
                                return match (a.next(), b.next()) {
                                    (None, None) => Ordering::Equal,
                                    (Some(_), None) => Ordering::Greater,
                                    (None, Some(_)) => Ordering::Less,
                                    (Some(a), Some(b)) if a == b => continue,
                                    (Some(a), Some(b)) => a.cmp(&b),
                                };
                            }
                        });

                        selected_podcast_index.set(
                            current_podcasts
                                .iter()
                                .enumerate()
                                .find_map(|(index, podcast)| {
                                    (podcast.url.as_str() == url.as_str()).then_some(index)
                                })
                                .unwrap_or_default(),
                        );

                        current_podcasts.save();
                    }
                    Err(err) => {
                        new_podcast_error.set(format!("{err:#}"));
                    }
                }
            });
        }
    };

    let track_title = player_state
        .current_track_tags
        .as_ref()
        .as_ref()
        .and_then(|tags| tags.title.clone())
        .or_else(|| match player_state.current_station.as_ref() {
            rradio_messages::CurrentStation::PlayingStation {
                tracks: Some(tracks),
                ..
            } => tracks
                .get(player_state.current_track_index)
                .and_then(|track| track.title.clone()),
            _ => None,
        })
        .unwrap_or_default();

    let track_position =
        TrackPositionText::new(&player_state.track_position, &player_state.track_duration);

    let seek_offset = std::time::Duration::from_secs(10);

    rsx! {
        div {
            id: "new-podcast",
            label {
                "Podcast URL: "
                input {
                    "type": "text",
                    value: "{new_podcast}",
                    oninput: move |ev| new_podcast.set(ev.value()),
                }
            }
            button {
                "type": "button",
                onclick: add_podcast,
                "Add Podcast"
            }
            output {
                "{new_podcast_error}"
            }
        }
        div {
            id: "select-podcast",
            style: "border-bottom: 1px solid black;",
            label {
                "Select Podcast: "
                select {
                    oninput: move |ev| selected_podcast_index.set(ev.value().parse().unwrap_or_default()),
                    {podcast_options}
                }
            }
        }
        main {
            style: "border-bottom: 1px solid black;",
            {fetched_podcast}
            {remove_podcast}
        }
        TrackPositionSlider { track_position }
        div {
            style: "text-align: center;",
            "{track_title}"
        }
        footer {
            button { onclick: move |_| commands.send(rradio_messages::Command::SeekBackwards(seek_offset)), "⏪" }
            button { onclick: move |_| commands.send(rradio_messages::Command::PlayPause), "⏯️" }
            button { onclick: move |_| commands.send(rradio_messages::Command::SeekForwards(seek_offset)), "⏩" }
        }
    }
}
