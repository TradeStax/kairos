use crate::{
    component::primitives::AZERET_MONO,
    modal::{self, ModifierKind},
    style::{self, tokens},
    widget::button_with_tooltip,
};
use data::{ChartBasis, LinkGroup};
use iced::{
    Element,
    widget::{button, column, container, pane_grid, row, text, tooltip},
};

use super::Message;

pub fn link_group_modal<'a>(
    pane: pane_grid::Pane,
    selected_group: Option<LinkGroup>,
) -> Element<'a, Message> {
    let mut grid = column![].spacing(tokens::spacing::XS);
    let rows = LinkGroup::ALL.chunks(3);

    for row_groups in rows {
        let mut button_row = row![].spacing(tokens::spacing::XS);

        for &group in row_groups {
            let is_selected = selected_group == Some(group);
            let btn_content = text(group.to_string()).font(AZERET_MONO);

            let btn = if is_selected {
                button_with_tooltip(
                    btn_content.align_x(iced::Alignment::Center),
                    Message::SwitchLinkGroup(pane, None),
                    Some("Unlink"),
                    tooltip::Position::Bottom,
                    move |theme, status| style::button::menu_body(theme, status, true),
                )
            } else {
                button(btn_content.align_x(iced::Alignment::Center))
                    .on_press(Message::SwitchLinkGroup(pane, Some(group)))
                    .style(move |theme, status| style::button::menu_body(theme, status, false))
                    .into()
            };

            button_row = button_row.push(btn);
        }

        grid = grid.push(button_row);
    }

    container(grid)
        .max_width(240)
        .padding(tokens::spacing::XL)
        .style(style::chart_modal)
        .into()
}

pub fn basis_modifier<'a>(
    id: pane_grid::Pane,
    selected_basis: ChartBasis,
    modifier: Option<modal::stream::Modifier>,
    kind: ModifierKind,
) -> Element<'a, Message> {
    use super::Event;
    use crate::modal::pane::Modal;

    let modifier_modal = Modal::StreamModifier(
        modal::stream::Modifier::new(kind).with_view_mode(modal::stream::ViewMode::BasisSelection),
    );

    let is_active =
        modifier.is_some_and(|m| m.view_mode == modal::stream::ViewMode::BasisSelection);

    button(text(selected_basis.to_string()))
        .style(move |theme, status| style::button::modifier(theme, status, !is_active))
        .on_press(Message::PaneEvent(id, Event::ShowModal(modifier_modal)))
        .into()
}
