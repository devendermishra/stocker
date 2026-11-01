//! Indian mutual-fund purchase stamp duty (0.005% since 1 Jul 2020).

/// Stamp duty rate on MF purchases / SIP instalments / switch-ins.
pub const MF_PURCHASE_STAMP_DUTY_RATE: f64 = 0.00005; // 0.005%

/// Allotment after deducting stamp duty from the invested amount.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MfPurchaseAllotment {
    /// Stamp duty deducted from `amount` (rounded to paise).
    pub stamp_duty: f64,
    /// Amount applied toward units (`amount - stamp_duty`).
    pub investable_amount: f64,
    /// Units allotted at the published NAV.
    pub quantity: f64,
    /// Scheme NAV used for allotment.
    pub published_nav: f64,
    /// Effective purchase price = `amount / quantity` (NAV adjusted for stamp duty).
    pub adjusted_nav: f64,
}

/// Round to nearest paise (2 decimal places), matching typical RTA cash rounding.
pub fn round_paise(amount: f64) -> f64 {
    (amount * 100.0).round() / 100.0
}

/// Compute stamp duty, investable amount, units, and adjusted NAV for an MF purchase.
///
/// Formula (AMC / broker practice):
/// - `stamp_duty = round_paise(amount × 0.005%)`
/// - `investable = amount - stamp_duty`
/// - `quantity = investable / published_nav`
/// - `adjusted_nav = amount / quantity`
pub fn allot_mf_purchase(amount: f64, published_nav: f64) -> Option<MfPurchaseAllotment> {
    if !(amount > 0.0 && published_nav > 0.0) {
        return None;
    }
    let stamp_duty = round_paise(amount * MF_PURCHASE_STAMP_DUTY_RATE);
    let investable_amount = amount - stamp_duty;
    if investable_amount <= 0.0 {
        return None;
    }
    let quantity = investable_amount / published_nav;
    if quantity <= 0.0 {
        return None;
    }
    let adjusted_nav = amount / quantity;
    Some(MfPurchaseAllotment {
        stamp_duty,
        investable_amount,
        quantity,
        published_nav,
        adjusted_nav,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allot_sip_matches_zerodha_example() {
        // ₹10,000 SIP at NAV ₹10 → stamp ₹0.50, units 999.95
        let a = allot_mf_purchase(10_000.0, 10.0).unwrap();
        assert!((a.stamp_duty - 0.50).abs() < 1e-9);
        assert!((a.investable_amount - 9_999.50).abs() < 1e-9);
        assert!((a.quantity - 999.95).abs() < 1e-9);
        assert!((a.published_nav - 10.0).abs() < 1e-9);
        assert!((a.adjusted_nav - (10_000.0 / 999.95)).abs() < 1e-9);
        assert!(a.adjusted_nav > a.published_nav);
    }

    #[test]
    fn allot_rejects_non_positive() {
        assert!(allot_mf_purchase(0.0, 10.0).is_none());
        assert!(allot_mf_purchase(1000.0, 0.0).is_none());
    }
}
