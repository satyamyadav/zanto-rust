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
const RULES_STORE: &str = "finance_category_rules";
const BUDGETS_STORE: &str = "finance_budgets";

/// Account a transaction belongs to when none is given (also the seeded account).
const DEFAULT_ACCOUNT: &str = "Cash";

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
            stores: vec![
                STORE.to_string(),
                PROFILE_STORE.to_string(),
                WIDGETS_STORE.to_string(),
                RULES_STORE.to_string(),
                BUDGETS_STORE.to_string(),
            ],
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
        // Enforce categories: keep a requested category only if it's in the
        // profile; else try merchant rules; else "uncategorized" (review queue).
        let category = self.resolve_category(data, &merchant, args.get("category").and_then(|v| v.as_str()));
        let note = args.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let date = args
            .get("date")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(today);
        let source = match args.get("source").and_then(|v| v.as_str()) {
            Some("import") => "import",
            _ => "manual",
        };
        // Which of the user's own accounts this belongs to (v0.4). Defaults to Cash.
        let account = args
            .get("account")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_ACCOUNT)
            .to_string();

        // Import dedupe: an imported row carries a stable hash of date+amount+
        // merchant; if one already exists, skip (re-running an import is a no-op).
        let import_h = (source == "import").then(|| import_hash(&date, amount, &merchant));
        if let Some(h) = &import_h {
            let mut q = Query::default();
            q.filters.push(Filter { field: "import_hash".into(), op: FilterOp::Eq, value: json!(h) });
            if !data.query(STORE, &q).map_err(|e| e.to_string())?.is_empty() {
                return Ok(json!({ "status": "duplicate_skipped", "import_hash": h }));
            }
        }

        let mut record = json!({
            "type": kind, "date": date, "amount": amount, "merchant": merchant,
            "category": category, "note": note, "source": source, "account": account,
        });
        if let Some(h) = import_h {
            record["import_hash"] = json!(h);
        }
        let id = data.insert(STORE, &record).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "added", "id": id, "record": record }))
    }

    /// Edit a transaction by id. Only the provided fields are changed; amount is
    /// re-coerced to a positive number, type is normalized.
    fn do_update_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "update_transaction requires an integer `id`".to_string())?;
        let mut rec = data
            .get(STORE, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no transaction with id {id}"))?
            .data;
        {
            let obj = rec
                .as_object_mut()
                .ok_or_else(|| "transaction record is not an object".to_string())?;
            if args.get("type").is_some() {
                obj.insert("type".into(), json!(txn_kind_str(args.get("type"))));
            }
            if args.get("amount").is_some() {
                obj.insert("amount".into(), json!(coerce_amount(args.get("amount")).abs()));
            }
            for field in ["merchant", "category", "date", "note"] {
                if let Some(s) = args.get(field).and_then(|v| v.as_str()) {
                    obj.insert(field.into(), json!(s));
                }
            }
        }
        // Re-resolve the category when merchant or category changed, so edits go
        // through the same enforcement as adds.
        if args.get("category").is_some() || args.get("merchant").is_some() {
            let merchant = rec.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let requested = rec.get("category").and_then(|v| v.as_str()).map(str::to_string);
            let resolved = self.resolve_category(data, &merchant, requested.as_deref());
            rec["category"] = json!(resolved);
        }
        data.update(STORE, id, &rec).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "updated", "id": id, "record": rec }))
    }

    /// Delete a transaction by id. Idempotent at the store layer.
    fn do_delete_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "delete_transaction requires an integer `id`".to_string())?;
        data.delete(STORE, id).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "deleted", "id": id }))
    }

    // ---- category enforcement + rules (v0.2) ----

    /// The profile's category list, or the defaults when none/empty — so category
    /// enforcement still works before onboarding.
    fn profile_categories(&self, data: &DataStore) -> Vec<String> {
        let cats: Vec<String> = self
            .get_profile(data)
            .ok()
            .and_then(|p| p.get("categories").cloned())
            .and_then(|c| c.as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()))
            .unwrap_or_default();
        if cats.is_empty() {
            DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect()
        } else {
            cats
        }
    }

    /// All saved merchant→category rules, each carrying its `id`.
    fn get_category_rules(&self, data: &DataStore) -> Result<Vec<Value>, String> {
        data.create_store(RULES_STORE).map_err(|e| e.to_string())?;
        let rows = data.query(RULES_STORE, &Query::default()).map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let mut o = r.data;
                o["id"] = json!(r.id);
                o
            })
            .collect())
    }

    /// Resolve a category: a requested one that's in the profile wins; else a
    /// matching merchant rule; else "uncategorized".
    fn resolve_category(&self, data: &DataStore, merchant: &str, requested: Option<&str>) -> String {
        let cats = self.profile_categories(data);
        let rules = self.get_category_rules(data).unwrap_or_default();
        resolve_category_pure(merchant, requested, &cats, &rules)
    }

    fn do_save_category_rule(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        data.create_store(RULES_STORE).map_err(|e| e.to_string())?;
        let merchant_contains = args
            .get("merchant_contains")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if merchant_contains.is_empty() || category.is_empty() {
            return Err("a rule needs a non-empty merchant_contains and category".to_string());
        }
        let record = json!({ "merchant_contains": merchant_contains, "category": category });
        let id = data.insert(RULES_STORE, &record).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "saved", "id": id, "record": record }))
    }

    fn do_delete_category_rule(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "delete_category_rule requires an integer `id`".to_string())?;
        data.delete(RULES_STORE, id).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "deleted", "id": id }))
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

        // Budget vs actual for this month (uses the per-category spend above).
        let (budget_status, over_budget) = compute_budget_status(&self.budgets_vec(data), &by_cat);

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

        // Month-over-month change in spend (this vs previous month).
        let n = series.len();
        let mom_delta = if n >= 2 { series[n - 1] - series[n - 2] } else { 0.0 };
        let mom_prev = if n >= 2 { series[n - 2] } else { 0.0 };
        let mom_pct = if mom_prev > 0.0 { mom_delta / mom_prev } else { 0.0 };

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
            "budget_status": budget_status,
            "over_budget": over_budget,
            "mom_delta": mom_delta,
            "mom_pct": mom_pct,
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

    /// Batch-import statement rows (already parsed + permission-checked in IPC).
    /// args: `{ headers: [..], rows: [[..]], mapping, account }`. Reuses
    /// `do_add_transaction` so category resolution + import dedupe both apply.
    fn do_import_transactions(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let headers: Vec<String> = args
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let rows = args.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let mapping = args.get("mapping").cloned().unwrap_or_else(|| json!({}));
        let account = args.get("account").and_then(|v| v.as_str()).unwrap_or(DEFAULT_ACCOUNT).to_string();

        let (mut inserted, mut skipped) = (0u64, 0u64);
        let mut errors = Vec::new();
        for (i, row_v) in rows.iter().enumerate() {
            let row: Vec<String> = row_v
                .as_array()
                .map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
                .unwrap_or_default();
            match import_row_to_args(&headers, &row, &mapping, &account) {
                Some(targs) => match self.do_add_transaction(data, targs) {
                    Ok(res) => {
                        if res.get("status").and_then(|s| s.as_str()) == Some("duplicate_skipped") {
                            skipped += 1;
                        } else {
                            inserted += 1;
                        }
                    }
                    Err(e) => errors.push(json!({ "row": i, "reason": e })),
                },
                None => errors.push(json!({ "row": i, "reason": "no amount — map a debit/credit or amount column" })),
            }
        }
        Ok(json!({ "inserted": inserted, "skipped": skipped, "errors": errors }))
    }

    /// Recurring/subscription detection over all transactions.
    fn compute_recurring(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        Ok(json!({ "items": detect_recurring(&all, &today()[..7]) }))
    }

    /// Per-category 6-month expense trends + overall month-over-month change.
    fn compute_trends(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        let months = last_n_months(&today()[..7], 6);
        let mut trends = compute_trends_data(&all, &months);

        // Overall MoM (this vs previous month).
        let mut by_month: HashMap<String, f64> = HashMap::new();
        for r in &all {
            let t = normalize_txn(&r.data);
            if matches!(t.kind, TxnKind::Expense) && t.date.len() >= 7 {
                *by_month.entry(t.date[..7].to_string()).or_insert(0.0) += t.amount;
            }
        }
        let this = *by_month.get(&months[months.len() - 1]).unwrap_or(&0.0);
        let last = if months.len() >= 2 { *by_month.get(&months[months.len() - 2]).unwrap_or(&0.0) } else { 0.0 };
        let mom_delta = this - last;
        let mom_pct = if last > 0.0 { mom_delta / last } else { 0.0 };
        if let Value::Object(o) = &mut trends {
            o.insert("mom_delta".into(), json!(mom_delta));
            o.insert("mom_pct".into(), json!(mom_pct));
        }
        Ok(trends)
    }

    // ---- budgets (v0.3) ----

    /// The latest saved per-category budgets, or an empty list. Insert-only,
    /// latest-wins by `saved_at` (mirrors widgets/profile).
    fn get_budgets(&self, data: &DataStore) -> Result<Value, String> {
        data.create_store(BUDGETS_STORE).map_err(|e| e.to_string())?;
        let rows = data.query(BUDGETS_STORE, &Query::default()).map_err(|e| e.to_string())?;
        let latest = rows
            .into_iter()
            .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0));
        match latest.and_then(|r| r.data.get("budgets").cloned()) {
            Some(budgets) => Ok(json!({ "budgets": budgets })),
            None => Ok(json!({ "budgets": [] })),
        }
    }

    /// The budget list as a plain array (for aggregation).
    fn budgets_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_budgets(data)
            .ok()
            .and_then(|b| b.get("budgets").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    /// Persist per-category budgets. Keeps only entries with a non-empty
    /// `category` and a positive `limit` (coerced from number/string).
    fn do_save_budgets(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        data.create_store(BUDGETS_STORE).map_err(|e| e.to_string())?;
        let budgets: Vec<Value> = match args.get("budgets").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|b| {
                    let category = b.get("category").and_then(|v| v.as_str())?.trim().to_string();
                    let limit = coerce_amount(b.get("limit")).abs();
                    if category.is_empty() || limit <= 0.0 {
                        return None;
                    }
                    Some(json!({ "category": category, "limit": limit }))
                })
                .collect(),
            None => Vec::new(),
        };
        let record = json!({ "budgets": budgets, "saved_at": unix_now_pub() });
        data.insert(BUDGETS_STORE, &record).map_err(|e| e.to_string())?;
        Ok(record)
    }
}

/// Default dashboard widgets mirroring the fixed F1 layout. Each `source`
/// names a slice of the `overview` query result.
fn default_widgets() -> Value {
    json!([
        { "kind": "kpi", "title": "Balance", "source": "balance" },
        { "kind": "kpi", "title": "This month", "source": "month_total" },
        { "kind": "kpi", "title": "Income", "source": "income" },
        { "kind": "kpi", "title": "Net", "source": "net_cash_flow" },
        { "kind": "chart", "title": "Spending — last 6 months", "source": "series" },
        { "kind": "table", "title": "Top categories", "source": "top_categories" },
        { "kind": "budget", "title": "Budget vs actual", "source": "budget_status" },
        { "kind": "subscriptions", "title": "Subscriptions", "source": "recurring" },
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
                .with_description("Record a transaction (expense or income) in the user's finances. Call this directly — `amount` is required; `type` defaults to expense; merchant/category/date default if omitted.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "enum": ["income", "expense"], "description": "Defaults to expense" },
                        "amount": { "type": "number", "description": "A positive number; the sign is set by type" },
                        "merchant": { "type": "string" },
                        "category": { "type": "string" },
                        "date": { "type": "string", "description": "YYYY-MM-DD; defaults to today" },
                        "note": { "type": "string" },
                        "source": { "type": "string", "enum": ["manual", "import"], "description": "Use 'import' for statement rows; duplicates are skipped" }
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
            GenaiTool::new("update_transaction")
                .with_description("Edit a recorded transaction by id. Pass `id` plus only the fields to change (type/amount/merchant/category/date/note).")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "type": { "type": "string", "enum": ["income", "expense"] },
                        "amount": { "type": "number" },
                        "merchant": { "type": "string" },
                        "category": { "type": "string" },
                        "date": { "type": "string" },
                        "note": { "type": "string" }
                    },
                    "required": ["id"]
                })),
            GenaiTool::new("delete_transaction")
                .with_description("Delete a recorded transaction by id.")
                .with_schema(json!({
                    "type": "object",
                    "properties": { "id": { "type": "integer" } },
                    "required": ["id"]
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
            "update_transaction" => Some(self.do_update_transaction(data, args).map(AppResult::Data)),
            "delete_transaction" => Some(self.do_delete_transaction(data, args).map(AppResult::Data)),
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
            "budgets" => self.get_budgets(data),
            "recurring" => self.compute_recurring(data),
            "trends" => self.compute_trends(data),
            "category_rules" => self.get_category_rules(data).map(|rules| json!({ "rules": rules })),
            other => Err(format!("unknown query: {other}")),
        }
    }

    fn action(&self, data: &DataStore, name: &str, args: Value) -> Result<Value, String> {
        match name {
            "add_transaction" => self.do_add_transaction(data, args),
            "update_transaction" => self.do_update_transaction(data, args),
            "delete_transaction" => self.do_delete_transaction(data, args),
            "save_profile" => self.do_save_profile(data, args),
            "save_widgets" => self.do_save_widgets(data, args),
            "save_category_rule" => self.do_save_category_rule(data, args),
            "delete_category_rule" => self.do_delete_category_rule(data, args),
            "save_budgets" => self.do_save_budgets(data, args),
            "import_transactions" => self.do_import_transactions(data, args),
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

/// Pure category resolution (no DataStore): a requested category that's in the
/// profile list wins (normalized to the profile's casing); else the first
/// merchant rule whose `merchant_contains` is a case-insensitive substring of the
/// merchant; else "uncategorized".
fn resolve_category_pure(
    merchant: &str,
    requested: Option<&str>,
    cats: &[String],
    rules: &[Value],
) -> String {
    if let Some(req) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some(found) = cats.iter().find(|c| c.eq_ignore_ascii_case(req)) {
            return found.clone();
        }
    }
    let ml = merchant.to_lowercase();
    for rule in rules {
        let sub = rule.get("merchant_contains").and_then(|v| v.as_str()).unwrap_or("");
        let cat = rule.get("category").and_then(|v| v.as_str()).unwrap_or("");
        if !sub.is_empty() && !cat.is_empty() && ml.contains(&sub.to_lowercase()) {
            return cat.to_string();
        }
    }
    "uncategorized".to_string()
}

/// Heuristic column mapping for a statement's headers: best-effort match of
/// date / merchant / category, and either debit+credit or a single amount.
pub fn suggest_mapping(headers: &[String]) -> Value {
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
fn import_row_to_args(headers: &[String], row: &[String], mapping: &Value, account: &str) -> Option<Value> {
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
fn detect_recurring(records: &[Record], _now_month: &str) -> Vec<Value> {
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
fn compute_trends_data(records: &[Record], months: &[String]) -> Value {
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

/// Budget vs actual for the current month. Returns (budget_status, over_budget):
/// `budget_status` is one row per budgeted category (spent defaults 0);
/// `over_budget` is the subset where spent exceeds the limit.
fn compute_budget_status(budgets: &[Value], spent_by_cat: &HashMap<String, f64>) -> (Vec<Value>, Vec<Value>) {
    let mut status = Vec::new();
    let mut over = Vec::new();
    for b in budgets {
        let category = b.get("category").and_then(|v| v.as_str()).unwrap_or("");
        let limit = b.get("limit").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if category.is_empty() || limit <= 0.0 {
            continue;
        }
        let spent = *spent_by_cat.get(category).unwrap_or(&0.0);
        let is_over = spent > limit;
        status.push(json!({
            "category": category, "limit": limit, "spent": spent,
            "pct": spent / limit, "over": is_over,
        }));
        if is_over {
            over.push(json!({ "category": category, "limit": limit, "spent": spent, "by": spent - limit }));
        }
    }
    (status, over)
}

/// A stable identity hash for import dedupe, over date + amount (2dp) + merchant
/// (case-insensitive). `DefaultHasher::new()` uses fixed keys → deterministic
/// across runs, which is all dedupe needs.
fn import_hash(date: &str, amount: f64, merchant: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{date}|{amount:.2}|{}", merchant.to_lowercase()).hash(&mut h);
    format!("{:016x}", h.finish())
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
    fn resolve_category_prefers_profile_then_rules_then_uncategorized() {
        let cats = vec!["dining".to_string(), "transport".to_string()];
        let rules = vec![json!({ "merchant_contains": "starbucks", "category": "dining" })];
        // requested category in the profile (case-insensitive) → profile casing
        assert_eq!(resolve_category_pure("x", Some("Dining"), &cats, &rules), "dining");
        // merchant matches a rule
        assert_eq!(resolve_category_pure("STARBUCKS #5", None, &cats, &rules), "dining");
        // requested not in profile + no rule → uncategorized
        assert_eq!(resolve_category_pure("Acme", Some("foobar"), &cats, &rules), "uncategorized");
    }

    #[test]
    fn trends_builds_per_category_expense_series() {
        let months = vec!["2026-05".to_string(), "2026-06".to_string()];
        let recs = vec![
            Record { id: 1, data: json!({ "type": "expense", "category": "dining", "amount": 10, "date": "2026-05-10" }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "expense", "category": "dining", "amount": 20, "date": "2026-06-10" }), created_at: 0 },
            Record { id: 3, data: json!({ "type": "income", "category": "salary", "amount": 1000, "date": "2026-06-01" }), created_at: 0 },
        ];
        let t = compute_trends_data(&recs, &months);
        let cats = t["categories"].as_array().unwrap();
        assert_eq!(cats.len(), 1); // income excluded
        assert_eq!(cats[0]["category"], json!("dining"));
        assert_eq!(cats[0]["data"], json!([10.0, 20.0]));
    }

    #[test]
    fn detect_recurring_finds_monthly_charge() {
        let recs = vec![
            Record { id: 1, data: json!({ "type": "expense", "merchant": "Netflix", "amount": 9.99, "date": "2026-04-03" }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "expense", "merchant": "Netflix", "amount": 9.99, "date": "2026-05-03" }), created_at: 0 },
            Record { id: 3, data: json!({ "type": "expense", "merchant": "netflix", "amount": 9.99, "date": "2026-06-03" }), created_at: 0 },
            Record { id: 4, data: json!({ "type": "expense", "merchant": "Coffee", "amount": 3.50, "date": "2026-06-01" }), created_at: 0 },
        ];
        let items = detect_recurring(&recs, "2026-06");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["merchant"], json!("Netflix"));
        assert_eq!(items[0]["count"], json!(3));
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
    fn budget_status_flags_overspend() {
        let budgets = vec![json!({ "category": "dining", "limit": 200 })];
        let mut spent = HashMap::new();
        spent.insert("dining".to_string(), 240.0);
        let (status, over) = compute_budget_status(&budgets, &spent);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0]["over"], json!(true));
        assert_eq!(over.len(), 1);
        assert_eq!(over[0]["by"], json!(40.0));

        // A budgeted category with no spend → 0 spent, not over.
        let (status2, over2) = compute_budget_status(&budgets, &HashMap::new());
        assert_eq!(status2[0]["spent"], json!(0.0));
        assert!(over2.is_empty());
    }

    #[test]
    fn import_hash_stable_and_merchant_case_insensitive() {
        let a = import_hash("2026-06-01", 12.5, "Cafe");
        assert_eq!(a, import_hash("2026-06-01", 12.50, "CAFE")); // same row → same hash
        assert_ne!(a, import_hash("2026-06-02", 12.5, "Cafe")); // different date → different
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
