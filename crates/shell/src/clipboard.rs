use std::sync::Arc;
use std::time::Instant;

use iced::Subscription;
use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};

#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    Text(String),
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Png(Arc<Vec<u8>>),
}

#[derive(Clone, Debug)]
pub struct Clip {
    pub content: Content,
    pub picture: Option<iced::widget::image::Handle>,
    pub when: Instant,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Link,
    Color,
    Image,
}

impl Clip {
    pub fn new(content: Content) -> Self {
        let picture = match &content {
            Content::Png(bytes) => Some(iced::widget::image::Handle::from_bytes(
                bytes.as_ref().clone(),
            )),
            Content::Text(_) => None,
        };
        Self {
            content,
            picture,
            when: Instant::now(),
            pinned: false,
        }
    }

    pub fn kind(&self) -> Kind {
        match &self.content {
            Content::Png(_) => Kind::Image,
            Content::Text(text) => {
                let text = text.trim();
                if text.starts_with("https://") || text.starts_with("http://") {
                    Kind::Link
                } else if apolline_theme::Rgba::parse(text).is_some() {
                    Kind::Color
                } else {
                    Kind::Text
                }
            }
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.content {
            Content::Text(text) => Some(text),
            Content::Png(_) => None,
        }
    }

    pub fn line(&self) -> String {
        match &self.content {
            Content::Text(text) => text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_owned(),
            Content::Png(bytes) => match png_size(bytes) {
                Some((width, height)) => format!("Image {width}×{height}"),
                None => "Image".to_owned(),
            },
        }
    }
}

pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let header = bytes.get(16..24)?;
    let width = u32::from_be_bytes(header[..4].try_into().ok()?);
    let height = u32::from_be_bytes(header[4..].try_into().ok()?);
    (bytes.starts_with(b"\x89PNG")).then_some((width, height))
}

pub fn remember(clips: &mut Vec<Clip>, clip: Clip) {
    let pinned = clips
        .iter()
        .find(|c| c.content == clip.content)
        .is_some_and(|c| c.pinned);
    clips.retain(|c| c.content != clip.content);
    clips.insert(0, Clip { pinned, ..clip });
    let mut unpinned = 0;
    clips.retain(|c| {
        unpinned += usize::from(!c.pinned);
        c.pinned || unpinned <= 100
    });
}

pub fn subscription() -> Subscription<Clip> {
    Subscription::run(watch)
}

fn watch() -> impl Stream<Item = Clip> {
    iced::stream::channel(8, async |mut output| {
        let (send, mut clips) = mpsc::unbounded();
        std::thread::spawn(move || {
            if let Err(err) = wayland::watch(send) {
                eprintln!("apolline: clipboard: {err}");
            }
        });
        while let Some(clip) = clips.next().await {
            if output.send(clip).await.is_err() {
                return;
            }
        }
    })
}

pub fn copy(content: &Content) {
    wayland::copy(content);
}

#[cfg(target_os = "linux")]
mod wayland {
    use std::io::{Read, Write};
    use std::os::fd::AsFd;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::Duration;

    use iced::futures::channel::mpsc::UnboundedSender;
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_registry, wl_seat};
    use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
    use wayland_protocols::ext::data_control::v1::client::ext_data_control_device_v1::{
        self, ExtDataControlDeviceV1,
    };
    use wayland_protocols::ext::data_control::v1::client::ext_data_control_manager_v1::{
        self, ExtDataControlManagerV1,
    };
    use wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::{
        self, ExtDataControlOfferV1,
    };
    use wayland_protocols::ext::data_control::v1::client::ext_data_control_source_v1::{
        self, ExtDataControlSourceV1,
    };

    use super::{Clip, Content};

    const TEXT: [&str; 5] = [
        "text/plain;charset=utf-8",
        "text/plain",
        "UTF8_STRING",
        "STRING",
        "TEXT",
    ];
    const PNG: &str = "image/png";
    const SECRET: &str = "x-kde-passwordManagerHint";
    const LIMIT: usize = 16 << 20;

    struct Handle {
        connection: Connection,
        queue: QueueHandle<Watcher>,
        manager: ExtDataControlManagerV1,
        device: ExtDataControlDeviceV1,
    }

    static HANDLE: OnceLock<Handle> = OnceLock::new();
    static OURS: AtomicBool = AtomicBool::new(false);

    #[derive(Default)]
    struct Mimes(Mutex<Vec<String>>);

    struct Payload {
        mimes: Vec<&'static str>,
        bytes: Arc<Vec<u8>>,
    }

    struct Watcher {
        send: UnboundedSender<Clip>,
        gone: bool,
    }

    pub fn watch(send: UnboundedSender<Clip>) -> Result<(), String> {
        let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
        let (globals, mut queue) =
            registry_queue_init::<Watcher>(&connection).map_err(|err| err.to_string())?;
        let handle = queue.handle();
        let seat: wl_seat::WlSeat = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("no seat: {err}"))?;
        let manager: ExtDataControlManagerV1 = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("the compositor doesn't share the clipboard: {err}"))?;
        let device = manager.get_data_device(&seat, &handle, ());
        let _ = HANDLE.set(Handle {
            connection: connection.clone(),
            queue: handle,
            manager,
            device,
        });
        let mut watcher = Watcher { send, gone: false };
        while !watcher.gone {
            queue
                .blocking_dispatch(&mut watcher)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn copy(content: &Content) {
        let Some(handle) = HANDLE.get() else {
            return;
        };
        let payload = match content {
            Content::Text(text) => Payload {
                mimes: TEXT.to_vec(),
                bytes: Arc::new(text.as_bytes().to_vec()),
            },
            Content::Png(bytes) => Payload {
                mimes: vec![PNG],
                bytes: bytes.clone(),
            },
        };
        let mimes = payload.mimes.clone();
        let source = handle.manager.create_data_source(&handle.queue, payload);
        for mime in mimes {
            source.offer(mime.to_owned());
        }
        OURS.store(true, Ordering::SeqCst);
        handle.device.set_selection(Some(&source));
        let _ = handle.connection.flush();
    }

    fn read(offer: &ExtDataControlOfferV1, mime: &str, connection: &Connection) -> Option<Vec<u8>> {
        let (mut reader, writer) = std::io::pipe().ok()?;
        offer.receive(mime.to_owned(), writer.as_fd());
        drop(writer);
        connection.flush().ok()?;
        let (done, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut reader).take(LIMIT as u64 + 1).read_to_end(&mut bytes);
            let _ = done.send(read.ok().map(|_| bytes));
        });
        wait.recv_timeout(Duration::from_secs(2))
            .ok()
            .flatten()
            .filter(|bytes| !bytes.is_empty() && bytes.len() <= LIMIT)
    }

    fn take(offer: &ExtDataControlOfferV1, connection: &Connection) -> Option<Content> {
        let mimes = offer.data::<Mimes>()?.0.lock().ok()?.clone();
        if mimes.iter().any(|m| m == SECRET) {
            return None;
        }
        if let Some(mime) = TEXT.iter().find(|t| mimes.iter().any(|m| m == *t)) {
            let text = String::from_utf8_lossy(&read(offer, mime, connection)?).into_owned();
            return (!text.trim().is_empty()).then_some(Content::Text(text));
        }
        if mimes.iter().any(|m| m == PNG) {
            return read(offer, PNG, connection).map(|bytes| Content::Png(Arc::new(bytes)));
        }
        None
    }

    impl Dispatch<ExtDataControlDeviceV1, ()> for Watcher {
        fn event(
            watcher: &mut Self,
            _: &ExtDataControlDeviceV1,
            event: ext_data_control_device_v1::Event,
            _: &(),
            connection: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                ext_data_control_device_v1::Event::Selection { id: Some(offer) } => {
                    if !OURS.swap(false, Ordering::SeqCst)
                        && let Some(content) = take(&offer, connection)
                        && watcher.send.unbounded_send(Clip::new(content)).is_err()
                    {
                        watcher.gone = true;
                    }
                    offer.destroy();
                }
                ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => {
                    offer.destroy();
                }
                ext_data_control_device_v1::Event::Finished => watcher.gone = true,
                _ => {}
            }
        }

        event_created_child!(Watcher, ExtDataControlDeviceV1, [
            ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, Mimes::default()),
        ]);
    }

    impl Dispatch<ExtDataControlOfferV1, Mimes> for Watcher {
        fn event(
            _: &mut Self,
            _: &ExtDataControlOfferV1,
            event: ext_data_control_offer_v1::Event,
            mimes: &Mimes,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let ext_data_control_offer_v1::Event::Offer { mime_type } = event
                && let Ok(mut mimes) = mimes.0.lock()
            {
                mimes.push(mime_type);
            }
        }
    }

    impl Dispatch<ExtDataControlSourceV1, Payload> for Watcher {
        fn event(
            _: &mut Self,
            source: &ExtDataControlSourceV1,
            event: ext_data_control_source_v1::Event,
            payload: &Payload,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                ext_data_control_source_v1::Event::Send { fd, .. } => {
                    let mut file = std::fs::File::from(fd);
                    let _ = file.write_all(&payload.bytes);
                }
                ext_data_control_source_v1::Event::Cancelled => source.destroy(),
                _ => {}
            }
        }
    }

    impl Dispatch<ExtDataControlManagerV1, ()> for Watcher {
        fn event(
            _: &mut Self,
            _: &ExtDataControlManagerV1,
            _: ext_data_control_manager_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<wl_seat::WlSeat, ()> for Watcher {
        fn event(
            _: &mut Self,
            _: &wl_seat::WlSeat,
            _: wl_seat::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Watcher {
        fn event(
            _: &mut Self,
            _: &wl_registry::WlRegistry,
            _: wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod wayland {
    use iced::futures::channel::mpsc::UnboundedSender;

    use super::{Clip, Content};

    pub fn watch(_: UnboundedSender<Clip>) -> Result<(), String> {
        Ok(())
    }

    pub fn copy(_: &Content) {}
}

pub fn sample() -> Vec<Clip> {
    let text = |text: &str| Clip::new(Content::Text(text.to_owned()));
    let mut clips = vec![
        text("cargo add iced --features tokio"),
        text("#e9b04b"),
        text("https://docs.rs/iced/latest/iced/widget"),
        text(
            "pub struct Workspace {\n    pub project: PathBuf,\n    pub layout: Layout,\n    windows: Vec<WindowId>,\n}",
        ),
        text("ssh arch.local"),
    ];
    clips[0].pinned = true;
    clips[4].pinned = true;
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_guessed_from_the_text() {
        let kind = |text: &str| Clip::new(Content::Text(text.into())).kind();
        assert_eq!(kind("https://docs.rs"), Kind::Link);
        assert_eq!(kind(" #86bdf0 "), Kind::Color);
        assert_eq!(kind("ssh arch.local"), Kind::Text);
        let png = Clip::new(Content::Png(Arc::new(
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x05\0\0\0\x02\xd0".to_vec(),
        )));
        assert_eq!(png.kind(), Kind::Image);
        assert_eq!(png.line(), "Image 1280×720");
    }

    #[test]
    fn copying_again_moves_to_the_top_and_keeps_the_pin() {
        let mut clips = sample();
        remember(
            &mut clips,
            Clip::new(Content::Text("ssh arch.local".into())),
        );
        assert_eq!(clips[0].line(), "ssh arch.local");
        assert!(clips[0].pinned);
        assert_eq!(clips.len(), 5);
        for i in 0..150 {
            remember(&mut clips, Clip::new(Content::Text(format!("note {i}"))));
        }
        assert_eq!(clips.iter().filter(|c| !c.pinned).count(), 100);
        assert_eq!(clips.iter().filter(|c| c.pinned).count(), 2);
    }
}
