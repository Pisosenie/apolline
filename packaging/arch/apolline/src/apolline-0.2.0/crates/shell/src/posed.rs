use std::cell::Cell;

use apolline_config::Edge;
use iced::advanced::layout;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Renderer as _, Shell, Widget};
use iced::{
    Alignment, Element, Event, Length, Rectangle, Size, Theme, Transformation, Vector, mouse,
};

use crate::motion::{self, Pose};
use crate::style;

// Draws what's inside in a pose: faded, moved and scaled, while the layout and the widget state
// stay what they are. The pointer is turned back to the content's own places, so it stays usable
// mid-move; a passive one is on its way out and hears nothing. Its tree is the content's, so
// putting one around something and taking it away again loses no state.
pub struct Posed<'a, Message> {
    content: Element<'a, Message>,
    pose: Pose,
    passive: bool,
}

pub fn posed<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    pose: Pose,
) -> Posed<'a, Message> {
    Posed {
        content: content.into(),
        pose,
        passive: false,
    }
}

impl<Message> Posed<'_, Message> {
    pub fn passive(mut self, passive: bool) -> Self {
        self.passive = passive;
        self
    }

    fn transformation(&self, bounds: Rectangle) -> Transformation {
        let Pose {
            offset,
            scale,
            origin: (x, y),
            ..
        } = self.pose;
        let x = bounds.x + bounds.width * x;
        let y = bounds.y + bounds.height * y;
        Transformation::translate(x + offset.x, y + offset.y)
            * Transformation::scale(scale)
            * Transformation::translate(-x, -y)
    }
}

thread_local! {
    // How far in the overlay being built is, for the cards in it to stand by.
    static ENTERING: Cell<f32> = const { Cell::new(1.0) };
}

// Builds a surface `progress` of the way in: its colors faded to that, and its cards placed by it.
pub fn within<T>(progress: f32, build: impl FnOnce() -> T) -> T {
    let was = ENTERING.replace(progress);
    let built = style::faded(progress, build);
    ENTERING.set(was);
    built
}

// A card that comes in by the `from` edge as the surface it's on does.
pub fn enter<'a, Message: 'a>(
    card: impl Into<Element<'a, Message>>,
    from: Edge,
    along: Alignment,
) -> Element<'a, Message> {
    posed(card, motion::entrance(ENTERING.get(), from, along)).into()
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Posed<'_, Message> {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
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
            .operate(tree, layout, renderer, operation);
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
        if self.passive {
            return;
        }
        let back = self.transformation(layout.bounds()).inverse();
        self.content.as_widget_mut().update(
            tree,
            event,
            layout,
            cursor * back,
            renderer,
            clipboard,
            shell,
            &(*viewport * back),
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
        if self.passive {
            return mouse::Interaction::None;
        }
        let back = self.transformation(layout.bounds()).inverse();
        self.content.as_widget().mouse_interaction(
            tree,
            layout,
            cursor * back,
            &(*viewport * back),
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
        if self.pose.opacity <= 0.0 {
            return;
        }
        let there = self.transformation(layout.bounds());
        let back = there.inverse();
        let mut text_color = style.text_color;
        text_color.a *= self.pose.opacity.min(1.0);
        let style = renderer::Style { text_color };
        // Colors are faded as they're made, the ones drawn now included.
        style::faded(self.pose.opacity, || {
            renderer.with_transformation(there, |renderer| {
                self.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    &style,
                    layout,
                    cursor * back,
                    &(*viewport * back),
                );
            });
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

// Still and listening, it's just its content.
impl<'a, Message: 'a> From<Posed<'a, Message>> for Element<'a, Message> {
    fn from(posed: Posed<'a, Message>) -> Self {
        if posed.pose.is_still() && !posed.passive {
            posed.content
        } else {
            Element::new(posed)
        }
    }
}
