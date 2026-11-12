// The wallpaper softens while niri's overview is open, so the workspaces stand out over it. The
// soft picture is made small, once for each wallpaper, as the lock screen's is.

use super::*;

const SOFTEN: Duration = Duration::from_millis(250);

impl Shell {
    fn softens(&self) -> bool {
        !self.demo && self.niri.is_some() && !self.current().is_some_and(|m| m.performance)
    }

    pub(super) fn load_blurred(&self) -> Task<Message> {
        if !self.softens() {
            return Task::none();
        }
        let missing: Vec<String> = self
            .wanted_wallpapers()
            .into_iter()
            .filter(|name| !self.blurred.iter().any(|(n, _)| n == name))
            .collect();
        Task::batch(missing.into_iter().map(|name| {
            Task::perform(wallpaper::load_blurred(name.clone()), move |result| {
                Message::Blurred(name.clone(), result)
            })
        }))
    }

    pub(super) fn overview_changed(&mut self) -> Task<Message> {
        self.overview_at = Some(Instant::now());
        self.load_blurred()
    }

    pub(super) fn softening(&self) -> bool {
        self.overview_at
            .is_some_and(|at| at.elapsed() < self.soften_time())
    }

    fn soften_time(&self) -> Duration {
        self.speed().time(SOFTEN)
    }

    // How far the soft picture has come in, from none to all of it.
    fn softness(&self) -> f32 {
        let open = self
            .niri
            .as_ref()
            .is_some_and(apolline_niri::State::overview);
        if !self.softens() {
            return 0.0;
        }
        let length = self.soften_time();
        let t = match self.overview_at {
            Some(at) if !length.is_zero() => {
                (at.elapsed().as_secs_f32() / length.as_secs_f32()).min(1.0)
            }
            _ => 1.0,
        };
        let eased = 1.0 - (1.0 - t).powi(3);
        if open { eased } else { 1.0 - eased }
    }

    pub(super) fn softened<'a>(
        &'a self,
        name: &str,
        sharp: Element<'a, Message>,
    ) -> Element<'a, Message> {
        let softness = self.softness();
        let Some((_, handle)) = self.blurred.iter().find(|(n, _)| n == name) else {
            return sharp;
        };
        if softness <= 0.0 {
            return sharp;
        }
        let soft = image(handle.clone())
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .opacity(softness);
        let dim = container(space()).width(Fill).height(Fill).style(move |_| {
            container::background(style::color(neutral::ink().alpha(0.25 * softness)))
        });
        stack![sharp, soft, dim].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wallpaper_softens_while_the_overview_is_open_and_not_in_a_game() {
        let (mut shell, _) = Shell::new(None, true);
        shell.demo = false;
        shell.mode = None;
        shell.niri = Some(apolline_niri::State::default());
        shell.config.desktop.wallpaper = "fog.jpg".into();
        shell.config.windows.animations = Some(0.0);
        let soft = image::Handle::from_rgba(1, 1, vec![0; 4]);
        let _ = shell.update(Message::Blurred("fog.jpg".into(), Ok(soft)));
        assert_eq!(shell.blurred.len(), 1);
        assert_eq!(shell.softness(), 0.0);

        let open =
            |is_open| Message::Niri(apolline_niri::Event::OverviewOpenedOrClosed { is_open });
        let _ = shell.update(open(true));
        assert_eq!(
            shell.softness(),
            1.0,
            "without animations it's soft at once"
        );
        let _ = shell.update(Message::Frame);
        assert_eq!(shell.overview_at, None);

        shell.config.modes = vec![ModeSettings {
            performance: true,
            ..ModeSettings::blank("Play")
        }];
        shell.mode = Some(0);
        assert_eq!(shell.softness(), 0.0, "a game keeps it sharp");
        shell.mode = None;
        let _ = shell.update(open(false));
        assert_eq!(shell.softness(), 0.0);
    }
}
