//! Personal Finance — the first micro-app. Full-stack: this Rust backend (stores,
//! deterministic flows, agent tools, component decls) + a Svelte frontend slice.
//! Aggregation is deterministic Rust (never the LLM).

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{json, Value};
use zanto_core::chat::{AppResult, GenaiTool, Target};
use zanto_core::data::{DataStore, Filter, FilterOp, Query, Record};
use zanto_core::session::{format_ts_display, unix_now_pub};
use crate::app::{App, AppManifest, ComponentDecl, StartAction};

const STORE: &str = "transactions";
const PROFILE_STORE: &str = "finance_profile";
const WIDGETS_STORE: &str = "finance_widgets";

/// Default expense categories seeded when a profile omits them.
const DEFAULT_CATEGORIES: &[&str] =
    &["groceries", "dining", "transport", "utilities", "rent", "entertainment", "other"];

pub struct FinanceApp {
    manifest: AppManifest,
}

impl FinanceApp {
    pub fn new() -> Arc<dyn App> {
        let manifest = AppManifest {
            id: "finance".to_string(),
            name: "Personal Finance".to_string(),
            description: "Track expenses and view spending summaries.".to_string(),
            stores: vec![STORE.to_string(), PROFILE_STORE.to_string(), WIDGETS_STORE.to_string()],
            components: vec![
                ComponentDecl {
                    id: "transactions_table".to_string(),
                    schema: json!({
                        "type": "object",
                        "properties": { "rows": { "type": "array" } }
                    }),
                },
                ComponentDecl {
                    id: "monthly_summary".to_string(),
                    schema: json!({
                        "type": "object",
                        "properties": {
                            "month": { "type": "string" },
                            "total": { "type": "number" },
                            "by_category": { "type": "array" }
                        }
                    }),
                },
            ],
            start_actions: vec![
                StartAction { label: "Add a transaction".into(), prompt: "Add a transaction".into() },
                StartAction { label: "This month's summary".into(), prompt: "Show me this month's spending summary".into() },
                StartAction { label: "Recent transactions".into(), prompt: "Show my recent transactions".into() },
                StartAction { label: "Set a budget".into(), prompt: "Help me set a monthly budget".into() },
                // F6 — canned multi-step workflows. Each prompt asks the agent to run a
                // sequence of finance tools (≥2 tool calls), so the C6 workflow view groups them.
                StartAction {
                    label: "Import & categorize a statement".into(),
                    prompt: "Import a bank statement: for each line item I give you, record it with \
                             add_transaction (inferring a sensible category), then call \
                             query_transactions to show the imported rows and monthly_summary for the \
                             affected month so I can review the categorization.".into(),
                },
                StartAction {
                    label: "Monthly review".into(),
                    prompt: "Run my monthly review: call monthly_summary for the current month, then \
                             query_transactions for that month to list the underlying transactions, \
                             and finish with a short written takeaway of where my money went.".into(),
                },
            ],
        };
        Arc::new(FinanceApp { manifest })
    }

    fn ensure_store(&self, data: &DataStore) -> Result<(), String> {
        data.create_store(STORE).map_err(|e| e.to_string())
    }

    // ---- deterministic flows (shared by agentic + manual paths) ----

    fn do_add_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        // Amount is stored POSITIVE; the sign is carried by `type` so aggregation
        // is unambiguous regardless of how the model phrased the value.
        let kind = txn_kind_str(args.get("type"));
        let amount = coerce_amount(args.get("amount")).abs();
        let merchant = args.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("uncategorized").to_string();
        let note = args.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let date = args
            .get("date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(today);

        let record = json!({
            "type": kind, "date": date, "amount": amount,
            "merchant": merchant, "category": category, "note": note, "source": "manual",
        });
        let id = data.insert(STORE, &record).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "added", "id": id, "record": record }))
    }

    fn compute_transactions(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let mut q = Query::default();
        if let Some(cat) = args.get("category").and_then(|v| v.as_str()) {
            q.filters.push(Filter { field: "category".into(), op: FilterOp::Eq, value: json!(cat) });
        }
        if let Some(month) = args.get("month").and_then(|v| v.as_str()) {
            q.filters.push(Filter { field: "date".into(), op: FilterOp::Contains, value: json!(month) });
        }
        // Include each row's `id` so the UI can edit/delete a specific transaction.
        let rows: Vec<Value> = data
            .query(STORE, &q)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| {
                let mut o = r.data;
                o["id"] = json!(r.id);
                o
            })
            .collect();
        Ok(json!({ "rows": rows }))
    }

    fn compute_monthly_summary(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let month = args
            .get("month")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| today()[..7].to_string()); // YYYY-MM

        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        let mut income = 0.0_f64;
        let mut total = 0.0_f64; // expenses
        let mut by_cat: HashMap<String, f64> = HashMap::new();
        for r in &all {
            let t = normalize_txn(&r.data);
            if !t.date.starts_with(&month) {
                continue;
            }
            match t.kind {
                TxnKind::Income => income += t.amount,
                TxnKind::Expense => {
                    total += t.amount;
                    *by_cat.entry(t.category).or_insert(0.0) += t.amount;
                }
            }
        }
        let by_category: Vec<Value> =
            by_cat.into_iter().map(|(c, t)| json!({ "category": c, "total": t })).collect();
        Ok(json!({ "month": month, "income": income, "total": total, "net": income - total, "by_category": by_category }))
    }

    /// Dashboard overview: lifetime balance, this-month spend, top categories
    /// (this month), and a 6-month spend series. `empty: true` when no
    /// transactions exist. All aggregation is deterministic Rust.
    fn compute_overview(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        if all.is_empty() {
            return Ok(json!({ "empty": true }));
        }

        let this_month = today()[..7].to_string(); // YYYY-MM
        let balance = lifetime_balance(&all); // lifetime income − expense
        let mut income = 0.0_f64; // this month
        let mut month_total = 0.0_f64; // this month expense
        let mut uncategorized_count: u64 = 0; // lifetime uncategorized expenses (review queue)
        let mut by_cat: HashMap<String, f64> = HashMap::new(); // this month, expenses
        // Per-month EXPENSE, keyed by YYYY-MM (the 6-month spend series).
        let mut by_month: HashMap<String, f64> = HashMap::new();

        for r in &all {
            let t = normalize_txn(&r.data);
            if matches!(t.kind, TxnKind::Expense) {
                if t.date.len() >= 7 {
                    *by_month.entry(t.date[..7].to_string()).or_insert(0.0) += t.amount;
                }
                if t.category == "uncategorized" {
                    uncategorized_count += 1;
                }
            }
            if t.date.starts_with(&this_month) {
                match t.kind {
                    TxnKind::Income => income += t.amount,
                    TxnKind::Expense => {
                        month_total += t.amount;
                        *by_cat.entry(t.category).or_insert(0.0) += t.amount;
                    }
                }
            }
        }

        // Top categories this month, descending by total.
        let mut top: Vec<(String, f64)> = by_cat.into_iter().collect();
        top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_categories: Vec<Value> = top
            .into_iter()
            .take(5)
            .map(|(c, t)| json!({ "category": c, "total": t }))
            .collect();

        // Last 6 months (oldest → newest) ending at the current month.
        let months = last_n_months(&this_month, 6);
        let series: Vec<f64> = months.iter().map(|m| *by_month.get(m).unwrap_or(&0.0)).collect();

        Ok(json!({
            "empty": false,
            "balance": balance,
            "month": this_month,
            "income": income,
            "month_total": month_total,
            "net_cash_flow": income - month_total,
            "transaction_count": all.len(),
            "top_categories": top_categories,
            "uncategorized_count": uncategorized_count,
            "series": { "labels": months, "data": series },
        }))
    }

    // ---- onboarding / profile (F2) ----

    fn ensure_profile_store(&self, data: &DataStore) -> Result<(), String> {
        data.create_store(PROFILE_STORE).map_err(|e| e.to_string())
    }

    /// The saved onboarding profile, or `{ "setup": false }` when none exists.
    /// Picks the row with the greatest `saved_at` (last write wins) rather than
    /// relying on the store's scan order, which is not contractually defined.
    fn get_profile(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_profile_store(data)?;
        let rows = data.query(PROFILE_STORE, &Query::default()).map_err(|e| e.to_string())?;
        let latest = rows
            .into_iter()
            .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0));
        match latest {
            Some(r) => Ok(r.data),
            None => Ok(json!({ "setup": false })),
        }
    }

    /// Persist the onboarding profile. Idempotent at the read layer: each save stamps
    /// a `saved_at`, and `get_profile` returns the row with the greatest `saved_at`, so
    /// a re-run effectively overwrites the active profile (the DataStore API is
    /// insert-only — no in-place update). Categories default when omitted.
    fn do_save_profile(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_profile_store(data)?;

        let currency = args
            .get("currency")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("USD")
            .to_string();

        let monthly_income = args.get("monthly_income").and_then(|v| v.as_f64());

        let categories: Vec<String> = match args.get("categories").and_then(|v| v.as_array()) {
            Some(arr) => {
                let cats: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if cats.is_empty() {
                    DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect()
                } else {
                    cats
                }
            }
            None => DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect(),
        };

        let profile = json!({
            "setup": true,
            "currency": currency,
            "monthly_income": monthly_income,
            "categories": categories,
            "saved_at": unix_now_pub(),
        });

        data.insert(PROFILE_STORE, &profile).map_err(|e| e.to_string())?;
        Ok(profile)
    }

    // ---- dashboard widgets (F4) ----

    fn ensure_widgets_store(&self, data: &DataStore) -> Result<(), String> {
        data.create_store(WIDGETS_STORE).map_err(|e| e.to_string())
    }

    /// The saved dashboard widget list. Picks the row with the greatest
    /// `saved_at` (last write wins). Returns `{ widgets: [...] }`. When none has
    /// been saved, returns a sensible default layout mirroring the fixed F1
    /// dashboard (balance + this-month KPIs, the 6-month chart, top categories).
    /// A widget def = `{ kind, title, source }` where `source` selects part of
    /// the `overview` data.
    fn get_widgets(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_widgets_store(data)?;
        let rows = data.query(WIDGETS_STORE, &Query::default()).map_err(|e| e.to_string())?;
        let latest = rows
            .into_iter()
            .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0));
        match latest.and_then(|r| r.data.get("widgets").cloned()) {
            Some(widgets) => Ok(json!({ "widgets": widgets })),
            None => Ok(json!({ "widgets": default_widgets() })),
        }
    }

    /// Persist the dashboard widget list. Insert-only like the profile: each save
    /// stamps a `saved_at`, and `get_widgets` returns the row with the greatest
    /// `saved_at`, so a re-save overwrites the active layout. Only the recognized
    /// fields (`kind`, `title`, `source`) of each widget are kept.
    fn do_save_widgets(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_widgets_store(data)?;

        let widgets: Vec<Value> = match args.get("widgets").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|w| {
                    let kind = w.get("kind").and_then(|v| v.as_str())?;
                    if !matches!(kind, "kpi" | "chart" | "table") {
                        return None;
                    }
                    let source = w.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let title = w.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    Some(json!({ "kind": kind, "title": title, "source": source }))
                })
                .collect(),
            None => Vec::new(),
        };

        let record = json!({ "widgets": widgets, "saved_at": unix_now_pub() });
        data.insert(WIDGETS_STORE, &record).map_err(|e| e.to_string())?;
        Ok(record)
    }
}

/// Default dashboard widgets mirroring the fixed F1 layout. Each `source`
/// names a slice of the `overview` query result.
fn default_widgets() -> Value {
    json!([
        { "kind": "kpi", "title": "Balance", "source": "balance" },
        { "kind": "kpi", "title": "This month", "source": "month_total" },
        { "kind": "chart", "title": "Spending — last 6 months", "source": "series" },
        { "kind": "table", "title": "Top categories", "source": "top_categories" },
    ])
}

impl App for FinanceApp {
    fn manifest(&self) -> &AppManifest {
        &self.manifest
    }

    fn skill(&self) -> String {
        "You manage the user's personal finances. Transactions live in the `transactions` \
         store (fields: date, amount, merchant, category). To record a transaction call \
         add_transaction. To list transactions call query_transactions. For spending totals \
         call monthly_summary — never compute totals yourself. When the user asks to open, \
         show in a panel, or view a dashboard, pass target=\"canvas\"; otherwise omit it \
         (defaults to inline).\n\n\
         Inbuilt multi-step workflows — when the user asks for one of these, run the whole \
         tool sequence in a single turn (do not stop after the first tool):\n\
         - Import & categorize a statement: for each line item, call add_transaction with an \
         inferred category, then call query_transactions and monthly_summary for the affected \
         month so the user can review the result.\n\
         - Monthly review: call monthly_summary for the target month, then query_transactions \
         for that month, and finish with a short written takeaway."
            .to_string()
    }

    fn agent_tools(&self) -> Vec<GenaiTool> {
        vec![
            GenaiTool::new("add_transaction")
                .with_description("Record a transaction in the user's finances. Call this directly with the details — `amount` is required; merchant/category/date default if omitted.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "amount": { "type": "number", "description": "Amount spent (a number)" },
                        "merchant": { "type": "string" },
                        "category": { "type": "string" },
                        "date": { "type": "string", "description": "YYYY-MM-DD; defaults to today" }
                    },
                    "required": ["amount"]
                })),
            GenaiTool::new("query_transactions")
                .with_description("Show transactions, optionally filtered by category or month.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "category": { "type": "string" },
                        "month": { "type": "string", "description": "YYYY-MM" },
                        "target": { "type": "string", "enum": ["inline", "canvas"] }
                    }
                })),
            GenaiTool::new("monthly_summary")
                .with_description("Spending total and per-category breakdown for a month.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "month": { "type": "string", "description": "YYYY-MM; defaults to current month" },
                        "target": { "type": "string", "enum": ["inline", "canvas"] }
                    }
                })),
        ]
    }

    fn dispatch_tool(&self, data: &DataStore, name: &str, args: Value) -> Option<Result<AppResult, String>> {
        let target = target_of(&args);
        match name {
            "add_transaction" => Some(self.do_add_transaction(data, args).map(AppResult::Data)),
            "query_transactions" => Some(self.compute_transactions(data, args).map(|d| AppResult::Block {
                component_id: "transactions_table".to_string(),
                data: d,
                target,
            })),
            "monthly_summary" => Some(self.compute_monthly_summary(data, args).map(|d| AppResult::Block {
                component_id: "monthly_summary".to_string(),
                data: d,
                target,
            })),
            _ => None,
        }
    }

    fn query(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "list_transactions" => self.compute_transactions(data, args),
            "monthly_summary" => self.compute_monthly_summary(data, args),
            "overview" => self.compute_overview(data),
            "profile" => self.get_profile(data),
            "widgets" => self.get_widgets(data),
            other => Err(format!("unknown query: {other}")),
        }
    }

    fn action(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "add_transaction" => self.do_add_transaction(data, args),
            "save_profile" => self.do_save_profile(data, args),
            "save_widgets" => self.do_save_widgets(data, args),
            other => Err(format!("unknown action: {other}")),
        }
    }
}

fn target_of(args: &Value) -> Target {
    match args.get("target").and_then(|v| v.as_str()) {
        Some("canvas") => Target::Canvas,
        _ => Target::Inline,
    }
}

/// Today's date as `YYYY-MM-DD` (from the core's display formatter).
fn today() -> String {
    format_ts_display(unix_now_pub())[..10].to_string()
}

/// Coerce a model-supplied amount into a number. Weak models often send the
/// amount as a string ("12.50", "$12.50", "12,50"); `as_f64()` alone would
/// silently treat those as 0, so parse a numeric value out of strings too.
fn coerce_amount(v: Option<&Value>) -> f64 {
    match v {
        Some(v) if v.is_number() => v.as_f64().unwrap_or(0.0),
        Some(v) => v
            .as_str()
            .map(|s| {
                let cleaned: String = s
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                    .collect();
                cleaned.parse::<f64>().unwrap_or(0.0)
            })
            .unwrap_or(0.0),
        None => 0.0,
    }
}

/// Income vs expense. Missing/unknown `type` defaults to Expense so legacy
/// transactions (pre-v2, no `type` field) still aggregate correctly.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum TxnKind {
    Income,
    Expense,
}

/// Normalize the `type` arg/field to a stored string ("income" | "expense").
fn txn_kind_str(v: Option<&Value>) -> &'static str {
    match v.and_then(|v| v.as_str()) {
        Some("income") => "income",
        _ => "expense",
    }
}

/// A normalized view of a stored transaction, defaulting legacy/missing fields.
struct Txn {
    kind: TxnKind,
    amount: f64, // always positive; sign comes from `kind`
    category: String,
    date: String,
}

fn normalize_txn(v: &Value) -> Txn {
    let kind = match v.get("type").and_then(|t| t.as_str()) {
        Some("income") => TxnKind::Income,
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

/// Lifetime balance = sum(income) − sum(expense) over normalized records.
fn lifetime_balance(records: &[Record]) -> f64 {
    records
        .iter()
        .map(|r| {
            let t = normalize_txn(&r.data);
            match t.kind {
                TxnKind::Income => t.amount,
                TxnKind::Expense => -t.amount,
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_is_income_minus_expense_with_legacy_default() {
        // Acceptance #1 + #5: income − expense, and a legacy row (no `type`)
        // counts as an expense.
        let recs = vec![
            Record { id: 1, data: json!({ "type": "income", "amount": 2000 }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "expense", "amount": 12.50 }), created_at: 0 },
            Record { id: 3, data: json!({ "amount": 8 }), created_at: 0 }, // legacy → expense
        ];
        assert_eq!(lifetime_balance(&recs), 2000.0 - 12.5 - 8.0);
    }

    #[test]
    fn normalize_defaults_and_signs() {
        let income = normalize_txn(&json!({ "type": "income", "amount": "-100" }));
        assert_eq!(income.kind, TxnKind::Income);
        assert_eq!(income.amount, 100.0); // abs: sign is carried by kind
        let legacy = normalize_txn(&json!({ "amount": 5, "date": "2026-06-01" }));
        assert_eq!(legacy.kind, TxnKind::Expense);
        assert_eq!(legacy.category, "uncategorized");
    }

    #[test]
    fn coerce_amount_handles_number_and_string() {
        assert_eq!(coerce_amount(Some(&json!(12.5))), 12.5);
        assert_eq!(coerce_amount(Some(&json!("12.50"))), 12.5);
        assert_eq!(coerce_amount(Some(&json!("$1,299"))), 1299.0);
        assert_eq!(coerce_amount(Some(&json!("-8"))), -8.0);
        assert_eq!(coerce_amount(Some(&json!(null))), 0.0);
        assert_eq!(coerce_amount(None), 0.0);
    }
}

/// The `n` months ending at `end` (inclusive), oldest → newest, as `YYYY-MM`.
fn last_n_months(end: &str, n: usize) -> Vec<String> {
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
