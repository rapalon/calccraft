//! Cooperative cancellation: a caller that runs a calculation on a thread of its own can stop
//! it from another thread, as the MCP server does when a call runs out of time. A thread can't
//! be killed, so the long loops ask: the formula evaluator (`engine::expression`) before every
//! operand, and the function analysis at every evaluation of f, f′ and f″, which then gives
//! NaN, so that the rest of the analysis ends at once. The caller throws the result away.
//!
//! The app never installs a flag: there each check reads an empty thread-local and changes
//! nothing.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    static FLAG: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// Runs `work` on this thread with `flag` as its cancellation flag: once another thread sets
/// it, `cancelled` is true inside `work`. The thread's previous flag comes back afterwards,
/// even if `work` panics.
pub fn with_flag<T>(flag: Arc<AtomicBool>, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Arc<AtomicBool>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            FLAG.with(|flag| *flag.borrow_mut() = previous);
        }
    }
    let _restore = Restore(FLAG.with(|current| current.replace(Some(flag))));
    work()
}

/// Whether the calculation running on this thread should stop.
pub fn cancelled() -> bool {
    FLAG.with(|flag| {
        flag.borrow()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Angle;
    use crate::engine::expression::{self, ExpressionError};
    use crate::graphing::Equation;
    use crate::graphing::analysis;
    use crate::graphing::parse::Syntax;
    use web_time::{Duration, Instant};

    #[test]
    fn without_a_flag_nothing_is_cancelled() {
        assert!(!cancelled());
        let flag = Arc::new(AtomicBool::new(false));
        with_flag(flag.clone(), || {
            assert!(!cancelled());
            flag.store(true, Ordering::Relaxed);
            assert!(cancelled());
        });
        assert!(!cancelled());
        assert_eq!(
            expression::evaluate("1/3", Angle::Degrees).unwrap().display,
            "0.33333333333333333333333333333333"
        );
    }

    #[test]
    fn a_cancelled_formula_stops() {
        let slow = vec!["asinh(1e-9999)"; 30].join("+");
        let flag = Arc::new(AtomicBool::new(true));
        let start = Instant::now();
        let result = with_flag(flag, || expression::evaluate(&slow, Angle::Degrees));
        assert_eq!(result, Err(ExpressionError::Cancelled));
        assert!(start.elapsed() < Duration::from_millis(50));
        // The same formula without the flag still has its value.
        assert_eq!(
            expression::evaluate("asinh(1e-9999)+asinh(1e-9999)", Angle::Degrees)
                .unwrap()
                .display,
            "2.e-9999"
        );
    }

    #[test]
    fn a_cancelled_analysis_stops() {
        let equation = Equation::parse("x^x^x^x^x^x^x^x", Syntax::default()).unwrap();
        let flag = Arc::new(AtomicBool::new(true));
        let start = Instant::now();
        let _ = with_flag(flag, || analysis::analyze(&equation, Angle::Radians));
        let cancelled = start.elapsed();
        let start = Instant::now();
        let full = analysis::analyze(&equation, Angle::Radians).unwrap();
        assert!(cancelled < start.elapsed(), "{cancelled:?}");
        assert_eq!(full.domain.as_deref(), Some("{x ≤ 0}"));
    }
}
