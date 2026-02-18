use iced::widget::{button, row, text};
use iced::{Element, Theme};

use crate::style;
use crate::style::tokens;

/// Visual style for the group.
#[derive(Debug, Clone, Copy, Default)]
pub enum GroupStyle {
    /// Tab-like buttons: active tab is filled, others transparent.
    #[default]
    Tab,
    /// Segmented control: all buttons have borders, active is highlighted.
    Segmented,
}

/// Builder for a row of mutually-exclusive buttons (tabs / segmented
/// control).
pub struct ButtonGroupBuilder<'a, Message> {
    items: Vec<(String, Message)>,
    selected: usize,
    group_style: GroupStyle,
    spacing: f32,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl<'a, Message: Clone + 'a> ButtonGroupBuilder<'a, Message> {
    /// Create a new button group.
    ///
    /// `items` is a list of (label, on_press_message) pairs.
    /// `selected` is the 0-based index of the active item.
    pub fn new(items: Vec<(String, Message)>, selected: usize) -> Self {
        Self {
            items,
            selected,
            group_style: GroupStyle::Tab,
            spacing: tokens::spacing::XXS,
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Use tab visual style.
    pub fn tab_style(mut self) -> Self {
        self.group_style = GroupStyle::Tab;
        self
    }

    /// Use segmented-control visual style.
    pub fn segmented_style(mut self) -> Self {
        self.group_style = GroupStyle::Segmented;
        self
    }

    /// Override spacing between buttons.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let selected_idx = self.selected;
        let group_style = self.group_style;

        let mut r = row![].spacing(self.spacing);

        for (i, (label, msg)) in self.items.into_iter().enumerate() {
            let is_active = i == selected_idx;

            let style_fn =
                move |theme: &Theme, status: iced::widget::button::Status| match group_style {
                    GroupStyle::Tab => {
                        if is_active {
                            style::button::tab_active(theme, status)
                        } else {
                            style::button::tab_inactive(theme, status)
                        }
                    }
                    GroupStyle::Segmented => {
                        style::button::bordered_toggle(theme, status, is_active)
                    }
                };

            let btn = button(text(label).size(tokens::text::BODY))
                .padding([tokens::spacing::XS, tokens::spacing::MD])
                .style(style_fn)
                .on_press(msg);

            r = r.push(btn);
        }

        r.into()
    }
}

impl<'a, Message: Clone + 'a> From<ButtonGroupBuilder<'a, Message>> for Element<'a, Message> {
    fn from(builder: ButtonGroupBuilder<'a, Message>) -> Self {
        builder.into_element()
    }
}
