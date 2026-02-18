//! Drawing Tools UI
//!
//! Provides drawing tool selection with category-based organization.
//! Tools are grouped into categories, each showing the currently selected tool's icon.

use crate::component::primitives::{Icon, icon_text};
use crate::style::{self, tokens};
use data::DrawingTool;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, padding};

/// Tool category for grouping related drawing tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// Selection/pan mode
    Cursor,
    /// Line-based tools: Line, Ray, TrendLine
    Lines,
    /// Horizontal and vertical lines
    Levels,
    /// Shape tools: Rectangle
    Shapes,
}

impl ToolCategory {
    /// All categories in display order
    pub const ALL: &'static [ToolCategory] = &[
        ToolCategory::Cursor,
        ToolCategory::Lines,
        ToolCategory::Levels,
        ToolCategory::Shapes,
    ];

    /// Get tools in this category
    pub fn tools(&self) -> &'static [DrawingTool] {
        match self {
            ToolCategory::Cursor => &[DrawingTool::None],
            ToolCategory::Lines => &[DrawingTool::Line, DrawingTool::Ray, DrawingTool::TrendLine],
            ToolCategory::Levels => &[DrawingTool::HorizontalLine, DrawingTool::VerticalLine],
            ToolCategory::Shapes => &[DrawingTool::Rectangle],
        }
    }

    /// Get default tool for this category
    pub fn default_tool(&self) -> DrawingTool {
        self.tools()[0]
    }

    /// Check if a tool belongs to this category
    pub fn contains(&self, tool: DrawingTool) -> bool {
        self.tools().contains(&tool)
    }

    /// Get the category for a tool
    pub fn for_tool(tool: DrawingTool) -> ToolCategory {
        for cat in Self::ALL {
            if cat.contains(tool) {
                return *cat;
            }
        }
        ToolCategory::Cursor
    }
}

/// Message type for drawing tools
#[derive(Debug, Clone, Copy)]
pub enum Message {
    /// A drawing tool was selected
    ToolSelected(DrawingTool),
    /// Toggle snap mode
    ToggleSnap,
    /// Toggle a category dropdown open/closed
    ToggleCategory(ToolCategory),
    /// Close any open dropdown
    CloseDropdown,
}

/// Action returned from the panel
#[derive(Debug, Clone, Copy)]
pub enum Action {
    /// User selected a drawing tool
    SelectTool(DrawingTool),
    /// Toggle snap mode
    ToggleSnap,
}

/// State for the drawing tools panel
#[derive(Debug, Clone)]
pub struct DrawingToolsPanel {
    /// Currently active tool
    pub active_tool: DrawingTool,
    /// Whether snap is enabled
    pub snap_enabled: bool,
    /// Currently open category dropdown (if any)
    open_category: Option<ToolCategory>,
    /// Last selected tool per category (for showing the right icon)
    selected_per_category: [DrawingTool; 4],
}

impl Default for DrawingToolsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawingToolsPanel {
    pub fn new() -> Self {
        Self {
            active_tool: DrawingTool::None,
            snap_enabled: true,
            open_category: None,
            selected_per_category: [
                DrawingTool::None,           // Cursor
                DrawingTool::Line,           // Lines
                DrawingTool::HorizontalLine, // Levels
                DrawingTool::Rectangle,      // Shapes
            ],
        }
    }

    #[allow(dead_code)]
    pub fn with_active_tool(mut self, tool: DrawingTool) -> Self {
        self.set_active_tool(tool);
        self
    }

    #[allow(dead_code)]
    pub fn with_snap(mut self, enabled: bool) -> Self {
        self.snap_enabled = enabled;
        self
    }

    fn set_active_tool(&mut self, tool: DrawingTool) {
        self.active_tool = tool;
        // Update the selected tool for this category
        let category = ToolCategory::for_tool(tool);
        let idx = ToolCategory::ALL
            .iter()
            .position(|c| *c == category)
            .unwrap_or(0);
        self.selected_per_category[idx] = tool;
    }

    fn get_selected_for_category(&self, category: ToolCategory) -> DrawingTool {
        let idx = ToolCategory::ALL
            .iter()
            .position(|c| *c == category)
            .unwrap_or(0);
        self.selected_per_category[idx]
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::ToolSelected(tool) => {
                self.set_active_tool(tool);
                self.open_category = None;
                Some(Action::SelectTool(tool))
            }
            Message::ToggleSnap => {
                self.snap_enabled = !self.snap_enabled;
                Some(Action::ToggleSnap)
            }
            Message::ToggleCategory(category) => {
                // Toggle: if already open, close; otherwise open this category
                if self.open_category == Some(category) {
                    self.open_category = None;
                } else {
                    self.open_category = Some(category);
                }
                None
            }
            Message::CloseDropdown => {
                self.open_category = None;
                None
            }
        }
    }

    /// Check if a dropdown is currently open
    #[allow(dead_code)]
    pub fn has_open_dropdown(&self) -> bool {
        self.open_category.is_some()
    }

    /// Get the currently open category (if any)
    pub fn open_category(&self) -> Option<ToolCategory> {
        self.open_category
    }

    /// View the compact tool buttons for the sidebar
    pub fn view_sidebar_buttons(&self) -> Element<'_, Message> {
        let mut buttons: Vec<Element<'_, Message>> = Vec::new();

        for &category in ToolCategory::ALL {
            let selected_tool = self.get_selected_for_category(category);
            let is_active = ToolCategory::for_tool(self.active_tool) == category
                && self.active_tool != DrawingTool::None
                || (category == ToolCategory::Cursor && self.active_tool == DrawingTool::None);
            let is_open = self.open_category == Some(category);
            let has_multiple_tools = category.tools().len() > 1;

            let btn = self.category_button(
                category,
                selected_tool,
                is_active,
                is_open,
                has_multiple_tools,
            );
            buttons.push(btn);
        }

        // Add snap toggle at the bottom
        let snap_btn = self.snap_button();
        buttons.push(snap_btn);

        column(buttons)
            .spacing(tokens::spacing::XXS)
            .align_x(Alignment::Center)
            .into()
    }

    /// View the dropdown for a category (if open)
    pub fn view_dropdown(&self) -> Option<Element<'_, Message>> {
        let category = self.open_category?;
        let tools = category.tools();

        if tools.len() <= 1 {
            return None;
        }

        let tool_buttons: Vec<Element<'_, Message>> = tools
            .iter()
            .map(|&tool| {
                let is_selected = tool == self.active_tool;
                dropdown_tool_button(tool, is_selected)
            })
            .collect();

        let dropdown_content = column(tool_buttons).spacing(tokens::spacing::XXXS).padding(tokens::spacing::XS);

        let dropdown = container(dropdown_content).style(style::dropdown_container);

        Some(dropdown.into())
    }

    fn category_button(
        &self,
        category: ToolCategory,
        selected_tool: DrawingTool,
        is_active: bool,
        is_open: bool,
        has_multiple_tools: bool,
    ) -> Element<'_, Message> {
        let icon = tool_icon(selected_tool);
        let icon_el = icon_text(icon, 14).width(16).align_x(Alignment::Center);

        // Build button content - always show expand arrow for categories with multiple tools
        let content: Element<'_, Message> = if has_multiple_tools {
            row![
                icon_el,
                icon_text(Icon::ExpandRight, 8)
                    .width(8)
                    .align_x(Alignment::Center),
            ]
            .spacing(0)
            .align_y(Alignment::Center)
            .width(24)
            .into()
        } else {
            container(icon_el)
                .width(24)
                .align_x(Alignment::Center)
                .into()
        };

        let msg = if has_multiple_tools {
            Message::ToggleCategory(category)
        } else {
            Message::ToolSelected(category.default_tool())
        };

        button(content)
            .padding(padding::all(tokens::spacing::XS))
            .on_press(msg)
            .style(move |theme, status| {
                style::button::transparent(theme, status, is_active || is_open)
            })
            .into()
    }

    fn snap_button(&self) -> Element<'_, Message> {
        let icon = if self.snap_enabled {
            Icon::SnapOn
        } else {
            Icon::SnapOff
        };

        let content = icon_text(icon, 12).width(24).align_x(Alignment::Center);

        button(content)
            .padding(padding::all(tokens::spacing::XS))
            .on_press(Message::ToggleSnap)
            .style(move |theme, status| {
                style::button::transparent(theme, status, self.snap_enabled)
            })
            .into()
    }

}


/// Create a tool button for the dropdown
fn dropdown_tool_button(tool: DrawingTool, is_selected: bool) -> Element<'static, Message> {
    let icon = tool_icon(tool);
    let label = tool_label(tool);

    button(
        row![icon_text(icon, 12).width(16), text(label).size(tokens::text::SMALL),]
            .spacing(tokens::spacing::SM)
            .align_y(Alignment::Center)
            .padding(padding::left(tokens::spacing::XXS).right(tokens::spacing::XS)),
    )
    .padding(padding::all(tokens::spacing::XS))
    .width(Length::Fill)
    .on_press(Message::ToolSelected(tool))
    .style(move |theme, status| style::button::transparent(theme, status, is_selected))
    .into()
}

/// Get the icon for a drawing tool
pub fn tool_icon(tool: DrawingTool) -> Icon {
    match tool {
        DrawingTool::None => Icon::DrawCursor,
        DrawingTool::Line => Icon::DrawLine,
        DrawingTool::Ray => Icon::DrawRay,
        DrawingTool::HorizontalLine => Icon::DrawHLine,
        DrawingTool::VerticalLine => Icon::DrawVLine,
        DrawingTool::Rectangle => Icon::DrawRectangle,
        DrawingTool::TrendLine => Icon::DrawTrendLine,
    }
}

/// Get the short label for a drawing tool
fn tool_label(tool: DrawingTool) -> &'static str {
    match tool {
        DrawingTool::None => "Select",
        DrawingTool::Line => "Line",
        DrawingTool::Ray => "Ray",
        DrawingTool::HorizontalLine => "H-Line",
        DrawingTool::VerticalLine => "V-Line",
        DrawingTool::Rectangle => "Rect",
        DrawingTool::TrendLine => "Trend",
    }
}
