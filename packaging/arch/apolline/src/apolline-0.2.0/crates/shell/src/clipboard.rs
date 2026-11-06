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
    // An image made small enough for the preview, decoded once.
    pub picture: Option<iced::widget::image::Handle>,
    // What a search looks through, lowercased once rather than on every frame.
    pub words: String,
    pub when: Instant,
    pub pinned: bool,
}

// The preview is about half this wide; this is for a screen at twice the scale.
const PREVIEW: u32 = 800;
// The history keeps the newest hundred, and no more than this between them: a few screenshots of
// a big screen are a lot of memory.
const KEPT: usize = 100;
const KEPT_BYTES: usize = 64 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Link,
    Color,
    Image,
}

impl Clip {
    pub fn new(content: Content) -> Self {
        let (picture, words) = match &content {
            Content::Png(bytes) => (preview(bytes), "image".to_owned()),
            Content::Text(text) => (None, text.to_lowercase()),
        };
        Self {
            content,
            picture,
            words,
            when: Instant::now(),
            pinned: false,
        }
    }

    // The same clip copied again, without decoding it again.
    pub fn again(&self) -> Self {
        Self {
            when: Instant::now(),
            ..self.clone()
        }
    }

    // Roughly what it holds in memory.
    fn bytes(&self) -> usize {
        let content = match &self.content {
            Content::Text(text) => text.len(),
            Content::Png(bytes) => bytes.len(),
        };
        let picture = match &self.picture {
            Some(iced::widget::image::Handle::Rgba { pixels, .. }) => pixels.len(),
            _ => 0,
        };
        content + picture + self.words.len()
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
                Some((width, height)) => crate::i18n::fill(
                    "Image {width}×{height}",
                    &[("width", &width), ("height", &height)],
                ),
                None => "Image".to_owned(),
            },
        }
    }
}

fn preview(png: &[u8]) -> Option<iced::widget::image::Handle> {
    let picture = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let picture = if picture.width().max(picture.height()) > PREVIEW {
        picture.thumbnail(PREVIEW, PREVIEW)
    } else {
        picture
    };
    let picture = picture.to_rgba8();
    Some(iced::widget::image::Handle::from_rgba(
        picture.width(),
        picture.height(),
        picture.into_raw(),
    ))
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
    let (mut unpinned, mut bytes) = (0, 0);
    clips.retain(|c| {
        if c.pinned {
            return true;
        }
        unpinned += 1;
        bytes += c.bytes();
        unpinned == 1 || (unpinned <= KEPT && bytes <= KEPT_BYTES)
    });
}

// What you pinned outlives the session; the rest of the history is only ever in memory.
fn kept_path() -> Option<std::path::PathBuf> {
    let data = apolline_config::xdg_dir("XDG_DATA_HOME", ".local/share")?;
    Some(data.join("apolline").join("pinned.json"))
}

pub fn pinned() -> Vec<Clip> {
    kept_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| unkeep(&text))
        .unwrap_or_default()
}

// Written whenever a pin changes; only you can read it, since pins can be private.
pub fn keep(clips: &[Clip]) -> iced::Task<crate::shell::Message> {
    let Some(path) = kept_path() else {
        return iced::Task::none();
    };
    let text = keep_text(clips);
    iced::Task::future(async move {
        let written = tokio::task::spawn_blocking(move || write_private(&path, &text)).await;
        if let Ok(Err(err)) = written {
            eprintln!("apolline: pinned clips: {err}");
        }
    })
    .discard()
}

fn keep_text(clips: &[Clip]) -> String {
    use base64::Engine;
    let kept: Vec<serde_json::Value> = clips
        .iter()
        .filter(|clip| clip.pinned)
        .map(|clip| match &clip.content {
            Content::Text(text) => serde_json::json!({ "text": text }),
            Content::Png(bytes) => {
                serde_json::json!({ "png": base64::prelude::BASE64_STANDARD.encode(bytes.as_ref()) })
            }
        })
        .collect();
    serde_json::Value::from(kept).to_string()
}

fn unkeep(text: &str) -> Vec<Clip> {
    use base64::Engine;
    serde_json::from_str::<Vec<serde_json::Value>>(text)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|kept| {
            let content = match (kept["text"].as_str(), kept["png"].as_str()) {
                (Some(text), _) => Content::Text(text.to_owned()),
                (None, Some(data)) => Content::Png(Arc::new(
                    base64::prelude::BASE64_STANDARD.decode(data).ok()?,
                )),
                (None, None) => return None,
            };
            Some(Clip {
                pinned: true,
                ..Clip::new(content)
            })
        })
        .collect()
}

fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let staged = path.with_extension("json.new");
    let mut file = std::fs::OpenOptions::new();
    file.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut file, 0o600);
    file.open(&staged)?.write_all(text.as_bytes())?;
    std::fs::rename(staged, path)
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
    use std::io::{ErrorKind, Read, Write};
    use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant};

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

    // An app that's slow to hand over what it copied gets two seconds, then the pipe is closed
    // on it; nothing is left waiting behind.
    fn read(offer: &ExtDataControlOfferV1, mime: &str, connection: &Connection) -> Option<Vec<u8>> {
        let (mut reader, writer) = std::io::pipe().ok()?;
        offer.receive(mime.to_owned(), writer.as_fd());
        drop(writer);
        connection.flush().ok()?;
        unblock(reader.as_fd()).ok()?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut bytes = Vec::new();
        let mut chunk = [0; 64 << 10];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) if bytes.len() + read <= LIMIT => bytes.extend_from_slice(&chunk[..read]),
                Ok(_) => return None,
                Err(err) if err.kind() == ErrorKind::Interrupted => {}
                Err(err) if err.kind() == ErrorKind::WouldBlock => {
                    if !ready(reader.as_fd(), libc::POLLIN, deadline) {
                        return None;
                    }
                }
                Err(_) => return None,
            }
        }
        (!bytes.is_empty()).then_some(bytes)
    }

    // What you copy from the shell is handed over on a thread of its own, and given up on after a
    // while: an app that never reads it must not hold up the clipboard.
    fn send(file: std::fs::File, bytes: Arc<Vec<u8>>) {
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            if unblock(file.as_fd()).is_err() {
                return;
            }
            let mut rest = bytes.as_slice();
            while !rest.is_empty() {
                match (&file).write(rest) {
                    Ok(0) => return,
                    Ok(written) => rest = &rest[written..],
                    Err(err) if err.kind() == ErrorKind::Interrupted => {}
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        if !ready(file.as_fd(), libc::POLLOUT, deadline) {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });
    }

    fn unblock(fd: BorrowedFd<'_>) -> std::io::Result<()> {
        // SAFETY: F_GETFL and F_SETFL only read and set the flags of a descriptor we hold.
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: as above.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    // Waits for the pipe to be readable or writable, until the deadline.
    fn ready(fd: BorrowedFd<'_>, events: libc::c_short, deadline: Instant) -> bool {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        let mut poll = libc::pollfd {
            fd: fd.as_raw_fd(),
            events,
            revents: 0,
        };
        let millis = left.as_millis().clamp(1, i32::MAX as u128) as i32;
        // SAFETY: one pollfd that lives through the call.
        let answered = unsafe { libc::poll(&mut poll, 1, millis) };
        // A closed pipe counts as ready: the read or write that follows says so.
        answered > 0
            || (answered < 0 && std::io::Error::last_os_error().kind() == ErrorKind::Interrupted)
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
                    send(std::fs::File::from(fd), payload.bytes.clone());
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
    fn only_pins_are_written_down_and_they_come_back_pinned() {
        let pin = |content| Clip {
            pinned: true,
            ..Clip::new(content)
        };
        let clips = vec![
            pin(Content::Text("ssh deploy@10.0.0.4 \"line\"".into())),
            Clip::new(Content::Text("a password, say".into())),
            pin(Content::Png(Arc::new(vec![137, 80, 78, 71]))),
        ];
        let back = unkeep(&keep_text(&clips));
        assert_eq!(back.len(), 2, "the unpinned one isn't kept");
        assert!(back.iter().all(|clip| clip.pinned));
        assert_eq!(back[0].content, clips[0].content);
        assert_eq!(back[1].content, clips[2].content);
        assert!(unkeep("not json").is_empty());
    }

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

    #[test]
    fn big_clips_crowd_out_old_ones_but_not_pins() {
        let mut clips = sample();
        let big = |i: usize| Clip::new(Content::Text(format!("{i}{}", "x".repeat(10 << 20))));
        for i in 0..8 {
            remember(&mut clips, big(i));
        }
        assert_eq!(clips.iter().filter(|c| c.pinned).count(), 2);
        let kept: usize = clips.iter().filter(|c| !c.pinned).map(Clip::bytes).sum();
        assert!(kept <= KEPT_BYTES, "{kept} bytes kept");
        assert!(
            clips[0].text().is_some_and(|t| t.starts_with('7')),
            "the newest stays"
        );
    }

    #[test]
    fn an_image_is_previewed_small_and_found_as_an_image() {
        let mut png = Vec::new();
        image::RgbaImage::new(1600, 400)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let clip = Clip::new(Content::Png(Arc::new(png)));
        let Some(iced::widget::image::Handle::Rgba { width, height, .. }) = &clip.picture else {
            panic!("no preview");
        };
        assert_eq!((*width, *height), (PREVIEW, 200));
        assert_eq!(clip.words, "image");
        assert_eq!(
            clip.again().picture,
            clip.picture,
            "copied again, not decoded again"
        );
    }
}
