use crate::drawing::DrawingId;
use iced::Point;

/// What was right-clicked on the chart
#[derive(Debug, Clone)]
pub enum ContextMenuKind {
    /// Right-clicked empty chart area
    Chart { position: Point },
    /// Right-clicked a specific drawing
    Drawing {
        position: Point,
        id: DrawingId,
        locked: bool,
    },
    /// Right-clicked a study overlay label
    StudyOverlay { position: Point, study_index: usize },
    /// Right-clicked an AI assistant message
    AiMessage {
        position: Point,
        message_index: usize,
    },
}

impl ContextMenuKind {
    pub fn position(&self) -> Point {
        match self {
            ContextMenuKind::Chart { position }
            | ContextMenuKind::Drawing { position, .. }
            | ContextMenuKind::StudyOverlay { position, .. }
            | ContextMenuKind::AiMessage { position, .. } => *position,
        }
    }
}

/// Actions available from chart context menu
#[derive(Debug, Clone)]
pub enum ContextMenuAction {
    RebuildChart,
    CenterLastPrice,
    OpenIndicators,
    DeleteDrawing(DrawingId),
    ToggleLockDrawing(DrawingId),
    CloneDrawing(DrawingId),
    OpenDrawingProperties(DrawingId),
    OpenStudyProperties(usize),
    CopyAiMessageText(usize),
}
