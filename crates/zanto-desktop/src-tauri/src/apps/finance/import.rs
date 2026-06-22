//! Statement import + money parsing for the finance app: amount coercion, header
//! mapping, per-row mapping to `add_transaction` args, the import-dedupe hash, and
//! the one-time legacy-row backfill. Kept out of `mod.rs` so the parsing/import
//! concern is isolated from aggregation and the store glue.

use serde_json::{json, Value};
use zanto_core::data::{DataStore, Query};
use super::{DEFAULT_ACCOUNT, STORE};

/// Coerce a model-supplied amount into a number. Weak models often send the
/// amount as a string ("12.50", "$12.50", "5-"); `as_f64()` alone would silently
/// treat those as 0, so parse a numeric value out of strings too.
pub(super) fn coerce_amount(v: Option<&Value>) -> f64 {
    match v {
        Some(v) if v.is_number() => v.as_f64().unwrap_or(0.0),
        Some(v) => v.as_str().map(parse_money).unwrap_or(0.0),
        None => 0.0,
    }
}

/// Parse a money string into a signed f64. Handles a leading sign, accounting
/// negatives — a TRAILING minus (`"5-"`, common in bank exports) or parentheses
/// (`"(5)"`) — plus currency symbols and thousands separators. Returns 0.0 only
/// for genuinely non-numeric input; the import path reports those rows as errors
/// (review H2) rather than silently dropping a debit that parsed to zero.
fn parse_money(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    let negative_paren = t.starts_with('(') && t.ends_with(')');
    let negative_trailing = t.ends_with('-');
    let negative_leading = t.starts_with('-');
    // Keep only the magnitude digits + decimal point (drops $, commas, spaces, signs).
    let digits: String = t.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    if digits.is_empty() || digits == "." {
        return 0.0;
    }
    let magnitude: f64 = match digits.parse() {
        Ok(v) => v,
        Err(_) => return 0.0,
    };
    if negative_paren || negative_trailing || negative_leading {
        -magnitude
    } else {
        magnitude
    }
}

/// Heuristic column mapping for a statement's headers: best-effort match of
/// date / merchant / category, and either debit+credit or a single amount.
pub(crate) fn suggest_mapping(headers: &[String]) -> Value {
    let find = |keys: &[&str]| -> Option<String> {
        headers
            .iter()
            .find(|h| {
                let hl = h.to_lowercase();
                keys.iter().any(|k| hl.contains(k))
            })
            .cloned()
    };
    let mut m = serde_json::Map::new();
    if let Some(h) = find(&["date"]) {
        m.insert("date".into(), json!(h));
    }
    if let Some(h) = find(&["description", "merchant", "payee", "name", "details", "narration"]) {
        m.insert("merchant".into(), json!(h));
    }
    if let Some(h) = find(&["category"]) {
        m.insert("category".into(), json!(h));
    }
    let debit = find(&["debit", "withdrawal", "paid out", "money out"]);
    let credit = find(&["credit", "deposit", "paid in", "money in"]);
    if debit.is_some() || credit.is_some() {
        if let Some(d) = debit {
            m.insert("debit".into(), json!(d));
        }
        if let Some(c) = credit {
            m.insert("credit".into(), json!(c));
        }
    } else if let Some(h) = find(&["amount", "value", "total"]) {
        m.insert("amount".into(), json!(h));
    }
    Value::Object(m)
}

/// Map one statement row to `add_transaction` args using `mapping` (field →
/// header name). Returns None when there's no usable amount (debit/credit/amount).
pub(super) fn import_row_to_args(headers: &[String], row: &[String], mapping: &Value, account: &str) -> Option<Value> {
    let col = |key: &str| -> Option<&str> {
        let header = mapping.get(key).and_then(|v| v.as_str())?;
        let idx = headers.iter().position(|h| h.eq_ignore_ascii_case(header))?;
        row.get(idx).map(|s| s.as_str())
    };
    let to_amt = |s: &str| coerce_amount(Some(&json!(s)));

    let (kind, amount) = if let Some(a) = col("amount").filter(|s| !s.trim().is_empty()) {
        let v = to_amt(a);
        if v == 0.0 {
            return None;
        }
        (if v < 0.0 { "expense" } else { "income" }, v.abs())
    } else {
        let debit = col("debit").map(to_amt).map(f64::abs).unwrap_or(0.0);
        let credit = col("credit").map(to_amt).map(f64::abs).unwrap_or(0.0);
        if debit > 0.0 {
            ("expense", debit)
        } else if credit > 0.0 {
            ("income", credit)
        } else {
            return None;
        }
    };

    let mut args = json!({
        "type": kind, "amount": amount, "account": account, "source": "import",
        "merchant": col("merchant").unwrap_or(""), "date": col("date").unwrap_or(""),
    });
    if let Some(c) = col("category").filter(|s| !s.trim().is_empty()) {
        args["category"] = json!(c);
    }
    Some(args)
}

/// A stable identity hash for import dedupe, over date + amount (2dp) + merchant
/// (case-insensitive) + account. `DefaultHasher::new()` uses fixed keys →
/// deterministic across runs, which is all dedupe needs.
pub(super) fn import_hash(date: &str, amount: f64, merchant: &str, account: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{date}|{amount:.2}|{}|{}", merchant.to_lowercase(), account.to_lowercase()).hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Pure legacy backfill: given a stored transaction, return an updated copy when
/// it predates the explicit money model (missing/empty `type`) or accounts
/// (missing/empty `account`), else None. Only absent fields are stamped — existing
/// values are never overwritten.
pub(super) fn legacy_backfill(rec: &Value) -> Option<Value> {
    let obj = rec.as_object()?;
    let has = |k: &str| obj.get(k).and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
    let (needs_type, needs_account) = (!has("type"), !has("account"));
    if !needs_type && !needs_account {
        return None;
    }
    let mut out = obj.clone();
    if needs_type {
        out.insert("type".into(), json!("expense"));
    }
    if needs_account {
        out.insert("account".into(), json!(DEFAULT_ACCOUNT));
    }
    Some(Value::Object(out))
}

/// One-time backfill over the transactions store (review C2). Idempotent: only
/// rows actually missing an explicit `type`/`account` are rewritten, so a second
/// run is a no-op. Returns the number of rows migrated.
pub(super) fn migrate_legacy_transactions(data: &DataStore) -> Result<u64, String> {
    let rows = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
    let mut migrated = 0u64;
    for r in rows {
        if let Some(updated) = legacy_backfill(&r.data) {
            data.update(STORE, r.id, &updated).map_err(|e| e.to_string())?;
            migrated += 1;
        }
    }
    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerce_amount_handles_number_and_string() {
        assert_eq!(coerce_amount(Some(&json!(12.5))), 12.5);
        assert_eq!(coerce_amount(Some(&json!("12.50"))), 12.5);
        assert_eq!(coerce_amount(Some(&json!("$1,299"))), 1299.0);
        assert_eq!(coerce_amount(Some(&json!("-8"))), -8.0);
        assert_eq!(coerce_amount(Some(&json!(null))), 0.0);
        assert_eq!(coerce_amount(None), 0.0);
        // B3-2: accounting negatives — trailing minus and parentheses — must not
        // parse to 0 (which would silently drop a debit).
        assert_eq!(coerce_amount(Some(&json!("5-"))), -5.0);
        assert_eq!(coerce_amount(Some(&json!("1,234.50-"))), -1234.5);
        assert_eq!(coerce_amount(Some(&json!("(5)"))), -5.0);
        assert_eq!(coerce_amount(Some(&json!("$ 42.00"))), 42.0);
        // Genuinely non-numeric still reads 0 (reported as an import error upstream).
        assert_eq!(coerce_amount(Some(&json!("n/a"))), 0.0);
    }

    #[test]
    fn import_maps_debit_credit_and_signed_amount() {
        let headers = vec!["Date".into(), "Description".into(), "Debit".into(), "Credit".into()];
        let mapping = json!({ "date": "Date", "merchant": "Description", "debit": "Debit", "credit": "Credit" });
        let expense = import_row_to_args(&headers, &vec!["2026-06-01".into(), "Cafe".into(), "12.50".into(), "".into()], &mapping, "Checking").unwrap();
        assert_eq!(expense["type"], json!("expense"));
        assert_eq!(expense["amount"], json!(12.5));
        assert_eq!(expense["account"], json!("Checking"));
        let income = import_row_to_args(&headers, &vec!["2026-06-02".into(), "Payroll".into(), "".into(), "2000".into()], &mapping, "Checking").unwrap();
        assert_eq!(income["type"], json!("income"));
        assert_eq!(income["amount"], json!(2000.0));

        // A single signed amount column: negative = expense.
        let h2 = vec!["Date".into(), "Amount".into()];
        let m2 = json!({ "date": "Date", "amount": "Amount" });
        let signed = import_row_to_args(&h2, &vec!["2026-06-03".into(), "-5".into()], &m2, "X").unwrap();
        assert_eq!(signed["type"], json!("expense"));
        assert_eq!(signed["amount"], json!(5.0));

        // A row with no amount is skipped.
        assert!(import_row_to_args(&headers, &vec!["2026-06-04".into(), "x".into(), "".into(), "".into()], &mapping, "X").is_none());
    }

    #[test]
    fn suggest_mapping_detects_columns() {
        let m = suggest_mapping(&["Transaction Date".into(), "Details".into(), "Debit".into(), "Credit".into()]);
        assert_eq!(m["date"], json!("Transaction Date"));
        assert_eq!(m["merchant"], json!("Details"));
        assert_eq!(m["debit"], json!("Debit"));
        assert_eq!(m["credit"], json!("Credit"));
    }

    #[test]
    fn import_hash_stable_case_insensitive_and_account_scoped() {
        let a = import_hash("2026-06-01", 12.5, "Cafe", "Checking");
        assert_eq!(a, import_hash("2026-06-01", 12.50, "CAFE", "checking")); // same row → same hash
        assert_ne!(a, import_hash("2026-06-02", 12.5, "Cafe", "Checking")); // different date
        assert_ne!(a, import_hash("2026-06-01", 12.5, "Cafe", "Savings")); // same row, different account
    }

    #[test]
    fn coerce_amount_parses_currency_strings() {
        // R-5: dollar-sign prefix must parse to the numeric value, not 0.
        assert_eq!(coerce_amount(Some(&json!("$12.50"))), 12.50);
        assert_eq!(coerce_amount(Some(&json!("12.50"))), 12.50);
        assert_eq!(coerce_amount(Some(&json!(12.50))), 12.50);
        assert_eq!(coerce_amount(None), 0.0);
        // Genuinely non-numeric → 0 (not a silent wrong value).
        assert_eq!(coerce_amount(Some(&json!("garbage"))), 0.0);
    }

    #[test]
    fn legacy_backfill_stamps_missing_type_and_account() {
        // Legacy row (no type/account) → explicit expense + Cash.
        let out = legacy_backfill(&json!({ "amount": 10, "date": "2026-01-01" })).unwrap();
        assert_eq!(out["type"], json!("expense"));
        assert_eq!(out["account"], json!(DEFAULT_ACCOUNT));
        // A fully-explicit row is left alone (None = no rewrite → idempotent).
        assert!(legacy_backfill(&json!({ "type": "income", "amount": 5, "account": "Bank" })).is_none());
        // Partial: keeps the existing type, stamps only the missing account.
        let p = legacy_backfill(&json!({ "type": "transfer", "amount": 1 })).unwrap();
        assert_eq!(p["type"], json!("transfer"));
        assert_eq!(p["account"], json!(DEFAULT_ACCOUNT));
    }
}
