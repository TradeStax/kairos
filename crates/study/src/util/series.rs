//! Series utilities for buffered study data.

/// Create a rolling window iterator over a slice.
///
/// Yields slices of length `window` by sliding one element at a time.
/// Returns an empty iterator if the data length is less than `window`.
pub fn rolling_window<T>(data: &[T], window: usize) -> impl Iterator<Item = &[T]> {
    data.windows(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_window() {
        let data = [1, 2, 3, 4, 5];
        let windows: Vec<&[i32]> = rolling_window(&data, 3).collect();
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0], &[1, 2, 3]);
        assert_eq!(windows[1], &[2, 3, 4]);
        assert_eq!(windows[2], &[3, 4, 5]);
    }

    #[test]
    fn test_rolling_window_too_short() {
        let data = [1, 2];
        let windows: Vec<&[i32]> = rolling_window(&data, 3).collect();
        assert!(windows.is_empty());
    }

    #[test]
    fn test_rolling_window_exact() {
        let data = [1, 2, 3];
        let windows: Vec<&[i32]> = rolling_window(&data, 3).collect();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0], &[1, 2, 3]);
    }
}
