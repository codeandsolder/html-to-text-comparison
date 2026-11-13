//! Conversion of size/count ratios into approximate presentation metrics.

/// Floating-point percentages have inherently approximate precision; `usize` may
/// exceed f64's exact-integer range on 64-bit hosts.
#[expect(
    clippy::cast_precision_loss,
    reason = "Only reporting approximate ratios as floating-point presentation metrics; counts remain integers"
)]
fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Percentage of a total count represented by a subset.
pub fn percent(part: usize, total: usize) -> f64 {
    ratio(part, total) * 100.0
}

/// Percentage reduction in size (may be negative for expanded output).
pub fn reduction_percent(output_size: usize, input_size: usize) -> f64 {
    if input_size == 0 {
        0.0
    } else {
        ratio(output_size, input_size).mul_add(-100.0, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{percent, reduction_percent};

    #[test]
    fn proportions_handle_zero_and_expansion() {
        assert_eq!(percent(0, 0), 0.0);
        assert_eq!(percent(25, 100), 25.0);
        assert_eq!(reduction_percent(0, 0), 0.0);
        assert_eq!(reduction_percent(50, 100), 50.0);
        assert_eq!(reduction_percent(200, 100), -100.0);
    }
}
