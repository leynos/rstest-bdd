//! Example household ledger for demonstrating Japanese-language
//! behaviour-driven tests with rstest-bdd.
//!
//! The library mirrors the data structures exercised by the
//! `household_ledger.feature` scenarios. It exposes a mutable balance API.
//! Fixtures can share the ledger safely across multiple steps via interior
//! mutability.

use std::cell::Cell;

/// Tracks a household's running balance using interior mutability.
///
/// The ledger stores the balance in yen and supports both explicit assignments
/// and incremental adjustments. Using `Cell` keeps mutation ergonomic for step
/// definitions that borrow the ledger immutably.
///
/// # Examples
///
/// ```
/// use japanese_ledger::HouseholdLedger;
///
/// let ledger = HouseholdLedger::new();
/// ledger.set_balance(5);
/// ledger.apply_income(3);
/// ledger.apply_expense(4);
/// assert_eq!(ledger.balance(), 4);
/// ```
#[derive(Debug, Default)]
pub struct HouseholdLedger {
    balance: Cell<i32>,
}

impl HouseholdLedger {
    /// Creates a ledger whose balance starts at zero yen.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the current balance in yen.
    #[must_use]
    pub fn balance(&self) -> i32 {
        self.balance.get()
    }

    /// Replaces the stored balance with the provided amount.
    pub fn set_balance(&self, amount: i32) {
        self.balance.set(amount);
    }

    /// Records an incoming amount by increasing the balance.
    /// Saturates at `i32::MAX` when the addition would overflow.
    pub fn apply_income(&self, amount: i32) {
        self.adjust(amount);
    }

    /// Records an expense by decreasing the balance.
    /// Saturates at `i32::MIN` when the subtraction would underflow.
    pub fn apply_expense(&self, amount: i32) {
        self.adjust(-amount);
    }

    fn adjust(&self, delta: i32) {
        let current = self.balance.get();
        let updated = current.saturating_add(delta);

        // Saturate at the numeric bounds rather than wrapping around. Wrapping
        // would silently produce balances that misrepresent the ledger when a
        // scenario reaches `i32::MIN` or `i32::MAX`.
        self.balance.set(updated);
    }
}

#[cfg(test)]
mod tests {
    use super::HouseholdLedger;
    use rstest::{fixture, rstest};

    /// Represents a high-level ledger interaction used by the feature
    /// scenarios. Using a dedicated type keeps the scenario test inputs terse
    /// while still clarifying intent.
    #[derive(Clone, Copy)]
    enum LedgerStep {
        SetBalance(i32),
        Income(i32),
        Expense(i32),
    }

    impl LedgerStep {
        fn apply(self, ledger: &HouseholdLedger) {
            match self {
                Self::SetBalance(amount) => ledger.set_balance(amount),
                Self::Income(amount) => ledger.apply_income(amount),
                Self::Expense(amount) => ledger.apply_expense(amount),
            }
        }
    }

    #[fixture]
    fn ledger() -> HouseholdLedger {
        HouseholdLedger::new()
    }

    /// Replays the declarative ledger steps so each case mirrors a Gherkin
    /// scenario from the Japanese feature file.
    fn run_steps(ledger: &HouseholdLedger, steps: &[LedgerStep]) {
        for step in steps {
            step.apply(ledger);
        }
    }

    #[rstest]
    fn saturates_when_income_would_overflow(#[from(ledger)] ledger: HouseholdLedger) {
        ledger.set_balance(i32::MAX);

        ledger.apply_income(1);

        assert_eq!(ledger.balance(), i32::MAX);
    }

    #[rstest]
    fn saturates_when_expense_would_underflow(#[from(ledger)] ledger: HouseholdLedger) {
        ledger.set_balance(i32::MIN);

        ledger.apply_expense(1);

        assert_eq!(ledger.balance(), i32::MIN);
    }

    #[rstest]
    #[case::records_income(
        &[LedgerStep::Income(5), LedgerStep::Income(3)],
        8,
        None
    )]
    #[case::records_expense(
        &[
            LedgerStep::Income(10),
            LedgerStep::Expense(6),
        ],
        4,
        Some(5)
    )]
    #[case::saturates_income(
        &[
            LedgerStep::SetBalance(i32::MAX),
            LedgerStep::Income(1),
        ],
        i32::MAX,
        None
    )]
    #[case::saturates_expense(
        &[
            LedgerStep::SetBalance(i32::MIN),
            LedgerStep::Expense(1),
        ],
        i32::MIN,
        None
    )]
    fn follows_feature_scenarios(
        #[from(ledger)] ledger: HouseholdLedger,
        #[case] steps: &[LedgerStep],
        #[case] expected_balance: i32,
        #[case] forbidden_balance: Option<i32>,
    ) {
        run_steps(&ledger, steps);

        assert_eq!(ledger.balance(), expected_balance);
        if let Some(balance) = forbidden_balance {
            assert_ne!(ledger.balance(), balance);
        }
    }
}
