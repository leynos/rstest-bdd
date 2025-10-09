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
    pub fn apply_income(&self, amount: i32) {
        self.adjust(amount);
    }

    /// Records an expense by decreasing the balance.
    pub fn apply_expense(&self, amount: i32) {
        self.adjust(-amount);
    }

    fn adjust(&self, delta: i32) {
        let updated = self.balance.get() + delta;
        self.balance.set(updated);
    }
}
