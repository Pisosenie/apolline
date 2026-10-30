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

fn watch(stages: &Stages) -> impl Stream<Item = (Stage, bool)> + use<> {
    let stages = stages.0.clone();
    iced::stream::channel(8, async move |mut output| {
        let (send, mut events) = mpsc::unbounded();
        std::thread::spawn(move || {
            if let Err(err) = wayland::watch(&stages, send) {
                eprintln!("apolline: idle: {err}");
            }
        });
        while let Some(event) = events.next().await {
            if output.send(event).await.is_err() {
                return;
            }
        }
    })
}

#[cfg(target_os = "linux")]
mod wayland {
    use iced::futures::channel::mpsc::UnboundedSender;
    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_registry, wl_seat};
    use wayland_client::{Connection, Dispatch, QueueHandle};
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::{
        self, ExtIdleNotificationV1,
    };
    use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::{
        self, ExtIdleNotifierV1,
    };

    use super::Stage;

    struct Watcher {
        send: UnboundedSender<(Stage, bool)>,
        gone: bool,
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
            if watcher.send.unbounded_send((*stage, idle)).is_err() {
                watcher.gone = true;
            }
        }
    }

    pub fn watch(
        stages: &[(Stage, u32)],
        send: UnboundedSender<(Stage, bool)>,
    ) -> Result<(), String> {
        let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
        let (globals, mut queue) =
            registry_queue_init::<Watcher>(&connection).map_err(|err| err.to_string())?;
        let handle = queue.handle();
        let seat: wl_seat::WlSeat = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("no seat: {err}"))?;
        let notifier: ExtIdleNotifierV1 = globals
            .bind(&handle, 1..=1, ())
            .map_err(|err| format!("the compositor has no idle notifications: {err}"))?;
        let _notifications: Vec<ExtIdleNotificationV1> = stages
            .iter()
            .map(|&(stage, seconds)| {
                notifier.get_idle_notification(seconds.saturating_mul(1000), &seat, &handle, stage)
            })
            .collect();
        let mut watcher = Watcher { send, gone: false };
        while !watcher.gone {
            queue
                .blocking_dispatch(&mut watcher)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod wayland {
    use iced::futures::channel::mpsc::UnboundedSender;

    use super::Stage;

    pub fn watch(_: &[(Stage, u32)], _: UnboundedSender<(Stage, bool)>) -> Result<(), String> {
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
            awake_in_play: true,
        };
        assert_eq!(
            stages(&config),
            [(Stage::Lock, 300), (Stage::Suspend, 1800)]
        );
    }
}
