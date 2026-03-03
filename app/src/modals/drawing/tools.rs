//! Drawing Tools UI
//!
//! Provides drawing tool selection with category-based organization.
//! Tools are grouped into sidebar groups; the sidebar shows one button
//! per group, with flyout submenus for multi-tool groups.

use crate::components::primitives::Icon;
use crate::drawing::DrawingTool;

/// Tool category for grouping related drawing tools (internal detail)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    Cursor,
    Lines,
    Levels,
    Fibonacci,
    Channels,
    Shapes,
    Annotations,
    Trading,
    Analysis,
    AiQuery,
}

impl ToolCategory {
    pub fn tools(&self) -> &'static [DrawingTool] {
        match self {
            ToolCategory::Cursor => &[DrawingTool::None],
            ToolCategory::Lines => &[
                DrawingTool::Line,
                DrawingTool::Ray,
                DrawingTool::ExtendedLine,
            ],
            ToolCategory::Levels => &[DrawingTool::HorizontalLine, DrawingTool::VerticalLine],
            ToolCategory::Fibonacci => &[DrawingTool::FibRetracement, DrawingTool::FibExtension],
            ToolCategory::Channels => &[DrawingTool::ParallelChannel],
            ToolCategory::Shapes => &[DrawingTool::Rectangle, DrawingTool::Ellipse],
            ToolCategory::Annotations => &[
                DrawingTool::TextLabel,
                DrawingTool::PriceLabel,
                DrawingTool::Arrow,
            ],
            ToolCategory::Trading => &[DrawingTool::BuyCalculator, DrawingTool::SellCalculator],
            ToolCategory::Analysis => &[DrawingTool::VolumeProfile, DrawingTool::DeltaProfile],
            ToolCategory::AiQuery => &[DrawingTool::AiContext],
        }
    }
}

// ── Sidebar groups ────────────────────────────────────────────────────

/// Consolidated sidebar groups shown as category buttons.
/// Each group maps to one or more `ToolCategory` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarGroup {
    /// Direct select/pan (no submenu)
    Select,
    /// Lines + Levels
    Lines,
    /// Fibonacci tools
    Fibonacci,
    /// Shapes + Channels
    Shapes,
    /// Annotations + Measurement
    Annotate,
    /// Trading calculators
    Trading,
    /// Analysis tools (Volume Profile)
    Analysis,
    /// AI context query
    AiQuery,
}

impl SidebarGroup {
    pub const ALL: &'static [SidebarGroup] = &[
        SidebarGroup::Select,
        SidebarGroup::Lines,
        SidebarGroup::Fibonacci,
        SidebarGroup::Shapes,
        SidebarGroup::Annotate,
        SidebarGroup::Trading,
        SidebarGroup::Analysis,
        SidebarGroup::AiQuery,
    ];

    /// All tools in this group (flat list).
    pub fn tools(&self) -> Vec<DrawingTool> {
        self.tool_sections()
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect()
    }

    /// Tools separated into sections (for rendering separators).
    /// Each inner slice is one visual section in the flyout.
    pub fn tool_sections(&self) -> Vec<&'static [DrawingTool]> {
        match self {
            SidebarGroup::Select => {
                vec![ToolCategory::Cursor.tools()]
            }
            SidebarGroup::Lines => {
                vec![ToolCategory::Lines.tools(), ToolCategory::Levels.tools()]
            }
            SidebarGroup::Fibonacci => {
                vec![ToolCategory::Fibonacci.tools()]
            }
            SidebarGroup::Shapes => {
                vec![ToolCategory::Shapes.tools(), ToolCategory::Channels.tools()]
            }
            SidebarGroup::Annotate => {
                vec![ToolCategory::Annotations.tools()]
            }
            SidebarGroup::Trading => {
                vec![ToolCategory::Trading.tools()]
            }
            SidebarGroup::Analysis => {
                vec![ToolCategory::Analysis.tools()]
            }
            SidebarGroup::AiQuery => {
                vec![ToolCategory::AiQuery.tools()]
            }
        }
    }

    /// Whether this group opens a flyout submenu (vs direct click).
    pub fn has_submenu(&self) -> bool {
        !matches!(self, SidebarGroup::Select | SidebarGroup::AiQuery)
    }

    /// Check if a tool belongs to this group.
    pub fn contains(&self, tool: DrawingTool) -> bool {
        self.tools().contains(&tool)
    }

    /// Find which group a tool belongs to.
    pub fn for_tool(tool: DrawingTool) -> SidebarGroup {
        for group in Self::ALL {
            if group.contains(tool) {
                return *group;
            }
        }
        SidebarGroup::Select
    }

    /// Default tool for this group.
    pub fn default_tool(&self) -> DrawingTool {
        match self {
            SidebarGroup::Select => DrawingTool::None,
            SidebarGroup::Lines => DrawingTool::Line,
            SidebarGroup::Fibonacci => DrawingTool::FibRetracement,
            SidebarGroup::Shapes => DrawingTool::Rectangle,
            SidebarGroup::Annotate => DrawingTool::TextLabel,
            SidebarGroup::Trading => DrawingTool::BuyCalculator,
            SidebarGroup::Analysis => DrawingTool::VolumeProfile,
            SidebarGroup::AiQuery => DrawingTool::AiContext,
        }
    }

    /// Tooltip label for the sidebar button.
    pub fn label(&self) -> &'static str {
        match self {
            SidebarGroup::Select => "Select",
            SidebarGroup::Lines => "Lines",
            SidebarGroup::Fibonacci => "Fibonacci",
            SidebarGroup::Shapes => "Shapes",
            SidebarGroup::Annotate => "Annotate",
            SidebarGroup::Trading => "Trading",
            SidebarGroup::Analysis => "Analysis",
            SidebarGroup::AiQuery => "AI Context",
        }
    }

    /// Icon for the sidebar button (uses the group's default tool icon).
    pub fn icon(&self, selected_tool: DrawingTool) -> Icon {
        tool_icon(selected_tool)
    }

    /// Index in `SidebarGroup::ALL` (for the per-group array).
    fn index(&self) -> usize {
        SidebarGroup::ALL
            .iter()
            .position(|g| g == self)
            .unwrap_or(0)
    }
}

/// Human-readable label for a drawing tool (used in flyout items).
pub fn tool_label(tool: DrawingTool) -> &'static str {
    match tool {
        DrawingTool::None => "Select",
        DrawingTool::Line => "Line",
        DrawingTool::Ray => "Ray",
        DrawingTool::ExtendedLine => "Extended Line",
        DrawingTool::HorizontalLine => "Horizontal Line",
        DrawingTool::VerticalLine => "Vertical Line",
        DrawingTool::FibRetracement => "Fib Retracement",
        DrawingTool::FibExtension => "Fib Extension",
        DrawingTool::ParallelChannel => "Parallel Channel",
        DrawingTool::Rectangle => "Rectangle",
        DrawingTool::Ellipse => "Ellipse",
        DrawingTool::TextLabel => "Text",
        DrawingTool::PriceLabel => "Price Label",
        DrawingTool::Arrow => "Arrow",
        DrawingTool::BuyCalculator => "Buy Calculator",
        DrawingTool::SellCalculator => "Sell Calculator",
        DrawingTool::VolumeProfile => "Volume Profile",
        DrawingTool::DeltaProfile => "Delta Profile",
        DrawingTool::AiContext => "AI Context",
    }
}

// ── Messages & Actions ────────────────────────────────────────────────

/// Message type for drawing tools
#[derive(Debug, Clone, Copy)]
pub enum Message {
    /// A drawing tool was selected
    ToolSelected(DrawingTool),
    /// Toggle snap mode
    ToggleSnap,
    /// Expand/collapse a sidebar group flyout
    ExpandGroup(Option<SidebarGroup>),
    /// Category button clicked — select its tool and toggle the flyout
    GroupClicked {
        tool: DrawingTool,
        group: SidebarGroup,
    },
}

/// Action returned from the panel
#[derive(Debug, Clone, Copy)]
pub enum Action {
    /// User selected a drawing tool
    SelectTool(DrawingTool),
    /// Toggle snap mode
    ToggleSnap,
}

// ── State ─────────────────────────────────────────────────────────────

/// Number of sidebar groups (must match `SidebarGroup::ALL.len()`).
const GROUP_COUNT: usize = SidebarGroup::ALL.len();

/// State for the drawing tools panel
#[derive(Debug, Clone)]
pub struct DrawingToolsPanel {
    /// Currently active tool
    pub active_tool: DrawingTool,
    /// Whether snap is enabled
    pub snap_enabled: bool,
    /// Last selected tool per sidebar group (indexed by `SidebarGroup::index()`)
    selected_per_group: [DrawingTool; GROUP_COUNT],
    /// Currently expanded flyout group (if any)
    pub expanded_group: Option<SidebarGroup>,
}

impl Default for DrawingToolsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawingToolsPanel {
    pub fn new() -> Self {
        let mut selected = [DrawingTool::None; GROUP_COUNT];
        for group in SidebarGroup::ALL {
            selected[group.index()] = group.default_tool();
        }
        Self {
            active_tool: DrawingTool::None,
            snap_enabled: true,
            selected_per_group: selected,
            expanded_group: None,
        }
    }

    pub fn set_active_tool(&mut self, tool: DrawingTool) {
        self.active_tool = tool;
        let group = SidebarGroup::for_tool(tool);
        self.selected_per_group[group.index()] = tool;
    }

    pub fn get_selected_for_group(&self, group: SidebarGroup) -> DrawingTool {
        self.selected_per_group[group.index()]
    }

    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::ToolSelected(tool) => {
                self.set_active_tool(tool);
                self.expanded_group = None;
                Some(Action::SelectTool(tool))
            }
            Message::ToggleSnap => {
                self.snap_enabled = !self.snap_enabled;
                Some(Action::ToggleSnap)
            }
            Message::ExpandGroup(group) => {
                self.expanded_group = group;
                None
            }
            Message::GroupClicked { tool, group } => {
                self.set_active_tool(tool);
                if self.expanded_group == Some(group) {
                    self.expanded_group = None;
                } else {
                    self.expanded_group = Some(group);
                }
                Some(Action::SelectTool(tool))
            }
        }
    }
}

/// Get the icon for a drawing tool.
/// Each tool maps to a unique glyph in the custom icon font (E820-E831).
pub fn tool_icon(tool: DrawingTool) -> Icon {
    match tool {
        DrawingTool::None => Icon::DrawCursor,               // E820
        DrawingTool::Line => Icon::DrawLine,                 // E821
        DrawingTool::Ray => Icon::DrawRay,                   // E822
        DrawingTool::ExtendedLine => Icon::DrawExtendedLine, // E823
        DrawingTool::HorizontalLine => Icon::DrawHLine,      // E824
        DrawingTool::VerticalLine => Icon::DrawVLine,        // E825
        DrawingTool::FibRetracement => Icon::DrawFibRetracement, // E828
        DrawingTool::FibExtension => Icon::DrawFibExtension, // E829
        DrawingTool::ParallelChannel => Icon::DrawChannel,   // E82A
        DrawingTool::Rectangle => Icon::DrawRectangle,       // E82B
        DrawingTool::Ellipse => Icon::DrawEllipse,           // E82C
        DrawingTool::TextLabel => Icon::DrawText,            // E82C
        DrawingTool::PriceLabel => Icon::DrawPriceLabel,     // E82D
        DrawingTool::Arrow => Icon::DrawArrow,               // E82E
        DrawingTool::BuyCalculator => Icon::DrawBuyCalc,     // E826 trending-up
        DrawingTool::SellCalculator => Icon::DrawSellCalc,   // E835 trending-down
        DrawingTool::VolumeProfile => Icon::DrawVolumeProfile, // E836 align-left
        DrawingTool::DeltaProfile => Icon::DrawDeltaProfile, // E836 align-left (delta)
        DrawingTool::AiContext => Icon::MessageSquare,       // E837
    }
}
