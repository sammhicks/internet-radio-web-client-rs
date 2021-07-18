use std::{fmt::Display, str::FromStr};

use anyhow::Result;
use rss::Item;
use yew::{
    format::{Json, Nothing},
    html,
    services::{
        fetch::{FetchTask, Request, Response},
        FetchService, StorageService,
    },
    Callback, Component, ComponentLink, Html, InputData, Properties, ShouldRender,
};

use rradio_messages::Command;

const PODCASTS_KEY: &str = "RRADIO_PODCASTS";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Podcast {
    title: String,
    url: String,
}

impl Display for Podcast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.title.fmt(f)
    }
}

#[derive(Clone, Properties)]
pub struct Props {
    pub send_command: Callback<Command>,
}

pub enum Msg {
    NewPodcastUrlChanged(String),
    NewPodcast,
    PodcastSelected(Podcast),
    Podcast {
        url: String,
        response: Response<Result<String>>,
    },
}

pub struct Podcasts {
    props: Props,
    link: ComponentLink<Self>,
    storage: StorageService,
    current_fetch_task: Option<FetchTask>,
    channel: Option<rss::Channel>,
    new_podcast_url: String,
    podcasts: Vec<Podcast>,
    selected_podcast: Option<Podcast>,
}

impl Podcasts {
    fn fetch_podcast(&mut self, url: String) {
        let request = Request::get(&url)
            .body(Nothing)
            .expect("Failed to build request");
        self.current_fetch_task = Some(
            FetchService::fetch(
                request,
                self.link
                    .callback_once(move |response| Msg::Podcast { url, response }),
            )
            .expect("Failed to send request"),
        );
    }

    fn render_item(&self, item: &Item) -> Html {
        let button = if let Some(enclosure) = &item.enclosure {
            let url = enclosure.url.clone();
            let on_click = self
                .props
                .send_command
                .reform(move |_| Command::PlayUrl(url.clone()));
            html! {
                <button onclick=on_click>{"Play"}</button>
            }
        } else {
            html! { <p>{"Nothing to play"}</p> }
        };

        html! {
            <>
                <h2>{item.title.clone().unwrap_or_default()}</h2>
                <p>{item.description.clone().unwrap_or_default()}</p>
                {button}
                <hr/>
            </>
        }
    }

    fn render_items(&self) -> Html {
        if let Some(channel) = &self.channel {
            let items = channel
                .items
                .iter()
                .map(|item| self.render_item(item))
                .collect::<Html>();
            html! {
                <div id="podcast-items">
                    <h1>{channel.title.clone()}</h1>
                    { items }
                </div>
            }
        } else {
            html! {
                <p>{"No Channel"}</p>
            }
        }
    }
}

impl Component for Podcasts {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let storage = StorageService::new(yew::services::storage::Area::Local).unwrap();

        let podcasts = storage
            .restore::<Json<Result<Vec<Podcast>>>>(PODCASTS_KEY)
            .0
            .unwrap_or_else(|err| {
                log::error!("Bad podcasts in storage: {}", err);
                Default::default()
            });

        let first_podcast = podcasts.first().cloned();

        let mut component = Self {
            props,
            link,
            storage,
            current_fetch_task: None,
            channel: None,
            new_podcast_url: String::new(),
            podcasts,
            selected_podcast: None,
        };

        if let Some(podcast) = first_podcast {
            component.fetch_podcast(podcast.url.clone());
            component.selected_podcast = Some(podcast);
        }

        component
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::NewPodcastUrlChanged(new_podcast_url) => {
                self.new_podcast_url = new_podcast_url;
                true
            }
            Msg::NewPodcast => {
                let new_podcast_url = std::mem::take(&mut self.new_podcast_url);
                self.fetch_podcast(new_podcast_url);
                true
            }
            Msg::PodcastSelected(podcast) => {
                self.fetch_podcast(podcast.url);
                true
            }
            Msg::Podcast { url, response } => {
                match response.body() {
                    Ok(body) => match rss::Channel::from_str(&body) {
                        Ok(channel) => {
                            self.selected_podcast = Some(
                                if let Some(podcast) = self
                                    .podcasts
                                    .iter()
                                    .find(|podcast| podcast.url == url)
                                    .cloned()
                                {
                                    podcast
                                } else {
                                    let new_podcast = Podcast {
                                        title: channel.title.clone(),
                                        url,
                                    };
                                    self.podcasts.push(new_podcast.clone());
                                    self.storage.store(PODCASTS_KEY, Json(&self.podcasts));
                                    new_podcast
                                },
                            );
                            self.channel = Some(channel);
                        }
                        Err(err) => log::error!("Bad Podcast: {:?}", err),
                    },
                    Err(err) => log::error!("Failed to fetch podcast: {:#}", err),
                }
                self.current_fetch_task = None;
                true
            } // Msg::PlayUrl(url) => {
              //     self.props
              //         .send_command
              //         .emit(rradio_messages::Command::PlayUrl(url));
              //     false
              // }
              // Msg::PlayPause => self.props.send_command,
              // Msg::Rewind => {
              //     self.props
              //         .send_command
              //         .emit(rradio_messages::Command::SeekBackwards(
              //             std::time::Duration::from_secs(10),
              //         ));
              //     false
              // }
              // Msg::Fastforward => {
              //     self.props
              //         .send_command
              //         .emit(rradio_messages::Command::SeekForwards(
              //             std::time::Duration::from_secs(10),
              //         ));
              //     false
              // }
        }
    }

    fn change(&mut self, props: Self::Properties) -> ShouldRender {
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let new_podcast_url_changed = self
            .link
            .callback(|input: InputData| Msg::NewPodcastUrlChanged(input.value));
        let on_new_url = self.link.callback(|event: yew::FocusEvent| {
            event.prevent_default();
            Msg::NewPodcast
        });

        let on_podcast_changed = self.link.callback(Msg::PodcastSelected);

        let seek_offset = std::time::Duration::from_secs(10);

        let items = self.render_items();
        html! {
            <div id="podcasts">
                <form onsubmit=on_new_url>
                    <label>{"Podcast URL: "}<input type="text" value=self.new_podcast_url.clone() oninput=new_podcast_url_changed/></label>
                    <input type="submit" value="Add Podcast"/>
                </form>
                <label>
                {"Select Podcast:"}
                <yew_components::Select<Podcast> options=self.podcasts.clone() selected=self.selected_podcast.clone() on_change=on_podcast_changed/>
                </label>
                {items}
                <footer id="podcasts-playback-controls">
                    <button onclick=self.props.send_command.reform(move|_|Command::SeekBackwards(seek_offset))>{"⏪"}</button>
                    <button onclick=self.props.send_command.reform(|_|Command::PlayPause)>{"⏯️"}</button>
                    <button onclick=self.props.send_command.reform(move|_|Command::SeekForwards(seek_offset))>{"⏩"}</button>
                </footer>
            </div>
        }
    }
}
