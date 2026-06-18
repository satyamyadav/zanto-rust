//! Deterministic finance aggregation: the pure functions that turn stored
//! transactions/accounts/budgets/goals into the numbers the dashboard shows
//! (balances, net worth, budget status, goal progress, forecast, trends,
//! recurring detection). No DataStore, no I/O — all unit-testable in isolation.

use std::collections::HashMap;
use serde_json::{json, Value};
use zanto_core::data::Record;
use super::import::coerce_amount;
use super::DEFAULT_ACCOUNT;

/// Income vs expense. Missing/unknown `type` defaults to Expense so legacy
/// transactions (pre-v2, no `type` field) still aggregate correctly.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum TxnKind {
    Income,
    Expense,
    /// A move between the user's own accounts — neutral to income/expense.
    Transfer,
}

/// Normalize the `type` arg/field to a stored string ("income" | "expense").
pub(super) fn txn_kind_str(v: Option<&Value>) -> &'static str {
    match v.and_then(|v| v.as_str()) {
        Some("income") => "income",
        _ => "expense",
    }
}

/// A normalized view of a stored transaction, defaulting legacy/missing fields.
pub(super) struct Txn {
    pub(super) kind: TxnKind,
    pub(super) amount: f64, // always positive; sign comes from `kind`
    pub(super) category: String,
    pub(super) date: String,
}

pub(super) fn normalize_txn(v: &Value) -> Txn {
    let kind = match v.get("type").and_then(|t| t.as_str()) {
        Some("income") => TxnKind::Income,
        Some("transfer") => TxnKind::Transfer,
        _ => TxnKind::Expense, // missing/expense/unknown → expense
    };
    let category = v
        .get("category")
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("uncategorized")
        .to_string();
    let date = v.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
    Txn { kind, amount: coerce_amount(v.get("amount")).abs(), category, date }
}

// Recurring-charge detection thresholds (named so they're tunable).
const RECUR_MIN_OCCURRENCES: usize = 3;
const RECUR_MIN_MONTHS: usize = 3;
const RECUR_MIN_GAP_DAYS: i64 = 25;
const RECUR_MAX_GAP_DAYS: i64 = 35;

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // Mar = 0 .. Feb = 11
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Parse a `YYYY-MM-DD` date into a day ordinal, or None if malformed.
fn date_ordinal(date: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Detect recurring expenses: same merchant + ~amount, ≥3 occurrences across ≥3
/// distinct months with a ~monthly median gap. Returns merchant/amount/count/
/// last_date/monthly_total, sorted by monthly_total desc.
pub(super) fn detect_recurring(records: &[Record], _now_month: &str) -> Vec<Value> {
    // (merchant_lower, amount rounded to whole unit) → [(ordinal, amount, date)]
    let mut groups: HashMap<(String, i64), Vec<(i64, f64, String)>> = HashMap::new();
    let mut display: HashMap<String, String> = HashMap::new();
    for r in records {
        let t = normalize_txn(&r.data);
        if !matches!(t.kind, TxnKind::Expense) {
            continue;
        }
        let merchant = r.data.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if merchant.trim().is_empty() {
            continue;
        }
        let ord = match date_ordinal(&t.date) {
            Some(o) => o,
            None => continue,
        };
        let mlow = merchant.to_lowercase();
        display.entry(mlow.clone()).or_insert_with(|| merchant.clone());
        groups.entry((mlow, t.amount.round() as i64)).or_default().push((ord, t.amount, t.date.clone()));
    }

    let mut out = Vec::new();
    for ((mlow, _), mut occ) in groups {
        if occ.len() < RECUR_MIN_OCCURRENCES {
            continue;
        }
        occ.sort_by_key(|(o, _, _)| *o);
        let months: std::collections::HashSet<&str> =
            occ.iter().map(|(_, _, d)| &d[..7.min(d.len())]).collect();
        if months.len() < RECUR_MIN_MONTHS {
            continue;
        }
        let mut gaps: Vec<i64> = occ.windows(2).map(|w| w[1].0 - w[0].0).collect();
        gaps.sort_unstable();
        let median = gaps[gaps.len() / 2];
        if !(RECUR_MIN_GAP_DAYS..=RECUR_MAX_GAP_DAYS).contains(&median) {
            continue;
        }
        let count = occ.len();
        let avg = occ.iter().map(|(_, a, _)| *a).sum::<f64>() / count as f64;
        let amount = (avg * 100.0).round() / 100.0;
        let last_date = occ.iter().map(|(_, _, d)| d.clone()).max().unwrap_or_default();
        let merchant = display.get(&mlow).cloned().unwrap_or(mlow);
        out.push(json!({
            "merchant": merchant, "amount": amount, "count": count,
            "last_date": last_date, "monthly_total": amount,
        }));
    }
    out.sort_by(|a, b| {
        b["monthly_total"].as_f64().unwrap_or(0.0)
            .partial_cmp(&a["monthly_total"].as_f64().unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Per-category expense series over the given months (top ~5 categories by total).
/// Returns `{ months, categories: [{ category, data: [..per month..] }] }`.
pub(super) fn compute_trends_data(records: &[Record], months: &[String]) -> Value {
    let mset: std::collections::HashSet<&str> = months.iter().map(|s| s.as_str()).collect();
    let mut by_mc: HashMap<(String, String), f64> = HashMap::new();
    let mut totals: HashMap<String, f64> = HashMap::new();
    for r in records {
        let t = normalize_txn(&r.data);
        if !matches!(t.kind, TxnKind::Expense) || t.date.len() < 7 {
            continue;
        }
        let m = &t.date[..7];
        if !mset.contains(m) {
            continue;
        }
        *by_mc.entry((m.to_string(), t.category.clone())).or_insert(0.0) += t.amount;
        *totals.entry(t.category).or_insert(0.0) += t.amount;
    }
    let mut tv: Vec<(String, f64)> = totals.into_iter().collect();
    tv.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let categories: Vec<Value> = tv
        .into_iter()
        .take(5)
        .map(|(c, _)| {
            let data: Vec<f64> =
                months.iter().map(|m| *by_mc.get(&(m.clone(), c.clone())).unwrap_or(&0.0)).collect();
            json!({ "category": c, "data": data })
        })
        .collect();
    json!({ "months": months, "categories": categories })
}

/// Rest-of-month run-rate forecast. `prev_months` are the complete months used
/// for the average baseline. Projects this month's end net worth by adding only
/// the *remaining* expected income/expense to the (month-to-date-inclusive) net
/// worth. Expected = max(month-to-date, 3-month average).
pub(super) fn compute_forecast_data(records: &[Record], net_worth: f64, this_month: &str, prev_months: &[String]) -> Value {
    let (mut mtd_exp, mut mtd_inc) = (0.0_f64, 0.0_f64);
    let mut by_month_exp: HashMap<String, f64> = HashMap::new();
    let mut by_month_inc: HashMap<String, f64> = HashMap::new();
    for r in records {
        let t = normalize_txn(&r.data);
        if t.date.len() < 7 {
            continue;
        }
        let m = t.date[..7].to_string();
        match t.kind {
            TxnKind::Expense => {
                if m == this_month {
                    mtd_exp += t.amount;
                }
                *by_month_exp.entry(m).or_insert(0.0) += t.amount;
            }
            TxnKind::Income => {
                if m == this_month {
                    mtd_inc += t.amount;
                }
                *by_month_inc.entry(m).or_insert(0.0) += t.amount;
            }
            TxnKind::Transfer => {}
        }
    }
    let avg = |map: &HashMap<String, f64>| -> f64 {
        if prev_months.is_empty() {
            return 0.0;
        }
        let sum: f64 = prev_months.iter().map(|m| *map.get(m).unwrap_or(&0.0)).sum();
        sum / prev_months.len() as f64
    };
    let avg_exp = avg(&by_month_exp);
    let avg_inc = avg(&by_month_inc);
    let expected_expense = mtd_exp.max(avg_exp);
    let expected_income = mtd_inc.max(avg_inc);
    let projected = net_worth + (expected_income - mtd_inc) - (expected_expense - mtd_exp);
    json!({
        "month": this_month,
        "projected_net_worth": projected,
        "expected_income": expected_income,
        "expected_expense": expected_expense,
        "avg_monthly_expense": avg_exp,
        "month_to_date_income": mtd_inc,
        "month_to_date_expense": mtd_exp,
    })
}

/// Goal progress derived from the linked account's balance. Savings: current vs
/// target; debt: amount still owed (a liability account's negative balance).
pub(super) fn compute_goal_status(goals: &[Value], accounts: &[Value]) -> Vec<Value> {
    let balance_of = |name: &str| -> f64 {
        accounts
            .iter()
            .find(|a| a.get("name").and_then(|v| v.as_str()) == Some(name))
            .and_then(|a| a.get("balance").and_then(|v| v.as_f64()))
            .unwrap_or(0.0)
    };
    goals
        .iter()
        .filter_map(|g| {
            let name = g.get("name").and_then(|v| v.as_str()).filter(|s| !s.is_empty())?.to_string();
            let kind = match g.get("kind").and_then(|v| v.as_str()) {
                Some("debt") => "debt",
                _ => "savings",
            };
            let account = g.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let target = g.get("target").and_then(|v| v.as_f64()).unwrap_or(0.0);
            // A goal whose account isn't present at all is "unlinked" — it must
            // never claim complete just because a missing balance reads as 0.
            let linked = accounts.iter().any(|a| a.get("name").and_then(|v| v.as_str()) == Some(account.as_str()));
            let b = balance_of(&account);
            let mut out = json!({
                "name": name, "kind": kind, "account": account, "target": target, "linked": linked,
                "target_date": g.get("target_date").cloned().unwrap_or(Value::Null),
            });
            if kind == "debt" {
                let owed = (-b).max(0.0);
                let progress = if target > 0.0 {
                    (1.0 - owed / target).clamp(0.0, 1.0)
                } else if owed == 0.0 {
                    1.0
                } else {
                    0.0
                };
                out["owed"] = json!(owed);
                out["progress"] = json!(progress);
                out["complete"] = json!(linked && owed <= 0.0);
            } else {
                let current = b.max(0.0);
                let progress = if target > 0.0 { (current / target).clamp(0.0, 1.0) } else { 0.0 };
                out["current"] = json!(current);
                out["progress"] = json!(progress);
                out["remaining"] = json!((target - current).max(0.0));
                out["complete"] = json!(linked && target > 0.0 && current >= target);
            }
            Some(out)
        })
        .collect()
}

/// The seeded default account list (one cash account).
pub(super) fn default_accounts() -> Value {
    json!([{ "name": DEFAULT_ACCOUNT, "type": "cash", "opening_balance": 0.0 }])
}

/// Per-account balances + net worth. Each account = opening_balance + income −
/// expense + transfers_in − transfers_out for that account. Transfers move money
/// between accounts but leave net worth unchanged. Returns (accounts, net_worth).
pub(super) fn compute_account_balances(accounts: &[Value], records: &[Record]) -> (Vec<Value>, f64) {
    let mut bal: HashMap<String, f64> = HashMap::new();
    let mut order: Vec<(String, String)> = Vec::new(); // declared (name, type) — preserve order
    let mut declared: std::collections::HashSet<String> = std::collections::HashSet::new();
    for a in accounts {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let typ = a.get("type").and_then(|v| v.as_str()).unwrap_or("cash").to_string();
        bal.insert(name.clone(), a.get("opening_balance").and_then(|v| v.as_f64()).unwrap_or(0.0));
        declared.insert(name.clone());
        order.push((name, typ));
    }
    // Apply every transaction to its account, creating a bucket for any account
    // a transaction references that ISN'T declared (renamed/deleted account) —
    // so money is never silently dropped from net worth.
    for r in records {
        let t = normalize_txn(&r.data);
        let acct = r.data.get("account").and_then(|v| v.as_str()).unwrap_or(DEFAULT_ACCOUNT).to_string();
        match t.kind {
            TxnKind::Income => *bal.entry(acct).or_insert(0.0) += t.amount,
            TxnKind::Expense => *bal.entry(acct).or_insert(0.0) -= t.amount,
            TxnKind::Transfer => {
                *bal.entry(acct).or_insert(0.0) -= t.amount;
                let to = r.data.get("to_account").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if !to.is_empty() {
                    *bal.entry(to).or_insert(0.0) += t.amount;
                }
            }
        }
    }
    let mut net = 0.0;
    let mut out: Vec<Value> = Vec::new();
    for (name, typ) in &order {
        let b = *bal.get(name).unwrap_or(&0.0);
        net += b;
        out.push(json!({ "name": name, "type": typ, "balance": b }));
    }
    // Orphaned accounts (referenced by transactions but not declared) surface as
    // "unlinked" rather than vanishing. Sorted for stable output.
    let mut orphans: Vec<(&String, &f64)> = bal.iter().filter(|(n, _)| !declared.contains(*n)).collect();
    orphans.sort_by(|a, b| a.0.cmp(b.0));
    for (name, b) in orphans {
        net += *b;
        out.push(json!({ "name": name, "type": "unlinked", "balance": *b }));
    }
    (out, net)
}

/// Budget vs actual for the current month. `f` is the fraction of the month
/// elapsed (day/days-in-month) used for a run-rate projection. Returns
/// (budget_status, over_budget, pace_warnings): `budget_status` is one row per
/// budgeted category (with `projected` + `on_track_to_exceed`); `over_budget` is
/// already over; `pace_warnings` is projected-to-exceed but not yet over.
pub(super) fn compute_budget_status(
    budgets: &[Value],
    spent_by_cat: &HashMap<String, f64>,
    f: f64,
) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let mut status = Vec::new();
    let mut over = Vec::new();
    let mut pace = Vec::new();
    for b in budgets {
        let category = b.get("category").and_then(|v| v.as_str()).unwrap_or("");
        let limit = b.get("limit").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if category.is_empty() || limit <= 0.0 {
            continue;
        }
        let spent = *spent_by_cat.get(category).unwrap_or(&0.0);
        let is_over = spent > limit;
        let projected = if f > 0.0 { spent / f } else { spent };
        let on_track = !is_over && projected > limit;
        status.push(json!({
            "category": category, "limit": limit, "spent": spent, "pct": spent / limit,
            "over": is_over, "projected": projected, "on_track_to_exceed": on_track,
        }));
        if is_over {
            over.push(json!({ "category": category, "limit": limit, "spent": spent, "by": spent - limit }));
        } else if on_track {
            pace.push(json!({ "category": category, "limit": limit, "spent": spent, "projected": projected }));
        }
    }
    (status, over, pace)
}

/// Days in a civil month (leap-year aware).
pub(super) fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Lifetime balance = sum(income) − sum(expense) over normalized records.
pub(super) fn lifetime_balance(records: &[Record]) -> f64 {
    records
        .iter()
        .map(|r| {
            let t = normalize_txn(&r.data);
            match t.kind {
                TxnKind::Income => t.amount,
                TxnKind::Expense => -t.amount,
                TxnKind::Transfer => 0.0, // internal move — neutral to balance
            }
        })
        .sum()
}

/// The `n` months ending at `end` (inclusive), oldest → newest, as `YYYY-MM`.
pub(super) fn last_n_months(end: &str, n: usize) -> Vec<String> {
    // Parse "YYYY-MM"; fall back to a single-element series on malformed input.
    let (year, month) = match (end.get(..4).and_then(|s| s.parse::<i32>().ok()), end.get(5..7).and_then(|s| s.parse::<u32>().ok())) {
        (Some(y), Some(m)) if (1..=12).contains(&m) => (y, m),
        _ => return vec![end.to_string()],
    };
    // Walk back from the end month.
    let mut out: Vec<String> = Vec::with_capacity(n);
    let mut y = year;
    let mut m = month as i32;
    for _ in 0..n {
        out.push(format!("{y:04}-{m:02}"));
        m -= 1;
        if m == 0 {
            m = 12;
            y -= 1;
        }
    }
    out.reverse();
    out
}

