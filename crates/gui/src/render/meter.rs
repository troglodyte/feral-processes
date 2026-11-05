//! A text meter shared by screens that draw a fraction as filled cells.

/// Cells in a meter.
pub(super) const BAR_CELLS: usize = 5;

/// A meter of `BAR_CELLS` cells, `fraction` of them filled.
pub(super) fn bar(fraction: f32) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR_CELLS as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(BAR_CELLS - filled))
}
