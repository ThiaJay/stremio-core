use crate::{
    constants::{META_RESOURCE_NAME, STREAM_RESOURCE_NAME},
    models::{
        ctx::Ctx,
        player::{Player, Selected},
    },
    runtime::{
        msg::{Action, ActionLoad, ActionPlayer},
        EnvFutureExt, Runtime, RuntimeAction, TryEnvFuture,
    },
    types::{
        addon::{ResourcePath, ResourceRequest, ResourceResponse},
        library::{LibraryBucket, LibraryItem, LibraryItemState},
        resource::{MetaItem, MetaItemPreview, SeriesInfo, Stream, StreamSource, Video},
    },
    unit_tests::{default_fetch_handler, Request, TestEnv, FETCH_HANDLER},
};
use chrono::{DateTime, Utc};
use futures::future;
use std::any::Any;
use stremio_derive::Model;

#[derive(Model, Default, Clone, Debug)]
#[model(TestEnv)]
struct TestModel {
    ctx: Ctx,
    player: Player,
}

fn video(season: u32, episode: u32) -> Video {
    Video {
        id: format!("tt123456:{season}:{episode}"),
        title: format!("S{season}E{episode}"),
        released: None,
        overview: None,
        thumbnail: None,
        streams: vec![],
        series_info: Some(SeriesInfo { season, episode }),
        trailer_streams: vec![],
    }
}

fn stream() -> Stream {
    Stream {
        source: StreamSource::Url {
            url: "https://source_url".parse().unwrap(),
        },
        name: None,
        description: None,
        thumbnail: None,
        subtitles: vec![],
        behavior_hints: Default::default(),
    }
}

fn item(id: &str, kind: &str, video_id: &str) -> LibraryItem {
    LibraryItem {
        id: id.into(),
        name: "Watch state regression".into(),
        r#type: kind.into(),
        poster: None,
        poster_shape: Default::default(),
        removed: false,
        temp: false,
        ctime: None,
        mtime: DateTime::<Utc>::default(),
        state: LibraryItemState {
            video_id: Some(video_id.into()),
            ..Default::default()
        },
        behavior_hints: Default::default(),
    }
}

fn request(resource: &str, kind: &str, id: &str) -> ResourceRequest {
    ResourceRequest {
        base: "https://transport_url/manifest.json".parse().unwrap(),
        path: ResourcePath {
            resource: resource.into(),
            r#type: kind.into(),
            id: id.into(),
            extra: vec![],
        },
    }
}

fn fetch_handler(request: Request) -> TryEnvFuture<Box<dyn Any + Send>> {
    match request {
        Request { url, .. } if url == "https://transport_url/meta/series/tt123456.json" => {
            future::ok(Box::new(ResourceResponse::Meta {
                meta: MetaItem {
                    preview: MetaItemPreview {
                        id: "tt123456".into(),
                        r#type: "series".into(),
                        name: "Test Series".into(),
                        ..Default::default()
                    },
                    videos: vec![video(1, 1), video(1, 2)],
                },
            }) as Box<dyn Any + Send>)
            .boxed_env()
        }
        Request { url, .. }
            if url == "https://transport_url/stream/series/tt123456%3A1%3A2.json" =>
        {
            future::ok(
                Box::new(ResourceResponse::Streams { streams: vec![] }) as Box<dyn Any + Send>
            )
            .boxed_env()
        }
        Request { url, .. } if url == "https://transport_url/meta/movie/tt654321.json" => {
            future::ok(Box::new(ResourceResponse::Meta {
                meta: MetaItem {
                    preview: MetaItemPreview {
                        id: "tt654321".into(),
                        r#type: "movie".into(),
                        name: "Test Movie".into(),
                        ..Default::default()
                    },
                    videos: vec![],
                },
            }) as Box<dyn Any + Send>)
            .boxed_env()
        }
        _ => default_fetch_handler(request),
    }
}

fn runtime_with(item: LibraryItem) -> (Runtime<TestEnv, TestModel>, impl std::any::Any) {
    let id = item.id.clone();
    Runtime::<TestEnv, _>::new(
        TestModel {
            ctx: Ctx {
                library: LibraryBucket {
                    uid: None,
                    items: vec![(id, item)].into_iter().collect(),
                },
                ..Default::default()
            },
            player: Player::default(),
        },
        vec![],
        1000,
    )
}

fn load(runtime: &Runtime<TestEnv, TestModel>, kind: &str, meta_id: &str, video_id: &str) {
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Load(ActionLoad::Player(Box::new(Selected {
                stream: stream(),
                stream_request: Some(request(STREAM_RESOURCE_NAME, kind, video_id)),
                meta_request: Some(request(META_RESOURCE_NAME, kind, meta_id)),
                subtitles_path: None,
            }))),
        });
    });
}

fn seek(runtime: &Runtime<TestEnv, TestModel>) {
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Player(ActionPlayer::Seek {
                time: 3_500_000,
                duration: 3_600_000,
                device: "test".into(),
            }),
        });
    });
}

fn ended(runtime: &Runtime<TestEnv, TestModel>) {
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Player(ActionPlayer::Ended),
        });
    });
}

fn unload(runtime: &Runtime<TestEnv, TestModel>) {
    TestEnv::run(|| {
        runtime.dispatch(RuntimeAction {
            field: None,
            action: Action::Unload,
        });
    });
}

#[test]
fn seek_to_end_then_ended_marks_series_episode_watched_before_resume_cleanup() {
    let _env_mutex = TestEnv::reset().expect("exclusive TestEnv");
    *FETCH_HANDLER.write().unwrap() = Box::new(fetch_handler);
    let (runtime, _rx) = runtime_with(item("tt123456", "series", "tt123456:1:1"));

    load(&runtime, "series", "tt123456", "tt123456:1:1");
    seek(&runtime);

    {
        let model = runtime.model().unwrap();
        let item = model.player.library_item.as_ref().unwrap();
        assert_eq!(item.state.times_watched, 0);
        assert_eq!(item.state.flagged_watched, 0);
    }

    ended(&runtime);

    {
        let model = runtime.model().unwrap();
        let item = model.player.library_item.as_ref().unwrap();
        let videos = vec![video(1, 1), video(1, 2)];
        let watched = item.state.watched_bitfield(&videos);
        assert!(watched.get_video("tt123456:1:1"));
        assert_eq!(item.state.times_watched, 1);
        assert_eq!(item.state.flagged_watched, 1);
    }

    unload(&runtime);

    let model = runtime.model().unwrap();
    let item = model.ctx.library.items.get("tt123456").unwrap();
    let videos = vec![video(1, 1), video(1, 2)];
    assert!(item
        .state
        .watched_bitfield(&videos)
        .get_video("tt123456:1:1"));
    assert_eq!(item.state.video_id.as_deref(), Some("tt123456:1:2"));
    assert_eq!(item.state.time_offset, 1);
}

#[test]
fn seek_to_end_then_ended_marks_movie_watched_before_resume_cleanup() {
    let _env_mutex = TestEnv::reset().expect("exclusive TestEnv");
    *FETCH_HANDLER.write().unwrap() = Box::new(fetch_handler);
    let (runtime, _rx) = runtime_with(item("tt654321", "movie", "tt654321"));

    load(&runtime, "movie", "tt654321", "tt654321");
    seek(&runtime);

    {
        let model = runtime.model().unwrap();
        let item = model.player.library_item.as_ref().unwrap();
        assert_eq!(item.state.times_watched, 0);
        assert_eq!(item.state.flagged_watched, 0);
    }

    ended(&runtime);

    {
        let model = runtime.model().unwrap();
        let item = model.player.library_item.as_ref().unwrap();
        assert_eq!(item.state.times_watched, 1);
        assert_eq!(item.state.flagged_watched, 1);
        assert!(item.state.last_watched.is_some());
    }

    unload(&runtime);

    let model = runtime.model().unwrap();
    let item = model.ctx.library.items.get("tt654321").unwrap();
    assert_eq!(item.state.time_offset, 0);
    assert_eq!(item.state.times_watched, 1);
    assert_eq!(item.state.flagged_watched, 1);
    assert!(!item.is_in_continue_watching());
}

#[test]
fn seek_to_end_without_ended_does_not_mark_watched() {
    let _env_mutex = TestEnv::reset().expect("exclusive TestEnv");
    *FETCH_HANDLER.write().unwrap() = Box::new(fetch_handler);
    let (runtime, _rx) = runtime_with(item("tt654321", "movie", "tt654321"));

    load(&runtime, "movie", "tt654321", "tt654321");
    seek(&runtime);

    let model = runtime.model().unwrap();
    let item = model.player.library_item.as_ref().unwrap();
    assert_eq!(item.state.times_watched, 0);
    assert_eq!(item.state.flagged_watched, 0);
}
