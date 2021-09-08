pub(crate) trait Percentage {
    // Using an extension trait for this is maybe a bit excessive, but making it
    // a method has the nice advantage of making it *really* obvious which is
    // the total and which is the amount.
    fn percent_of(self, total: Self) -> Self;
}

impl Percentage for usize {
    fn percent_of(self, total: Self) -> Self {
        percentage(total as f64, self as f64) as Self
    }
}

impl Percentage for u64 {
    fn percent_of(self, total: Self) -> Self {
        percentage(total as f64, self as f64) as Self
    }
}

pub(crate) fn percentage(total: f64, amount: f64) -> f64 {
    debug_assert!(
        total >= amount,
        "assertion failed: total >= amount; total={}, amount={}",
        total,
        amount
    );
    (amount / total) * 100.0
}
