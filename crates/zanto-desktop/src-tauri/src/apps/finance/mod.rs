//! Personal Finance — the first micro-app. Full-stack: this Rust backend (stores,
//! deterministic flows, agent tools, component decls) + a Svelte frontend slice.
//! Aggregation is deterministic Rust (never the LLM).

use std::collections::HashMap;
use std::sync::Arc;
use serde_json::{json, Value};
use zanto_core::chat::{AppResult, GenaiTool, Target};
use zanto_core::data::{DataStore, Filter, FilterOp, Query};
use zanto_core::session::{format_ts_display, unix_now_pub};
use crate::app::{App, AppManifest, ComponentDecl, StartAction};

const STORE: &str = "transactions";

pub struct FinanceApp {
    manifest: AppManifest,
}

impl FinanceApp {
    pub fn new() -> Arc<dyn App> {
        let manifest = AppManifest {
            id: "finance".to_string(),
            name: "Personal Finance".to_string(),
            description: "Track expenses and view spending summaries.".to_string(),
            stores: vec![STORE.to_string()],
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
        let amount = args.get("amount").cloned().unwrap_or(json!(0));
        let merchant = args.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("uncategorized").to_string();
        let date = args
            .get("date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(today);

        let record = json!({ "date": date, "amount": amount, "merchant": merchant, "category": category });
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
        let rows: Vec<Value> = data
            .query(STORE, &q)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| r.data)
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
        let mut total = 0.0_f64;
        let mut by_cat: HashMap<String, f64> = HashMap::new();
        for r in &all {
            let date = r.data.get("date").and_then(|v| v.as_str()).unwrap_or("");
            if !date.starts_with(&month) {
                continue;
            }
            let amt = r.data.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            total += amt;
            let cat = r.data.get("category").and_then(|v| v.as_str()).unwrap_or("uncategorized").to_string();
            *by_cat.entry(cat).or_insert(0.0) += amt;
        }
        let by_category: Vec<Value> =
            by_cat.into_iter().map(|(c, t)| json!({ "category": c, "total": t })).collect();
        Ok(json!({ "month": month, "total": total, "by_category": by_category }))
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
        let mut balance = 0.0_f64;
        let mut month_total = 0.0_f64;
        let mut by_cat: HashMap<String, f64> = HashMap::new();
        // Per-month spend, keyed by YYYY-MM.
        let mut by_month: HashMap<String, f64> = HashMap::new();

        for r in &all {
            let amt = r.data.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let date = r.data.get("date").and_then(|v| v.as_str()).unwrap_or("");
            balance += amt;
            if date.len() >= 7 {
                *by_month.entry(date[..7].to_string()).or_insert(0.0) += amt;
            }
            if date.starts_with(&this_month) {
                month_total += amt;
                let cat = r.data.get("category").and_then(|v| v.as_str()).unwrap_or("uncategorized").to_string();
                *by_cat.entry(cat).or_insert(0.0) += amt;
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
            "month_total": month_total,
            "transaction_count": all.len(),
            "top_categories": top_categories,
            "series": { "labels": months, "data": series },
        }))
    }
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
         (defaults to inline)."
            .to_string()
    }

    fn agent_tools(&self) -> Vec<GenaiTool> {
        vec![
            GenaiTool::new("add_transaction")
                .with_description("Record a transaction in the user's finances.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "amount": { "type": "number", "description": "Amount spent" },
                        "merchant": { "type": "string" },
                        "category": { "type": "string" },
                        "date": { "type": "string", "description": "YYYY-MM-DD; defaults to today" }
                    },
                    "required": ["amount", "merchant"]
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
            other => Err(format!("unknown query: {other}")),
        }
    }

    fn action(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "add_transaction" => self.do_add_transaction(data, args),
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
