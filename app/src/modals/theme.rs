use iced::{
    Element,
    widget::{column, container, pick_list, text_input::default},
};

use crate::{
    components::input::color_picker::color_picker,
    components::overlay::modal_header::ModalHeaderBuilder,
    components::primitives::Icon,
    style::{self, tokens},
};
use palette::Hsva;

#[derive(Debug, Clone, PartialEq)]
pub enum Component {
    Background,
    Text,
    Primary,
    Success,
    Danger,
    Warning,
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Component {
    const ALL: [Self; 6] = [
        Self::Background,
        Self::Text,
        Self::Primary,
        Self::Success,
        Self::Danger,
        Self::Warning,
    ];
}

#[derive(Debug, Clone)]
pub enum Message {
    ComponentChanged(Component),
    CloseRequested,
    Color(Hsva),
    HexInput(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    UpdateTheme(iced_core::Theme),
    Exit,
}

pub struct ThemeEditor {
    pub custom_theme: Option<iced_core::Theme>,
    component: Component,
    hex_input: Option<String>,
    editing: Option<Hsva>,
}

impl ThemeEditor {
    pub fn new(custom_theme: Option<crate::config::Theme>) -> Self {
        Self {
            custom_theme: custom_theme.map(|t| crate::style::theme::theme_to_iced(&t)),
            component: Component::Background,
            hex_input: None,
            editing: None,
        }
    }

    fn focused_color(&self, theme: &iced_core::Theme) -> iced_core::Color {
        let palette = theme.palette();
        match self.component {
            Component::Background => palette.background,
            Component::Text => palette.text,
            Component::Primary => palette.primary,
            Component::Success => palette.success,
            Component::Danger => palette.danger,
            Component::Warning => palette.warning,
        }
    }

    pub fn update(&mut self, message: Message, theme: &iced_core::Theme) -> Option<Action> {
        match message {
            Message::Color(hsva) => {
                self.hex_input = None;
                self.editing = Some(hsva);

                let mut new_palette = theme.palette();
                let rgba = crate::config::theme::hsva_to_rgba(hsva);
                let color = crate::style::theme::rgba_to_iced_color(rgba);

                match self.component {
                    Component::Background => new_palette.background = color,
                    Component::Text => new_palette.text = color,
                    Component::Primary => new_palette.primary = color,
                    Component::Success => new_palette.success = color,
                    Component::Danger => new_palette.danger = color,
                    Component::Warning => new_palette.warning = color,
                }

                let new_theme = iced_core::Theme::custom("Custom".to_string(), new_palette);
                self.custom_theme = Some(new_theme.clone());

                Some(Action::UpdateTheme(new_theme))
            }
            Message::ComponentChanged(component) => {
                self.component = component;
                let color = self.focused_color(theme);
                self.editing = Some(crate::config::theme::rgba_to_hsva(
                    crate::style::theme::iced_color_to_rgba(color),
                ));
                None
            }
            Message::HexInput(input) => {
                let mut action = None;

                if let Some(rgba) = crate::config::theme::hex_to_rgba_safe(&input) {
                    let color = crate::style::theme::rgba_to_iced_color(rgba);
                    let mut new_palette = theme.palette();

                    match self.component {
                        Component::Background => new_palette.background = color,
                        Component::Text => new_palette.text = color,
                        Component::Primary => new_palette.primary = color,
                        Component::Success => new_palette.success = color,
                        Component::Danger => new_palette.danger = color,
                        Component::Warning => new_palette.warning = color,
                    }

                    self.editing = Some(crate::config::theme::rgba_to_hsva(rgba));

                    let new_theme = iced_core::Theme::custom("Custom".to_string(), new_palette);
                    self.custom_theme = Some(new_theme.clone());

                    action = Some(Action::UpdateTheme(new_theme));
                }

                self.hex_input = Some(input);
                action
            }
            Message::CloseRequested => Some(Action::Exit),
        }
    }

    pub fn view(&self, theme: &iced_core::Theme) -> Element<'_, Message> {
        let color = self.focused_color(theme);
        let hsva_in = self.editing.unwrap_or_else(|| {
            crate::config::theme::rgba_to_hsva(crate::style::theme::iced_color_to_rgba(color))
        });

        let is_input_valid = self.hex_input.is_none()
            || self
                .hex_input
                .as_deref()
                .and_then(crate::config::theme::hex_to_rgba_safe)
                .is_some();

        let hex_input = iced::widget::text_input(
            "",
            self.hex_input.as_deref().unwrap_or(
                crate::config::theme::rgba_to_hex_string(crate::style::theme::iced_color_to_rgba(
                    color,
                ))
                .as_str(),
            ),
        )
        .on_input(Message::HexInput)
        .width(80)
        .style(move |theme: &iced::Theme, status| {
            let palette = theme.extended_palette();

            iced::widget::text_input::Style {
                border: iced::Border {
                    color: if is_input_valid {
                        palette.background.strong.color
                    } else {
                        palette.danger.base.color
                    },
                    width: tokens::border::THIN,
                    radius: tokens::radius::SM.into(),
                },
                ..default(theme, status)
            }
        });

        let focused_field = pick_list(
            Component::ALL.to_vec(),
            Some(&self.component),
            Message::ComponentChanged,
        );

        let header = ModalHeaderBuilder::new("Theme")
            .close_icon(Icon::Return)
            .push_control(hex_input)
            .push_control(focused_field)
            .on_close(Message::CloseRequested);

        let body = container(color_picker(hsva_in, Message::Color, 280.0)).padding(iced::Padding {
            top: tokens::spacing::MD,
            right: tokens::spacing::XXL,
            bottom: tokens::spacing::XXL,
            left: tokens::spacing::XXL,
        });

        let content = column![header, body];

        container(content)
            .max_width(380)
            .style(style::dashboard_modal)
            .into()
    }
}
