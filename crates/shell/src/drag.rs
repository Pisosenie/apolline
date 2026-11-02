use iced::advanced::layout;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme, Vector, mouse};

// How far the pointer moves with the button down before a press becomes a drag.
const THRESHOLD: f32 = 5.0;

// Lets what's inside be picked up and moved. A press goes through to the content as usual; once the
// pointer has moved far enough it becomes a drag, the content stops hearing the pointer, and the
// drop doesn't count as a click on whatever is under it. Points are relative to the widget, and
// keep coming while the pointer is outside it.
pub struct Drag<'a, Message> {
    content: Element<'a, Message>,
    on_start: Box<dyn Fn(Point) -> Message + 'a>,
    on_move: Box<dyn Fn(Point) -> Message + 'a>,
    on_drop: Box<dyn Fn(Point) -> Message + 'a>,
}

pub fn drag<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    on_start: impl Fn(Point) -> Message + 'a,
    on_move: impl Fn(Point) -> Message + 'a,
    on_drop: impl Fn(Point) -> Message + 'a,
) -> Drag<'a, Message> {
    Drag {
        content: content.into(),
        on_start: Box::new(on_start),
        on_move: Box::new(on_move),
        on_drop: Box::new(on_drop),
    }
}

#[derive(Default)]
struct State {
    pressed: Option<Point>,
    dragging: bool,
    last: Point,
}

impl<Message> Widget<Message, Theme, iced::Renderer> for Drag<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
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
        let state = tree.state.downcast_mut::<State>();
        let origin = layout.bounds().position();
        let inside = |p: Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.pressed = cursor.position_over(layout.bounds());
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(from) = state.pressed {
                    state.last = *position;
                    if !state.dragging && from.distance(*position) > THRESHOLD {
                        state.dragging = true;
                        shell.publish((self.on_start)(inside(from)));
                    }
                    if state.dragging {
                        shell.publish((self.on_move)(inside(*position)));
                        shell.capture_event();
                        shell.request_redraw();
                        return;
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let dragging = state.dragging;
                state.pressed = None;
                state.dragging = false;
                if dragging {
                    shell.publish((self.on_drop)(inside(state.last)));
                    shell.capture_event();
                    return;
                }
            }
            _ => {}
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
        if tree.state.downcast_ref::<State>().dragging {
            return mouse::Interaction::Grabbing;
        }
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
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

impl<'a, Message: 'a> From<Drag<'a, Message>> for Element<'a, Message> {
    fn from(drag: Drag<'a, Message>) -> Self {
        Element::new(drag)
    }
}
