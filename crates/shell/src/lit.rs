// Buttons whose hover look fades in and out rather than switching at once. `button` stands in for
// iced's: the same builder, and each style is asked for its resting and hovered looks, blended.
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use iced::advanced::layout;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::time::Instant;
use iced::widget::button::{Button, Status, Style};
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Rectangle, Shadow, Size, Theme,
    Vector, mouse, window,
};

use crate::motion::{Speed, Tween};

const FADE: Duration = Duration::from_millis(120);

static FADE_MICROS: AtomicU64 = AtomicU64::new(FADE.as_micros() as u64);

thread_local! {
    // How lit the button being drawn is, for its style to read.
    static LIT: Cell<Option<f32>> = const { Cell::new(None) };
}

pub fn pace(speed: Speed) {
    let micros = u64::try_from(speed.time(FADE).as_micros()).unwrap_or(u64::MAX);
    FADE_MICROS.store(micros, Ordering::Relaxed);
}

fn fade() -> Duration {
    Duration::from_micros(FADE_MICROS.load(Ordering::Relaxed))
}

pub struct Lit<'a, Message> {
    button: Button<'a, Message>,
    pressable: bool,
}

pub fn button<'a, Message>(content: impl Into<Element<'a, Message>>) -> Lit<'a, Message> {
    Lit {
        button: iced::widget::button(content),
        pressable: false,
    }
}

impl<'a, Message: Clone + 'a> Lit<'a, Message> {
    pub fn on_press(self, message: Message) -> Self {
        Self {
            button: self.button.on_press(message),
            pressable: true,
        }
    }

    pub fn on_press_maybe(self, message: Option<Message>) -> Self {
        Self {
            pressable: message.is_some(),
            button: self.button.on_press_maybe(message),
        }
    }

    pub fn width(self, width: impl Into<Length>) -> Self {
        Self {
            button: self.button.width(width),
            ..self
        }
    }

    pub fn height(self, height: impl Into<Length>) -> Self {
        Self {
            button: self.button.height(height),
            ..self
        }
    }

    pub fn padding(self, padding: impl Into<Padding>) -> Self {
        Self {
            button: self.button.padding(padding),
            ..self
        }
    }

    pub fn clip(self, clip: bool) -> Self {
        Self {
            button: self.button.clip(clip),
            ..self
        }
    }

    pub fn style(self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self {
        Self {
            button: self
                .button
                .style(move |theme, status| faded(&style, theme, status)),
            ..self
        }
    }
}

impl<'a, Message: Clone + 'a> From<Lit<'a, Message>> for Element<'a, Message> {
    fn from(lit: Lit<'a, Message>) -> Self {
        Element::new(Fade {
            content: lit.button.into(),
            pressable: lit.pressable,
        })
    }
}

struct Fade<'a, Message> {
    content: Element<'a, Message>,
    pressable: bool,
}

struct State(Tween);

impl<Message> Widget<Message, Theme, iced::Renderer> for Fade<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State(Tween::still(0.0, Instant::now())))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let State(tween) = tree.state.downcast_mut::<State>();
            let over = self.pressable && cursor.is_over(layout.bounds());
            *tween = tween.go(if over { 1.0 } else { 0.0 }, *now, fade());
            if tween.moving(*now) {
                shell.request_redraw();
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let State(tween) = tree.state.downcast_ref::<State>();
        let outer = LIT.replace(Some(tween.value(Instant::now())));
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        LIT.set(outer);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

fn faded(style: &impl Fn(&Theme, Status) -> Style, theme: &Theme, status: Status) -> Style {
    match (status, LIT.get()) {
        (Status::Active | Status::Hovered, Some(lit)) => blend(
            style(theme, Status::Active),
            style(theme, Status::Hovered),
            lit,
        ),
        _ => style(theme, status),
    }
}

fn blend(rest: Style, lit: Style, t: f32) -> Style {
    if t <= 0.0 {
        return rest;
    }
    if t >= 1.0 {
        return lit;
    }
    Style {
        background: match (rest.background, lit.background) {
            (Some(Background::Color(a)), Some(Background::Color(b))) => {
                Some(Background::Color(mix(a, b, t)))
            }
            (None, Some(Background::Color(b))) => {
                Some(Background::Color(mix(Color::TRANSPARENT, b, t)))
            }
            (Some(Background::Color(a)), None) => {
                Some(Background::Color(mix(a, Color::TRANSPARENT, t)))
            }
            (a, b) => {
                if t < 0.5 {
                    a
                } else {
                    b
                }
            }
        },
        text_color: mix(rest.text_color, lit.text_color, t),
        border: Border {
            color: mix(rest.border.color, lit.border.color, t),
            width: lerp(rest.border.width, lit.border.width, t),
            radius: lit.border.radius,
        },
        shadow: Shadow {
            color: mix(rest.shadow.color, lit.shadow.color, t),
            offset: rest.shadow.offset + (lit.shadow.offset - rest.shadow.offset) * t,
            blur_radius: lerp(rest.shadow.blur_radius, lit.shadow.blur_radius, t),
        },
        snap: lit.snap,
    }
}

// A clear end takes the other's hue, so a fade from nothing doesn't pass through black.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let a = if a.a == 0.0 { Color { a: 0.0, ..b } } else { a };
    let b = if b.a == 0.0 { Color { a: 0.0, ..a } } else { b };
    Color {
        r: lerp(a.r, b.r, t),
        g: lerp(a.g, b.g, t),
        b: lerp(a.b, b.b, t),
        a: lerp(a.a, b.a, t),
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(background: Option<Color>, text: Color) -> Style {
        Style {
            background: background.map(Background::Color),
            text_color: text,
            ..Style::default()
        }
    }

    #[test]
    fn halfway_lit_is_halfway_between_the_two_looks() {
        let white = Color::WHITE;
        let rest = look(None, white);
        let lit = look(Some(Color::from_rgba(1.0, 1.0, 1.0, 0.2)), Color::BLACK);
        assert_eq!(blend(rest, lit, 0.0), rest);
        assert_eq!(blend(rest, lit, 1.0), lit);
        let half = blend(rest, lit, 0.5);
        assert_eq!(
            half.background,
            Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.1)))
        );
        assert_eq!(half.text_color, Color::from_rgb(0.5, 0.5, 0.5));
    }

    #[test]
    fn a_style_sees_its_rest_and_hover_looks_only_while_one_is_drawn() {
        let style = |_: &Theme, status| match status {
            Status::Hovered => look(Some(Color::WHITE), Color::BLACK),
            Status::Pressed => look(Some(Color::BLACK), Color::WHITE),
            _ => look(None, Color::WHITE),
        };
        let theme = Theme::Dark;
        assert_eq!(
            faded(&style, &theme, Status::Hovered),
            style(&theme, Status::Hovered)
        );
        LIT.set(Some(0.0));
        assert_eq!(
            faded(&style, &theme, Status::Hovered),
            style(&theme, Status::Active)
        );
        assert_eq!(
            faded(&style, &theme, Status::Pressed),
            style(&theme, Status::Pressed)
        );
        LIT.set(Some(1.0));
        assert_eq!(
            faded(&style, &theme, Status::Active),
            style(&theme, Status::Hovered)
        );
        LIT.set(None);
    }

    #[test]
    fn no_animations_means_no_fade() {
        pace(Speed::new(Some(0.0), false));
        assert_eq!(fade(), Duration::ZERO);
        pace(Speed::default());
        assert_eq!(fade(), FADE);
    }
}
