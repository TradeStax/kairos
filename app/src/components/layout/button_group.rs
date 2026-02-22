use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length, Theme};

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
    fill_width: bool,
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
            fill_width: false,
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

    /// Make each button fill equal horizontal space.
    pub fn fill_width(mut self) -> Self {
        self.fill_width = true;
        self
    }

    pub fn into_element(self) -> Element<'a, Message> {
        let selected_idx = self.selected;
        let group_style = self.group_style;
        let fill = self.fill_width;

        let mut r = row![].spacing(self.spacing);
        if fill {
            r = r.width(Length::Fill);
        }

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

            let label_text = text(label)
                .size(tokens::text::BODY)
                .align_x(Alignment::Center);
            let label_el: Element<'a, Message> = if fill {
                label_text.width(Length::Fill).into()
            } else {
                label_text.into()
            };

            let mut btn = button(label_el)
                .padding([tokens::spacing::XS, tokens::spacing::LG])
                .style(style_fn)
                .on_press(msg);

            if fill {
                btn = btn.width(Length::Fill);
            }

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
