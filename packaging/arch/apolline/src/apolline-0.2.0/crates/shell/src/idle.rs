use std::sync::atomic::{AtomicU64, Ordering};

use iced::Subscription;
use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Lock,
    ScreenOff,
    Suspend,
}

#[derive(Hash)]
struct Stages(Vec<(Stage, u32)>);

pub fn subscription(stages: Vec<(Stage, u32)>) -> Subscription<(Stage, bool)> {
    if stages.is_empty() {
        return Subscription::none();
    }
    Subscription::run_with(Stages(stages), watch)
}

// Each set of stages gets a number, so a set that ends after a newer one began can't clear it.
static NEXT: AtomicU64 = AtomicU64::new(1);

fn watch(stages: &Stages) -> impl Stream<Item = (Stage, bool)> + use<> {
    let stages = stages.0.clone();
    iced::stream::channel(8, async move |mut output| {
        let (send, mut events) = mpsc::unbounded();
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Err(err) = wayland::follow(id, stages, Some(send)) {
            eprintln!("apolline: idle: {err}");
            return;
        }
        let _following = Following(id);
        while let Some(event) = events.next().await {
            if output.send(event).await.is_err() {
                return;
            }
        }
    })
}

// Stages stop being watched when their subscription ends: charging, a timer, staying awake.
struct Following(u64);

impl Drop for Following {
    fn drop(&mut self) {
        let _ = wayland::follow(self.0, Vec::new(), None);
    }
}

// One thread and one Wayland connection for the shell's whole life. New stages are handed to it
// and it's woken to take them, rather than a thread started for each set and left waiting.
#[cfg(target_os = "linux")]
mod wayland {
    use std::sync::Mutex;
    use std::sync::mpsc::{Receiver, Sender};

    use iced::futures::channel::mpsc::UnboundedSender;
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_callback, wl_registry, wl_seat};
    use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{
        self, ExtIdleNotificationV1,
    };
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{
        self, ExtIdleNotifierV1,
    };

    use super::Stage;

    type Tell = UnboundedSender<(Stage, bool)>;

    struct Asked {
        id: u64,
        stages: Vec<(Stage, u32)>,
        // None when the stages with this number end.
        send: Option<Tell>,
    }

    struct Running {
        asks: Sender<Asked>,
        connection: Connection,
        queue: QueueHandle<Watcher>,
    }

    static RUNNING: Mutex<Option<Running>> = Mutex::new(None);

    pub fn follow(id: u64, stages: Vec<(Stage, u32)>, send: Option<Tell>) -> Result<(), String> {
        let mut running = RUNNING
            .lock()
            .map_err(|_| "the idle watch broke".to_owned())?;
        if running.is_none() {
            if send.is_none() {
                return Ok(());
            }
            *running = Some(start()?);
        }
        let Some(now) = running.as_ref() else {
            return Ok(());
        };
        if now.asks.send(Asked { id, stages, send }).is_err() {
            *running = None;
            return Err("the idle watch stopped".into());
        }
        // The compositor's answer to a sync is an event on the thread's queue, which wakes it.
        now.connection.display().sync(&now.queue, ());
        now.connection.flush().map_err(|err| err.to_string())
    }

    fn start() -> Result<Running, String> {
        let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
        let (globals, queue) =
            registry_queue_init::<Watcher>(&connection).map_err(|err| err.to_string())?;
        let handle = queue.handle();
        let seat: wl_seat::WlSeat = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("no seat: {err}"))?;
        let notifier: ExtIdleNotifierV1 = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("the compositor has no idle notifications: {err}"))?;
        let (asks, asked) = std::sync::mpsc::channel();
        std::thread::spawn(move || run(queue, &notifier, &seat, &asked));
        Ok(Running {
            asks,
            connection,
            queue: handle,
        })
    }

    fn run(
        mut queue: EventQueue<Watcher>,
        notifier: &ExtIdleNotifierV1,
        seat: &wl_seat::WlSeat,
        asked: &Receiver<Asked>,
    ) {
        let handle = queue.handle();
        let mut watcher = Watcher {
            id: 0,
            send: None,
            notifications: Vec::new(),
        };
        loop {
            while let Ok(asked) = asked.try_recv() {
                watcher.follow(asked, notifier, seat, &handle);
            }
            if let Err(err) = queue.blocking_dispatch(&mut watcher) {
                eprintln!("apolline: idle: {err}");
                return;
            }
        }
    }

    struct Watcher {
        id: u64,
        send: Option<Tell>,
        notifications: Vec<ExtIdleNotificationV1>,
    }

    impl Watcher {
        fn follow(
            &mut self,
            asked: Asked,
            notifier: &ExtIdleNotifierV1,
            seat: &wl_seat::WlSeat,
            handle: &QueueHandle<Self>,
        ) {
            if asked.send.is_none() && asked.id != self.id {
                return;
            }
            self.stop();
            self.id = asked.id;
            self.send = asked.send;
            if self.send.is_some() {
                self.notifications = asked
                    .stages
                    .iter()
                    .map(|&(stage, seconds)| {
                        let millis = seconds.saturating_mul(1000);
                        notifier.get_idle_notification(millis, seat, handle, stage)
                    })
                    .collect();
            }
        }

        fn stop(&mut self) {
            for notification in self.notifications.drain(..) {
                notification.destroy();
            }
            self.send = None;
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

    impl Dispatch<wl_callback::WlCallback, ()> for Watcher {
        fn event(
            _: &mut Self,
            _: &wl_callback::WlCallback,
            _: wl_callback::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<ExtIdleNotifierV1, ()> for Watcher {
        fn event(
            _: &mut Self,
            _: &ExtIdleNotifierV1,
            _: ext_idle_notifier_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<ExtIdleNotificationV1, Stage> for Watcher {
        fn event(
            watcher: &mut Self,
            _: &ExtIdleNotificationV1,
            event: ext_idle_notification_v1::Event,
            stage: &Stage,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            let idle = match event {
                ext_idle_notification_v1::Event::Idled => true,
                ext_idle_notification_v1::Event::Resumed => false,
                _ => return,
            };
            let sent = watcher
                .send
                .as_ref()
                .is_some_and(|send| send.unbounded_send((*stage, idle)).is_ok());
            if !sent {
                watcher.stop();
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod wayland {
    use iced::futures::channel::mpsc::UnboundedSender;

    use super::Stage;

    pub fn follow(
        _: u64,
        _: Vec<(Stage, u32)>,
        _: Option<UnboundedSender<(Stage, bool)>>,
    ) -> Result<(), String> {
        Ok(())
    }
}

pub fn stages(config: &apolline_config::Idle) -> Vec<(Stage, u32)> {
    [
        (Stage::Lock, config.lock_after),
        (Stage::ScreenOff, config.screen_off_after),
        (Stage::Suspend, config.suspend_after),
    ]
    .into_iter()
    .filter(|&(_, minutes)| minutes > 0)
    .map(|(stage, minutes)| (stage, u32::from(minutes) * 60))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_minutes_means_never() {
        let config = apolline_config::Idle {
            lock_after: 5,
            screen_off_after: 0,
            suspend_after: 30,
            battery: apolline_config::IdleTimes::default(),
        };
        assert_eq!(
            stages(&config),
            [(Stage::Lock, 300), (Stage::Suspend, 1800)]
        );
    }
}
