use std::path::{Path, PathBuf};
use std::time::Duration;

use iced::futures::{SinkExt, Stream};

use super::{Message, stamp};

// The config file is looked at when the kernel says its folder changed; where it can't, or the
// folder isn't there yet, once a second.
pub(super) fn watch_config() -> impl Stream<Item = Message> {
    let path = apolline_config::default_path();
    iced::stream::channel(1, async move |mut output| {
        let mut seen = stamp(&path);
        let mut folder: Option<Folder> = None;
        let mut watched = Vec::new();
        loop {
            if folder.is_none() {
                watched = folders(&path);
                folder = Folder::watch(&watched);
            }
            let changed = match &mut folder {
                Some(watch) => watch.changed().await,
                None => false,
            };
            if changed {
                // A save is often a few events: a write, then a rename over the old file.
                tokio::time::sleep(Duration::from_millis(100)).await;
                if let Some(watch) = &mut folder {
                    watch.drain();
                }
                // Gone, or now a link to somewhere else: watched anew.
                if !path.parent().is_some_and(Path::exists) || folders(&path) != watched {
                    folder = None;
                }
            } else {
                folder = None;
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            let now = stamp(&path);
            if now != seen {
                seen = now;
                if output.send(Message::ConfigChanged).await.is_err() {
                    return;
                }
            }
        }
    })
}

// The config's folder, and the folder of the file it links to, if it's a link into dotfiles.
fn folders(path: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = path.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(real) = std::fs::canonicalize(path)
        .ok()
        .and_then(|real| real.parent().map(Path::to_path_buf))
        && !folders.contains(&real)
    {
        folders.push(real);
    }
    folders
}

// Files in a few folders being written, moved, made or removed, from inotify.
#[cfg(target_os = "linux")]
struct Folder {
    fd: tokio::io::unix::AsyncFd<std::os::fd::OwnedFd>,
}

#[cfg(target_os = "linux")]
impl Folder {
    fn watch(folders: &[PathBuf]) -> Option<Self> {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: inotify_init1 takes flags and returns a new descriptor or -1.
        let raw = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw < 0 {
            return None;
        }
        // SAFETY: the descriptor is new and owned by nothing else.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let mask = libc::IN_CLOSE_WRITE
            | libc::IN_MOVED_TO
            | libc::IN_MOVED_FROM
            | libc::IN_CREATE
            | libc::IN_DELETE
            | libc::IN_DELETE_SELF
            | libc::IN_MOVE_SELF;
        let mut watched = false;
        for folder in folders {
            let Ok(name) = std::ffi::CString::new(folder.as_os_str().as_bytes()) else {
                continue;
            };
            // SAFETY: a descriptor we own and a nul-terminated path that outlives the call.
            watched |= unsafe { libc::inotify_add_watch(fd.as_raw_fd(), name.as_ptr(), mask) } >= 0;
        }
        watched.then_some(())?;
        // SAFETY: an OwnedFd stays open and is the same descriptor for as long as it's held.
        let fd = unsafe { tokio::io::unix::AsyncFd::register(fd) }.ok()?;
        Some(Self { fd })
    }

    // Waits for any event; which one doesn't matter, the file's time says if it changed.
    async fn changed(&mut self) -> bool {
        loop {
            let Ok(mut ready) = self.fd.readable().await else {
                return false;
            };
            match ready.try_io(|fd| read(fd.get_ref())) {
                Ok(read) => return read.is_ok(),
                Err(_would_block) => {}
            }
        }
    }

    fn drain(&mut self) {
        while read(self.fd.get_ref()).is_ok() {}
    }
}

#[cfg(target_os = "linux")]
fn read(fd: &std::os::fd::OwnedFd) -> std::io::Result<usize> {
    use std::os::fd::AsRawFd;
    let mut events = [0u8; 4096];
    // SAFETY: reads into a buffer of the length given, from a descriptor we own.
    let read = unsafe { libc::read(fd.as_raw_fd(), events.as_mut_ptr().cast(), events.len()) };
    if read < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(read as usize)
}

#[cfg(not(target_os = "linux"))]
struct Folder;

#[cfg(not(target_os = "linux"))]
impl Folder {
    fn watch(_: &[PathBuf]) -> Option<Self> {
        None
    }

    async fn changed(&mut self) -> bool {
        false
    }

    fn drain(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_linked_config_is_watched_where_it_really_is() {
        let base = std::env::temp_dir().join(format!("apolline-watch-{}", std::process::id()));
        let (config, dotfiles) = (base.join("config"), base.join("dotfiles"));
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&dotfiles).unwrap();
        std::fs::write(dotfiles.join("config.toml"), "").unwrap();
        std::os::unix::fs::symlink(dotfiles.join("config.toml"), config.join("config.toml"))
            .unwrap();
        let found = folders(&config.join("config.toml"));
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], config);
        assert!(found[1].ends_with("dotfiles"));
        assert_eq!(folders(Path::new("/nowhere/config.toml")).len(), 1);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_write_in_the_folder_is_heard() {
        let base = std::env::temp_dir().join(format!("apolline-heard-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let heard = runtime.block_on(async {
            let mut folder = Folder::watch(std::slice::from_ref(&base)).unwrap();
            std::fs::write(base.join("config.toml"), "x").unwrap();
            tokio::time::timeout(Duration::from_secs(2), folder.changed()).await
        });
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(heard, Ok(true));
    }
}
