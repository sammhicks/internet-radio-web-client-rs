use std::str::FromStr;

use anyhow::Context;
use dioxus::{logger::tracing::error, prelude::*};

use gloo_storage::Storage;

use crate::{
    track_position_slider::{TrackPositionSlider, TrackPositionText},
    PlayerState,
};

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
                error!("Failed to load {}: {}", Self::STORAGE_KEY, err);
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
            error!("Failed to save podcasts_list: {}", err);
        }
    }
}

#[component]
fn NewPodcastView(
    podcasts: Signal<Vec<Podcast>>,
    selected_podcast_index: Signal<usize>,
) -> Element {
    let mut new_podcast = use_signal(String::new);
    let mut new_podcast_error = use_signal(String::new);

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
    }
}

#[component]
fn SelectPodcastView(
    podcasts: Signal<Vec<Podcast>>,
    selected_podcast_index: Signal<usize>,
) -> Element {
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

    rsx! {
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
    }
}

#[component]
fn FetchedPodcastView(podcast: Option<Podcast>) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>();

    let Some(Podcast { title, url }) = podcast else {
        return rsx! { div { "Index out of range" } };
    };

    let mut is_loaded = use_signal(|| false);

    let podcast = use_resource(use_reactive!(|url| async move {
        is_loaded.set(false);

        let new_podcast = Podcast::fetch(&url).await;

        is_loaded.set(true);

        Some(new_podcast)
    }));

    let podcast = podcast.read();

    match podcast
        .as_ref()
        .and_then(Option::as_ref)
        .filter(|_| is_loaded())
    {
        None => rsx! { div { "Loading {title}..." } },
        Some(Err(err)) => rsx! { div { "{err:#}" } },
        Some(Ok(rss::Channel {
            title,
            description,
            items,
            ..
        })) => {
            let items = items.iter().map(|item| {
                let rss_title = item.title.as_deref().unwrap_or("No Title");
                let description = item
                    .description
                    .as_deref()
                    .map_or_else(VNode::empty, |description| rsx! { p { "{description}" } });

                let link = match &item.enclosure {
                    Some(enclosure) => {
                        let title = title.clone();
                        let track_title = item.title.clone().unwrap_or_else(|| title.clone());
                        let url = enclosure.url.clone();

                        let play_track = move |_| {
                            commands.send(rradio_messages::Command::SetPlaylist {
                                title: title.clone(),
                                tracks: vec![rradio_messages::SetPlaylistTrack {
                                    title: track_title.clone(),
                                    url: url.clone(),
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
                    Fragment {
                        h2 { "{rss_title}" }
                        {link}
                        {description}
                        hr { }
                    }
                }
            });

            rsx! {
                h1 { "{title}" }
                p { em { "{description}" } }
                {items}
            }
        }
    }
}

#[component]
fn RemovePodcastView(
    podcasts: Signal<Vec<Podcast>>,
    selected_podcast_index: Signal<usize>,
) -> Element {
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

    rsx! {
        div {
            button {
                "type": "button",
                onclick: move |_| remove_current_podcast(),
                "Remove Podcast"
            }
        }
    }
}

#[component]
pub fn PodcastsView(player_state: PlayerState) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>();

    let podcasts = use_signal(Podcasts::load);
    let selected_podcast_index = use_signal(|| 0_usize);

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
        NewPodcastView { podcasts, selected_podcast_index }
        SelectPodcastView { podcasts, selected_podcast_index }
        main {
            style: "border-bottom: 1px solid black;",
            if !podcasts.is_empty() {
                FetchedPodcastView { podcast: podcasts.get(selected_podcast_index()).as_deref().cloned() }
            }
            RemovePodcastView { podcasts, selected_podcast_index }
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
