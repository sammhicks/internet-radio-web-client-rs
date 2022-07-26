use std::str::FromStr;

use anyhow::Context;
use dioxus::prelude::*;

use gloo_storage::Storage;
use rradio_messages::ArcStr;

use super::FastEqRc;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Podcast {
    title: ArcStr,
    url: ArcStr,
}

impl Podcast {
    const STORAGE_KEY: &'static str = "RRADIO_PODCASTS";

    fn load_podcasts() -> Vec<Podcast> {
        match gloo_storage::LocalStorage::get(Self::STORAGE_KEY) {
            Ok(podcasts) => podcasts,
            Err(gloo_storage::errors::StorageError::KeyNotFound(_)) => Vec::new(),
            Err(err) => {
                tracing::error!("Failed to load {}: {}", Self::STORAGE_KEY, err);
                Vec::new()
            }
        }
    }

    fn save_podcasts(podcasts: &[Podcast]) -> Result<(), ()> {
        gloo_storage::LocalStorage::set(Self::STORAGE_KEY, podcasts).map_err(|err| {
            tracing::error!("Failed to save podcasts_list: {}", err);
        })
    }

    async fn fetch(url: &str) -> anyhow::Result<rss::Channel> {
        let response = gloo_net::http::Request::get(url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch {}", url))?;

        if response.status() != 200 {
            anyhow::bail!(
                "Failed to fetch {}: Error {}: {}",
                url,
                response.status(),
                response.status_text()
            );
        }

        rss::Channel::from_str(&response.text().await?)
            .with_context(|| format!("Failed to parse RSS from {:?}", url))
    }
}

#[allow(non_snake_case)]
#[inline_props]
fn FetchedPodcastItem<'a>(cx: Scope<'a>, playlist_title: &'a str, item: &'a rss::Item) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>(&cx).expect("Commands");

    let title = item.title().unwrap_or("No Title");
    let description = item
        .description()
        .map(|description| rsx! { p { "{description}" } });

    let link = match item.enclosure() {
        Some(enclosure) => {
            let play_track = move |_: dioxus::core::UiEvent<dioxus::events::MouseData>| {
                commands.send(rradio_messages::Command::SetPlaylist {
                    title: String::from(*playlist_title),
                    tracks: vec![rradio_messages::SetPlaylistTrack {
                        title: String::from(item.title().unwrap_or(playlist_title)),
                        url: enclosure.url.clone(),
                    }],
                })
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

    cx.render(rsx! {
        h2 { "{title}" }
        link
        description
        hr { }
    })
}

#[allow(non_snake_case)]
#[inline_props]
fn FetchedPodcast(cx: Scope, fetched_podcast: FastEqRc<rss::Channel>) -> Element {
    let title = fetched_podcast.title();
    let description = fetched_podcast.description();

    let items = fetched_podcast
        .as_ref()
        .items()
        .iter()
        .enumerate()
        .map(|(index, item)| rsx! { FetchedPodcastItem { key: "{index}", playlist_title: title, item: item } });

    cx.render(rsx! {
        h1 { "{title}" }
        p { em { "{description}" } }
        items
    })
}

#[allow(non_snake_case)]
#[inline_props]
pub fn View(cx: Scope) -> Element {
    let commands = use_coroutine_handle::<rradio_messages::Command>(&cx).expect("Commands");

    let (new_podcast, new_podcast_store) = use_state(&cx, String::new).split();
    let (new_podcast_error, new_podcast_error_store) = use_state(&cx, String::new).split();

    let (podcasts, podcasts_store) = use_state(&cx, Podcast::load_podcasts).split();
    let (&selected_podcast_index, selected_podcast_index_store) =
        use_state(&cx, || 0_usize).split();
    let selected_podcast = podcasts.get(selected_podcast_index);

    let fetched_podcast = use_future(&cx, (podcasts_store, selected_podcast_index_store), {
        move |(podcasts_store, selected_podcast_index_store)| async move {
            let podcasts = podcasts_store.current();

            if podcasts.is_empty() {
                return Ok(None);
            }

            let podcast = podcasts
                .get(*selected_podcast_index_store.current())
                .context("Podcast index out of range")?;

            Podcast::fetch(&podcast.url)
                .await
                .map(|channel| Some(FastEqRc::new(channel)))
        }
    })
    .value();

    let fetched_podcast = match fetched_podcast {
        None => rsx! { "Loading..." },
        Some(Err(err)) => rsx! { "{err:#}" },
        Some(Ok(None)) => rsx! { "" },
        Some(Ok(Some(fetched_podcast))) => {
            rsx! { FetchedPodcast { fetched_podcast: fetched_podcast.clone() } }
        }
    };

    let podcast_options = podcasts.iter().enumerate().map(|(index, option)| {
        let is_selected = selected_podcast_index == index;
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
        let disabled = if selected_podcast.is_none() {
            "disabled"
        } else {
            ""
        };
        let selected = if selected_podcast.is_none() {
            "selected"
        } else {
            ""
        };
        rsx! {
            option {
                key: "none-selected",
                disabled: "{disabled}",
                selected: "{selected}",

            }
        }
    })
    .chain(podcast_options);

    let add_podcast = {
        move |_| {
            let new_podcast_store = new_podcast_store.clone();
            let new_podcast_error_store = new_podcast_error_store.clone();
            let podcasts_store = podcasts_store.clone();
            let selected_podcast_index_store = selected_podcast_index_store.clone();
            cx.spawn(async move {
                let url = new_podcast_store.current();

                match Podcast::fetch(url.as_str()).await {
                    Ok(podcast) => {
                        let mut current_podcasts = podcasts_store.current().as_ref().clone();
                        let new_selected_podcast_index = current_podcasts.len();
                        current_podcasts.push(Podcast {
                            title: podcast.title.into(),
                            url: url.as_str().into(),
                        });

                        if let Ok(()) = Podcast::save_podcasts(&current_podcasts) {
                            new_podcast_store.set(String::new());
                            podcasts_store.set(current_podcasts);
                            selected_podcast_index_store.set(new_selected_podcast_index);
                        }
                    }
                    Err(err) => {
                        new_podcast_error_store.set(format!("{err:#}"));
                    }
                }
            })
        }
    };

    let seek_offset = std::time::Duration::from_secs(10);

    cx.render(rsx! {
        div {
            id: "new-podcast",
            label {
                "Podcast URL: "
                input {
                    "type": "text",
                    value: "{new_podcast}",
                    oninput: move |ev| new_podcast_store.set(ev.value.clone()),
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
                    onchange: move |ev| selected_podcast_index_store.set(ev.value.parse().unwrap()),
                    podcast_options
                }
            }
        }
        main {
            fetched_podcast
        }
        footer {
            style: "border-top: 1px solid black;",
            button { onclick: move |_| commands.send(rradio_messages::Command::SeekBackwards(seek_offset)), "⏪" }
            button { onclick: move |_| commands.send(rradio_messages::Command::PlayPause), "⏯️" }
            button { onclick: move |_| commands.send(rradio_messages::Command::SeekForwards(seek_offset)), "⏩" }
        }
    })
}
