use std::sync::Mutex;

use iced::Subscription;
use iced::futures::Stream;

// Only one client may hold a screen's gamma, so a new night light waits for the old one to let go.
static SCREENS: Mutex<()> = Mutex::new(());

// Keyed by the screens too, so a monitor plugged in later gets warmed as well.
pub fn subscription(temperature: Option<u32>, outputs: &[String]) -> Subscription<()> {
    match temperature {
        Some(kelvin) => Subscription::run_with((kelvin, outputs.to_vec()), hold),
        None => Subscription::none(),
    }
}

fn hold((kelvin, _): &(u32, Vec<String>)) -> impl Stream<Item = ()> + use<> {
    let kelvin = *kelvin;
    iced::stream::channel(1, async move |_output| {
        let (keep, stop) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            let _screens = SCREENS.lock().unwrap_or_else(|held| held.into_inner());
            if let Err(err) = wayland::warm(kelvin, stop) {
                eprintln!("apolline: night light: {err}");
            }
        });
        iced::futures::future::pending::<()>().await;
        drop(keep);
    })
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn ramp(kelvin: u32, size: usize) -> Vec<u16> {
    let [r, g, b] = white(kelvin);
    let mut table = vec![0u16; size * 3];
    for i in 0..size {
        let level = if size > 1 {
            i as f64 / (size - 1) as f64
        } else {
            1.0
        };
        for (channel, gain) in [r, g, b].into_iter().enumerate() {
            table[channel * size + i] = (level * gain * f64::from(u16::MAX)).round() as u16;
        }
    }
    table
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn white(kelvin: u32) -> [f64; 3] {
    let t = f64::from(kelvin.clamp(1000, 6500)) / 100.0;
    let red = if t <= 66.0 {
        1.0
    } else {
        (329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2) / 255.0).clamp(0.0, 1.0)
    };
    let green = if t <= 66.0 {
        (99.470_802_586_1 * t.ln() - 161.119_568_166_1) / 255.0
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2) / 255.0
    }
    .clamp(0.0, 1.0);
    let blue = if t >= 66.0 {
        1.0
    } else if t <= 19.0 {
        0.0
    } else {
        ((138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7) / 255.0).clamp(0.0, 1.0)
    };
    [red, green, blue]
}

#[cfg(target_os = "linux")]
mod wayland {
    use std::io::{Seek, Write};
    use std::sync::mpsc::Receiver;

    use wayland_client::globals::{GlobalListContents, registry_queue_init};
    use wayland_client::protocol::{wl_output, wl_registry};
    use wayland_client::{Connection, Dispatch, QueueHandle};
    use wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_manager_v1::{
        self, ZwlrGammaControlManagerV1,
    };
    use wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_v1::{
        self, ZwlrGammaControlV1,
    };

    #[derive(Default)]
    struct Ramps {
        sizes: Vec<(u32, usize)>,
        failed: bool,
    }

    impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Ramps {
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

    impl Dispatch<wl_output::WlOutput, ()> for Ramps {
        fn event(
            _: &mut Self,
            _: &wl_output::WlOutput,
            _: wl_output::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<ZwlrGammaControlManagerV1, ()> for Ramps {
        fn event(
            _: &mut Self,
            _: &ZwlrGammaControlManagerV1,
            _: zwlr_gamma_control_manager_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }

    impl Dispatch<ZwlrGammaControlV1, u32> for Ramps {
        fn event(
            state: &mut Self,
            _: &ZwlrGammaControlV1,
            event: zwlr_gamma_control_v1::Event,
            output: &u32,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                zwlr_gamma_control_v1::Event::GammaSize { size } => {
                    state.sizes.push((*output, size as usize));
                }
                zwlr_gamma_control_v1::Event::Failed => state.failed = true,
                _ => {}
            }
        }
    }

    pub fn warm(kelvin: u32, stop: Receiver<()>) -> Result<(), String> {
        let connection = Connection::connect_to_env().map_err(|err| err.to_string())?;
        let (globals, mut queue) =
            registry_queue_init::<Ramps>(&connection).map_err(|err| err.to_string())?;
        let handle = queue.handle();
        let manager: ZwlrGammaControlManagerV1 = globals
            .bind(&handle, 1..=1, ())
            .map_err(|_| "niri doesn't offer gamma control".to_owned())?;
        let outputs: Vec<(u32, wl_output::WlOutput)> = globals.contents().with_list(|list| {
            list.iter()
                .filter(|global| global.interface == "wl_output")
                .map(|global| {
                    let output = globals.registry().bind::<wl_output::WlOutput, _, _>(
                        global.name,
                        global.version.min(4),
                        &handle,
                        (),
                    );
                    (global.name, output)
                })
                .collect()
        });
        let controls: Vec<(u32, ZwlrGammaControlV1)> = outputs
            .iter()
            .map(|(name, output)| (*name, manager.get_gamma_control(output, &handle, *name)))
            .collect();
        let mut ramps = Ramps::default();
        queue.roundtrip(&mut ramps).map_err(|err| err.to_string())?;
        if ramps.failed && ramps.sizes.is_empty() {
            return Err("the screens refused a new gamma table".into());
        }
        for (name, control) in &controls {
            let Some((_, size)) = ramps.sizes.iter().find(|(n, _)| n == name) else {
                continue;
            };
            let table = super::ramp(kelvin, *size);
            let bytes: Vec<u8> = table.iter().flat_map(|v| v.to_ne_bytes()).collect();
            let mut file = memfd()?;
            file.write_all(&bytes)
                .and_then(|()| file.rewind())
                .map_err(|err| err.to_string())?;
            control.set_gamma(std::os::fd::AsFd::as_fd(&file));
        }
        connection.flush().map_err(|err| err.to_string())?;
        let _ = stop.recv();
        for (_, control) in controls {
            control.destroy();
        }
        connection.flush().map_err(|err| err.to_string())
    }

    fn memfd() -> Result<std::fs::File, String> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(std::env::temp_dir);
        let path = dir.join(format!("apolline-gamma-{}", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .map_err(|err| err.to_string())?;
        let _ = std::fs::remove_file(&path);
        Ok(file)
    }
}

#[cfg(not(target_os = "linux"))]
mod wayland {
    pub fn warm(_: u32, stop: std::sync::mpsc::Receiver<()>) -> Result<(), String> {
        let _ = stop.recv();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmer_light_keeps_red_and_drops_blue() {
        let [r, g, b] = white(6500);
        assert!(
            r > 0.99 && g > 0.95 && b > 0.95,
            "daylight is neutral: {r} {g} {b}"
        );
        let [r, g, b] = white(3500);
        assert!(r > 0.99 && g < 0.85 && b < 0.65 && b < g, "{r} {g} {b}");
        let table = ramp(3500, 4);
        assert_eq!(table.len(), 12);
        assert_eq!(table[0], 0);
        assert_eq!(table[3], u16::MAX, "red at full");
        assert!(table[11] < table[7], "blue below green at the top");
    }
}
