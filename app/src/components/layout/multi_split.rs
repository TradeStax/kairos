use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{Limits, Node},
        overlay,
        renderer::Style,
        widget::{Operation, Tree, tree},
    },
    mouse::{self, Cursor, Interaction},
    widget::rule,
};
use std::fmt::{Debug, Formatter};

use crate::style;

pub const DRAG_SIZE: f32 = 1.0;
const MIN_PANEL_SIZE: f32 = 40.0;

/// Axis along which panels are split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitAxis {
    /// Panels stacked vertically, draggable horizontal rules.
    Vertical,
    /// Panels placed side by side, draggable vertical rules.
    Horizontal,
}

#[derive(Default)]
struct State {
    dragging_index: Option<usize>,
    hovering_index: Option<usize>,
}

pub struct MultiSplit<'a, Message> {
    panels: Vec<Element<'a, Message>>,
    splits: &'a Vec<f32>,
    resize: fn(usize, f32) -> Message,
    axis: SplitAxis,
}

impl<Message> Debug for MultiSplit<'_, Message> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiSplit")
            .field("splits", &self.splits)
            .field("axis", &self.axis)
            .finish_non_exhaustive()
    }
}

impl<'a, Message> MultiSplit<'a, Message>
where
    Message: 'a,
{
    /// Create a vertically-split multi-panel widget (panels stacked top-to-bottom).
    pub fn new(
        panels: Vec<Element<'a, Message>>,
        splits: &'a Vec<f32>,
        resize: fn(usize, f32) -> Message,
    ) -> Self {
        debug_assert!(panels.len() >= 2, "MultiSplit needs at least 2 panels");
        debug_assert_eq!(
            panels.len() - 1,
            splits.len(),
            "Number of splits must be one less than number of panels"
        );

        let mut elements = Vec::with_capacity(panels.len() * 2 - 1);
        for (i, panel) in panels.into_iter().enumerate() {
            elements.push(panel);
            if i < splits.len() {
                elements.push(rule::horizontal(DRAG_SIZE).style(style::split_ruler).into());
            }
        }

        Self {
            panels: elements,
            splits,
            resize,
            axis: SplitAxis::Vertical,
        }
    }

    /// Create a horizontally-split multi-panel widget (panels placed left-to-right).
    pub fn horizontal(
        panels: Vec<Element<'a, Message>>,
        splits: &'a Vec<f32>,
        resize: fn(usize, f32) -> Message,
    ) -> Self {
        debug_assert!(panels.len() >= 2, "MultiSplit needs at least 2 panels");
        debug_assert_eq!(
            panels.len() - 1,
            splits.len(),
            "Number of splits must be one less than number of panels"
        );

        let mut elements = Vec::with_capacity(panels.len() * 2 - 1);
        for (i, panel) in panels.into_iter().enumerate() {
            elements.push(panel);
            if i < splits.len() {
                elements.push(rule::vertical(DRAG_SIZE).style(style::split_ruler).into());
            }
        }

        Self {
            panels: elements,
            splits,
            resize,
            axis: SplitAxis::Horizontal,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for MultiSplit<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        self.panels.iter().map(Tree::new).collect()
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.panels);
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        let max_limits = limits.max();
        let panel_count = self.panels.len().div_ceil(2);
        let mut children = Vec::with_capacity(self.panels.len());

        match self.axis {
            SplitAxis::Vertical => {
                let mut current_y = 0.0;
                for i in 0..self.panels.len() {
                    if i % 2 == 0 {
                        let panel_index = i / 2;
                        let is_last = panel_index == panel_count - 1;

                        let height = if is_last {
                            max_limits.height - current_y
                        } else {
                            let split_position = self.splits[panel_index];
                            let split_y = max_limits.height * split_position;
                            split_y - current_y - (DRAG_SIZE * 0.5)
                        };

                        let panel_limits = Limits::new(
                            Size::new(0.0, 0.0),
                            Size::new(max_limits.width, height.max(0.0)),
                        );

                        let panel_node = self.panels[i]
                            .as_widget_mut()
                            .layout(&mut tree.children[i], renderer, &panel_limits)
                            .translate(Vector::new(0.0, current_y));

                        children.push(panel_node);

                        if !is_last {
                            current_y += height;
                        }
                    } else {
                        let ruler_limits = Limits::new(
                            Size::new(0.0, DRAG_SIZE),
                            Size::new(max_limits.width, DRAG_SIZE),
                        );

                        let ruler_node = self.panels[i]
                            .as_widget_mut()
                            .layout(&mut tree.children[i], renderer, &ruler_limits)
                            .translate(Vector::new(0.0, current_y));

                        children.push(ruler_node);
                        current_y += DRAG_SIZE;
                    }
                }
            }
            SplitAxis::Horizontal => {
                let mut current_x = 0.0;
                for i in 0..self.panels.len() {
                    if i % 2 == 0 {
                        let panel_index = i / 2;
                        let is_last = panel_index == panel_count - 1;

                        let width = if is_last {
                            max_limits.width - current_x
                        } else {
                            let split_position = self.splits[panel_index];
                            let split_x = max_limits.width * split_position;
                            split_x - current_x - (DRAG_SIZE * 0.5)
                        };

                        let panel_limits = Limits::new(
                            Size::new(0.0, 0.0),
                            Size::new(width.max(0.0), max_limits.height),
                        );

                        let panel_node = self.panels[i]
                            .as_widget_mut()
                            .layout(&mut tree.children[i], renderer, &panel_limits)
                            .translate(Vector::new(current_x, 0.0));

                        children.push(panel_node);

                        if !is_last {
                            current_x += width;
                        }
                    } else {
                        let ruler_limits = Limits::new(
                            Size::new(DRAG_SIZE, 0.0),
                            Size::new(DRAG_SIZE, max_limits.height),
                        );

                        let ruler_node = self.panels[i]
                            .as_widget_mut()
                            .layout(&mut tree.children[i], renderer, &ruler_limits)
                            .translate(Vector::new(current_x, 0.0));

                        children.push(ruler_node);
                        current_x += DRAG_SIZE;
                    }
                }
            }
        }

        Node::with_children(max_limits, children)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.panels
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .for_each(|((child, tree), layout)| {
                child.as_widget_mut().update(
                    tree, event, layout, cursor, renderer, clipboard, shell, viewport,
                );
            });

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        let mut ruler_bounds = Vec::new();
        for (i, layout_child) in layout.children().enumerate() {
            if i % 2 == 1 {
                ruler_bounds.push((i / 2, layout_child.bounds().expand(DRAG_SIZE * 4.0)));
            }
        }

        if let Event::Mouse(event) = event {
            match event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    for (index, bounds) in &ruler_bounds {
                        if cursor.is_over(*bounds) {
                            state.dragging_index = Some(*index);
                            shell.capture_event();
                            break;
                        }
                    }
                }
                mouse::Event::CursorMoved { position, .. } => {
                    if let Some(index) = state.dragging_index {
                        let split_at = match self.axis {
                            SplitAxis::Vertical => {
                                (position.y - bounds.y) / bounds.height
                            }
                            SplitAxis::Horizontal => {
                                (position.x - bounds.x) / bounds.width
                            }
                        };

                        let dimension = match self.axis {
                            SplitAxis::Vertical => bounds.height,
                            SplitAxis::Horizontal => bounds.width,
                        };
                        let threshold = (DRAG_SIZE + MIN_PANEL_SIZE) / dimension;

                        let lower = if index > 0 {
                            self.splits[index - 1] + threshold
                        } else {
                            threshold
                        };
                        let upper = if index < self.splits.len() - 1 {
                            self.splits[index + 1] - threshold
                        } else {
                            1.0 - threshold
                        };

                        let (min_bound, max_bound) = if lower <= upper {
                            (lower, upper)
                        } else {
                            (upper, lower)
                        };

                        let split_at = split_at.clamp(min_bound, max_bound);

                        shell.publish((self.resize)(index, split_at));
                        shell.capture_event();
                    } else {
                        let mut new_hovering = None;
                        for (index, bounds) in &ruler_bounds {
                            if cursor.is_over(*bounds) {
                                new_hovering = Some(*index);
                                break;
                            }
                        }

                        if state.hovering_index != new_hovering {
                            state.hovering_index = new_hovering;
                            shell.request_redraw();
                        }
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => {
                    if state.dragging_index.is_some() {
                        state.dragging_index = None;
                        shell.capture_event();
                    }
                }
                _ => {}
            }
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.panels
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .filter(|(_, layout)| layout.bounds().intersects(viewport))
            .for_each(|((child, tree), layout)| {
                child
                    .as_widget()
                    .draw(tree, renderer, theme, style, layout, cursor, viewport);
            });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.dragging_index.is_some() || state.hovering_index.is_some() {
            match self.axis {
                SplitAxis::Vertical => Interaction::ResizingVertically,
                SplitAxis::Horizontal => Interaction::ResizingHorizontally,
            }
        } else {
            self.panels
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
                .map(|((child, tree), layout)| {
                    child
                        .as_widget()
                        .mouse_interaction(tree, layout, cursor, viewport, renderer)
                })
                .max()
                .unwrap_or_default()
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.panels,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
    }
}

impl<'a, Message> From<MultiSplit<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
    fn from(widget: MultiSplit<'a, Message>) -> Self {
        Self::new(widget)
    }
}
