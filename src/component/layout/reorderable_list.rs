//! Thin re-export of the `column_drag` widget for use through the component
//! API.  All key types are brought into scope so callers only need to depend
//! on `crate::component::layout::reorderable_list`.

pub use crate::widget::column_drag::Column as ReorderableColumn;
pub use crate::widget::column_drag::DragEvent as ReorderableDragEvent;
pub use crate::widget::column_drag::reorder_vec;
