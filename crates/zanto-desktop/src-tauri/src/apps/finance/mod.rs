//! Personal Finance — the first micro-app. Full-stack: this Rust backend (stores,
//! deterministic flows, agent tools, component decls) + a Svelte frontend slice.
//! Aggregation is deterministic Rust (never the LLM).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use serde_json::{json, Value};
use zanto_core::chat::{AppResult, GenaiTool, Target};
use zanto_core::data::{DataStore, Filter, FilterOp, Query};
use zanto_core::session::{format_ts_display, unix_now_pub};
use crate::app::{App, AppManifest, ComponentDecl, StartAction};

mod aggregate;
mod import;
use aggregate::*;
pub(crate) use import::suggest_mapping;
use import::{coerce_amount, import_hash, import_row_to_args, migrate_legacy_transactions};

const STORE: &str = "transactions";
const PROFILE_STORE: &str = "finance_profile";
const WIDGETS_STORE: &str = "finance_widgets";
const RULES_STORE: &str = "finance_category_rules";
const BUDGETS_STORE: &str = "finance_budgets";
const ACCOUNTS_STORE: &str = "finance_accounts";
const GOALS_STORE: &str = "finance_goals";

/// Account a transaction belongs to when none is given (also the seeded account).
const DEFAULT_ACCOUNT: &str = "Cash";

/// Every dashboard widget kind the UI can render. The save validator and
/// `default_widgets()` both draw from this list so they can't drift (the default
/// layout previously shipped kinds `do_save_widgets` rejected → save dropped them).
const WIDGET_KINDS: &[&str] =
    &["kpi", "chart", "table", "budget", "subscriptions", "accounts", "goals", "forecast"];

/// Default expense categories seeded when a profile omits them.
const DEFAULT_CATEGORIES: &[&str] =
    &["groceries", "dining", "transport", "utilities", "rent", "entertainment", "other"];

pub struct FinanceApp {
    manifest: AppManifest,
    /// Set once the legacy backfill has run this process (see `ensure_store`).
    migrated: AtomicBool,
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
                ACCOUNTS_STORE.to_string(),
                GOALS_STORE.to_string(),
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
                             and finish with a short written takeaway of where my money went, and \
                             mention any goals progress and the rest-of-month forecast.".into(),
                },
            ],
        };
        Arc::new(FinanceApp { manifest, migrated: AtomicBool::new(false) })
    }

    fn ensure_store(&self, data: &DataStore) -> Result<(), String> {
        data.create_store(STORE).map_err(|e| e.to_string())?;
        // One-time legacy backfill (B2-3): stamp explicit type/account on rows that
        // predate the money model / accounts, so aggregation stops relying on lossy
        // read-time defaults. Runs once per process; idempotent if it runs again.
        if !self.migrated.load(Ordering::Acquire) {
            migrate_legacy_transactions(data)?;
            self.migrated.store(true, Ordering::Release);
        }
        Ok(())
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
        let import_h = (source == "import").then(|| import_hash(&date, amount, &merchant, &account));
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
                TxnKind::Transfer => {} // neutral to a month's income/expense
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
                    TxnKind::Transfer => {} // neutral to income/expense
                }
            }
        }

        // Budget vs actual for this month (uses the per-category spend above).
        // `f` = fraction of the month elapsed, for the run-rate pace warning.
        let day_of_month: f64 = today().get(8..10).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let year: i64 = this_month.get(..4).and_then(|s| s.parse().ok()).unwrap_or(2000);
        let month_n: u32 = this_month.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
        let f = (day_of_month / days_in_month(year, month_n) as f64).clamp(0.0, 1.0);
        let (budget_status, over_budget, pace_warnings) =
            compute_budget_status(&self.budgets_vec(data), &by_cat, f);

        // Per-account balances + net worth (v0.4).
        let (accounts, net_worth) = compute_account_balances(&self.accounts_vec(data), &all);

        // Goal progress against the linked account balances (v0.5).
        let goal_status = compute_goal_status(&self.goals_vec(data), &accounts);

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

        // Rest-of-month run-rate forecast (v0.5): projected end-of-month net worth,
        // using the 3 complete months before this one as the run-rate baseline.
        let prev3 = &months[months.len().saturating_sub(4)..months.len().saturating_sub(1)];
        let projected_net_worth = compute_forecast_data(&all, net_worth, &this_month, prev3)
            .get("projected_net_worth")
            .and_then(|v| v.as_f64())
            .unwrap_or(net_worth);

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
            "accounts": accounts,
            "net_worth": net_worth,
            "goal_status": goal_status,
            "projected_net_worth": projected_net_worth,
            "pace_warnings": pace_warnings,
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
        match latest_singleton(data, WIDGETS_STORE, "widgets") {
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
                    if !WIDGET_KINDS.contains(&kind) {
                        return None;
                    }
                    let source = w.get("source").and_then(|v| v.as_str()).unwrap_or("");
                    let title = w.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    Some(json!({ "kind": kind, "title": title, "source": source }))
                })
                .collect(),
            None => Vec::new(),
        };

        save_singleton(data, WIDGETS_STORE, "widgets", json!(widgets))
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

        // Resolve category-enforcement inputs ONCE (not per row) and collect the
        // existing import hashes in a single query — the per-row do_add_transaction
        // path previously did 1000+ lock cycles (review M3).
        let cats = self.profile_categories(data);
        let rules = self.get_category_rules(data).unwrap_or_default();
        let mut seen: std::collections::HashSet<String> = data
            .query(STORE, &Query::default())
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter_map(|r| r.data.get("import_hash").and_then(|v| v.as_str()).map(str::to_string))
            .collect();

        let mut to_insert: Vec<Value> = Vec::new();
        let mut skipped = 0u64;
        let mut errors = Vec::new();
        for (i, row_v) in rows.iter().enumerate() {
            let row: Vec<String> = row_v
                .as_array()
                .map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
                .unwrap_or_default();
            let targs = match import_row_to_args(&headers, &row, &mapping, &account) {
                Some(t) => t,
                None => {
                    errors.push(json!({ "row": i, "reason": "no amount — map a debit/credit or amount column" }));
                    continue;
                }
            };
            // Same record shape + category enforcement as do_add_transaction.
            let kind = txn_kind_str(targs.get("type"));
            let amount = coerce_amount(targs.get("amount")).abs();
            let merchant = targs.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let category = resolve_category_pure(&merchant, targs.get("category").and_then(|v| v.as_str()), &cats, &rules);
            let date = targs.get("date").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(today);
            let hash = import_hash(&date, amount, &merchant, &account);
            // Dedupe against existing rows AND earlier rows in this same batch.
            if !seen.insert(hash.clone()) {
                skipped += 1;
                continue;
            }
            to_insert.push(json!({
                "type": kind, "date": date, "amount": amount, "merchant": merchant,
                "category": category, "note": "", "source": "import", "account": account,
                "import_hash": hash,
            }));
        }

        // One transaction for the whole batch: all-or-nothing (B3-4).
        let inserted = if to_insert.is_empty() {
            0
        } else {
            data.insert_batch(STORE, &to_insert).map_err(|e| e.to_string())?.len() as u64
        };

        let mut result = json!({ "inserted": inserted, "skipped": skipped, "errors": errors });
        // Echo parse-level data loss (B3-3) so the import UI can warn the user.
        if let Value::Object(o) = &mut result {
            for k in ["total_rows", "truncated", "malformed"] {
                if let Some(v) = args.get(k) {
                    o.insert(k.to_string(), v.clone());
                }
            }
        }
        Ok(result)
    }

    /// Recurring/subscription detection over all transactions.
    fn compute_recurring(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        Ok(json!({ "items": detect_recurring(&all, &today()[..7]) }))
    }

    /// Rest-of-this-month cash-flow forecast (run-rate + 3-month averages).
    fn compute_forecast(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = data.query(STORE, &Query::default()).map_err(|e| e.to_string())?;
        let (_, net_worth) = compute_account_balances(&self.accounts_vec(data), &all);
        let this_month = today()[..7].to_string();
        let months = last_n_months(&this_month, 4);
        let prev: Vec<String> = months.iter().take(3).cloned().collect();
        Ok(compute_forecast_data(&all, net_worth, &this_month, &prev))
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
        match latest_singleton(data, BUDGETS_STORE, "budgets") {
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
        save_singleton(data, BUDGETS_STORE, "budgets", json!(budgets))
    }

    // ---- accounts + transfers (v0.4) ----

    /// The user's accounts (latest-wins), defaulting to a single seeded "Cash".
    fn get_accounts(&self, data: &DataStore) -> Result<Value, String> {
        match latest_singleton(data, ACCOUNTS_STORE, "accounts") {
            Some(a) if a.as_array().map(|x| !x.is_empty()).unwrap_or(false) => Ok(json!({ "accounts": a })),
            _ => Ok(json!({ "accounts": default_accounts() })),
        }
    }

    fn accounts_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_accounts(data)
            .ok()
            .and_then(|a| a.get("accounts").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    /// Persist the account list. Keeps entries with a non-empty name; type
    /// defaults to "cash", opening_balance coerced to a number (default 0).
    fn do_save_accounts(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        data.create_store(ACCOUNTS_STORE).map_err(|e| e.to_string())?;
        let accounts: Vec<Value> = match args.get("accounts").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|a| {
                    let name = a.get("name").and_then(|v| v.as_str())?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let typ = a.get("type").and_then(|v| v.as_str()).unwrap_or("cash").to_string();
                    let opening = coerce_amount(a.get("opening_balance"));
                    Some(json!({ "name": name, "type": typ, "opening_balance": opening }))
                })
                .collect(),
            None => Vec::new(),
        };
        save_singleton(data, ACCOUNTS_STORE, "accounts", json!(accounts))
    }

    /// Record a transfer between two of the user's accounts (a single row,
    /// neutral to income/expense; moves money in `compute_account_balances`).
    fn do_add_transfer(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let amount = coerce_amount(args.get("amount")).abs();
        let from = args.get("from_account").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(DEFAULT_ACCOUNT).to_string();
        let to = args.get("to_account").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if to.is_empty() || from == to {
            return Err("a transfer needs distinct from/to accounts".to_string());
        }
        let date = args.get("date").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(today);
        let note = args.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let record = json!({
            "type": "transfer", "amount": amount, "account": from, "to_account": to,
            "category": "transfer", "date": date, "note": note, "source": "manual",
        });
        let id = data.insert(STORE, &record).map_err(|e| e.to_string())?;
        Ok(json!({ "status": "added", "id": id, "record": record }))
    }

    // ---- goals (v0.5) ----

    /// The user's savings/debt goals (latest-wins), default empty.
    fn get_goals(&self, data: &DataStore) -> Result<Value, String> {
        match latest_singleton(data, GOALS_STORE, "goals") {
            Some(goals) => Ok(json!({ "goals": goals })),
            None => Ok(json!({ "goals": [] })),
        }
    }

    fn goals_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_goals(data)
            .ok()
            .and_then(|g| g.get("goals").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    /// Persist goals. Keeps entries with a non-empty name; kind defaults to
    /// savings, target coerced to a number.
    fn do_save_goals(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        data.create_store(GOALS_STORE).map_err(|e| e.to_string())?;
        let goals: Vec<Value> = match args.get("goals").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|g| {
                    let name = g.get("name").and_then(|v| v.as_str())?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let kind = match g.get("kind").and_then(|v| v.as_str()) {
                        Some("debt") => "debt",
                        _ => "savings",
                    };
                    let account = g.get("account").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let target = coerce_amount(g.get("target")).abs();
                    let target_date = g.get("target_date").cloned().unwrap_or(Value::Null);
                    Some(json!({ "name": name, "kind": kind, "account": account, "target": target, "target_date": target_date }))
                })
                .collect(),
            None => Vec::new(),
        };
        save_singleton(data, GOALS_STORE, "goals", json!(goals))
    }
}

/// Default dashboard widgets mirroring the fixed F1 layout. Each `source`
/// names a slice of the `overview` query result.
fn default_widgets() -> Value {
    json!([
        { "kind": "kpi", "title": "Net worth", "source": "net_worth" },
        { "kind": "kpi", "title": "Balance", "source": "balance" },
        { "kind": "kpi", "title": "This month", "source": "month_total" },
        { "kind": "kpi", "title": "Income", "source": "income" },
        { "kind": "kpi", "title": "Net", "source": "net_cash_flow" },
        { "kind": "chart", "title": "Spending — last 6 months", "source": "series" },
        { "kind": "table", "title": "Top categories", "source": "top_categories" },
        { "kind": "budget", "title": "Budget vs actual", "source": "budget_status" },
        { "kind": "subscriptions", "title": "Subscriptions", "source": "recurring" },
        { "kind": "accounts", "title": "Accounts", "source": "accounts" },
        { "kind": "goals", "title": "Goals", "source": "goal_status" },
        { "kind": "forecast", "title": "Forecast", "source": "forecast" },
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
                        "account": { "type": "string", "description": "Which of the user's accounts; defaults to Cash" },
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
            GenaiTool::new("add_transfer")
                .with_description("Move money between two of the user's own accounts (neutral to income/expense).")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "amount": { "type": "number" },
                        "from_account": { "type": "string" },
                        "to_account": { "type": "string" },
                        "date": { "type": "string", "description": "YYYY-MM-DD; defaults to today" },
                        "note": { "type": "string" }
                    },
                    "required": ["amount", "from_account", "to_account"]
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
            "add_transfer" => Some(self.do_add_transfer(data, args).map(AppResult::Data)),
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
            "forecast" => self.compute_forecast(data),
            "accounts" => self.get_accounts(data),
            "goals" => self.get_goals(data),
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
            "save_accounts" => self.do_save_accounts(data, args),
            "add_transfer" => self.do_add_transfer(data, args),
            "save_goals" => self.do_save_goals(data, args),
            "migrate_transactions" => {
                self.ensure_store(data)?;
                migrate_legacy_transactions(data).map(|n| json!({ "migrated": n }))
            }
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

/// Read the latest value stored under `key` in a latest-wins singleton store
/// (widgets/budgets/accounts/goals). Returns the `key` field of the row with the
/// greatest `saved_at`, or None when the store is empty.
fn latest_singleton(data: &DataStore, store: &str, key: &str) -> Option<Value> {
    data.create_store(store).ok()?;
    let rows = data.query(store, &Query::default()).ok()?;
    rows.into_iter()
        .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0))
        .and_then(|r| r.data.get(key).cloned())
}

/// Persist `value` under `key` in a singleton store, UPDATING the existing row in
/// place (and pruning any older duplicates) instead of inserting a new row every
/// save. The widgets/budgets/accounts/goals stores previously grew one dead row
/// per save (review H6) — this converges them to a single row.
fn save_singleton(data: &DataStore, store: &str, key: &str, value: Value) -> Result<Value, String> {
    data.create_store(store).map_err(|e| e.to_string())?;
    let record = json!({ key: value, "saved_at": unix_now_pub() });
    let rows = data.query(store, &Query::default()).map_err(|e| e.to_string())?;
    let latest = rows
        .iter()
        .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0))
        .map(|r| r.id);
    match latest {
        Some(id) => {
            data.update(store, id, &record).map_err(|e| e.to_string())?;
            // Prune older rows so legacy multi-row stores converge to one.
            for r in &rows {
                if r.id != id {
                    let _ = data.delete(store, r.id);
                }
            }
        }
        None => {
            data.insert(store, &record).map_err(|e| e.to_string())?;
        }
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zanto_core::data::Record;

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
    fn every_default_widget_kind_is_saveable() {
        // Regression: default_widgets() ships budget/subscriptions/accounts/goals/
        // forecast, which do_save_widgets used to reject → saving the dashboard
        // silently dropped them. Every default kind must be in WIDGET_KINDS.
        let defaults = default_widgets();
        for w in defaults.as_array().unwrap() {
            let kind = w.get("kind").and_then(|v| v.as_str()).unwrap();
            assert!(WIDGET_KINDS.contains(&kind), "default widget kind '{kind}' is not saveable");
        }
    }

    #[test]
    fn forecast_projects_from_run_rate() {
        // 3 complete months at £1000 expense; £300 spent so far this month.
        let recs = vec![
            Record { id: 1, data: json!({ "type": "expense", "amount": 1000, "date": "2026-03-15" }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "expense", "amount": 1000, "date": "2026-04-15" }), created_at: 0 },
            Record { id: 3, data: json!({ "type": "expense", "amount": 1000, "date": "2026-05-15" }), created_at: 0 },
            Record { id: 4, data: json!({ "type": "expense", "amount": 300, "date": "2026-06-10" }), created_at: 0 },
        ];
        let prev = vec!["2026-03".to_string(), "2026-04".to_string(), "2026-05".to_string()];
        let f = compute_forecast_data(&recs, 5000.0, "2026-06", &prev);
        assert_eq!(f["avg_monthly_expense"], json!(1000.0));
        assert_eq!(f["expected_expense"], json!(1000.0)); // max(300, 1000)
        // 5000 + (0−0) − (1000−300) = 4300
        assert_eq!(f["projected_net_worth"], json!(4300.0));
    }

    #[test]
    fn goal_status_savings_and_debt() {
        let accounts = vec![
            json!({ "name": "Savings", "type": "savings", "balance": 2500.0 }),
            json!({ "name": "Card", "type": "card", "balance": -800.0 }),
        ];
        let goals = vec![
            json!({ "name": "Emergency", "kind": "savings", "account": "Savings", "target": 10000 }),
            json!({ "name": "Pay off card", "kind": "debt", "account": "Card", "target": 1000 }),
        ];
        let s = compute_goal_status(&goals, &accounts);
        let prog = |g: &Value| g["progress"].as_f64().unwrap();
        let savings = s.iter().find(|g| g["name"] == "Emergency").unwrap();
        assert_eq!(savings["current"], json!(2500.0));
        assert!((prog(savings) - 0.25).abs() < 1e-9);
        assert_eq!(savings["complete"], json!(false));
        let debt = s.iter().find(|g| g["name"] == "Pay off card").unwrap();
        assert_eq!(debt["owed"], json!(800.0));
        assert!((prog(debt) - 0.2).abs() < 1e-9); // 1 - 800/1000
    }

    #[test]
    fn account_balances_and_net_worth_with_transfer() {
        let accounts = vec![
            json!({ "name": "Checking", "type": "checking", "opening_balance": 1000 }),
            json!({ "name": "Card", "type": "card", "opening_balance": 0 }),
        ];
        let recs = vec![
            Record { id: 1, data: json!({ "type": "expense", "amount": 50, "account": "Card" }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "transfer", "amount": 200, "account": "Checking", "to_account": "Card" }), created_at: 0 },
            Record { id: 3, data: json!({ "type": "income", "amount": 500, "account": "Checking" }), created_at: 0 },
        ];
        let (accts, net) = compute_account_balances(&accounts, &recs);
        let bal = |name: &str| accts.iter().find(|a| a["name"] == name).unwrap()["balance"].as_f64().unwrap();
        assert_eq!(bal("Checking"), 1300.0); // 1000 + 500 income − 200 transfer out
        assert_eq!(bal("Card"), 150.0); // 0 − 50 expense + 200 transfer in
        assert_eq!(net, 1450.0); // transfer nets to zero across accounts
    }

    #[test]
    fn budget_status_flags_overspend() {
        let budgets = vec![json!({ "category": "dining", "limit": 200 })];
        let mut spent = HashMap::new();
        spent.insert("dining".to_string(), 240.0);
        let (status, over, _pace) = compute_budget_status(&budgets, &spent, 1.0);
        assert_eq!(status.len(), 1);
        assert_eq!(status[0]["over"], json!(true));
        assert_eq!(over.len(), 1);
        assert_eq!(over[0]["by"], json!(40.0));

        // A budgeted category with no spend → 0 spent, not over.
        let (status2, over2, _) = compute_budget_status(&budgets, &HashMap::new(), 1.0);
        assert_eq!(status2[0]["spent"], json!(0.0));
        assert!(over2.is_empty());
    }

    #[test]
    fn pace_warning_when_on_track_to_exceed() {
        let budgets = vec![json!({ "category": "dining", "limit": 200 })];
        let mut spent = HashMap::new();
        spent.insert("dining".to_string(), 160.0);
        // 60% through the month: projected 160/0.6 ≈ 266 > 200, not yet over.
        let (status, over, pace) = compute_budget_status(&budgets, &spent, 0.6);
        assert!(over.is_empty());
        assert_eq!(pace.len(), 1);
        assert_eq!(status[0]["on_track_to_exceed"], json!(true));
    }

    #[test]
    fn orphaned_account_money_is_not_lost() {
        // A transaction on an account that is NOT declared (renamed/deleted) must
        // still count toward net worth as an "unlinked" bucket.
        let accounts = vec![json!({ "name": "Checking", "type": "checking", "opening_balance": 100 })];
        let recs = vec![
            Record { id: 1, data: json!({ "type": "expense", "amount": 30, "account": "Checking" }), created_at: 0 },
            Record { id: 2, data: json!({ "type": "income", "amount": 50, "account": "OldCard" }), created_at: 0 },
        ];
        let (accts, net) = compute_account_balances(&accounts, &recs);
        let unlinked = accts.iter().find(|a| a["name"] == "OldCard").unwrap();
        assert_eq!(unlinked["type"], json!("unlinked"));
        assert_eq!(unlinked["balance"], json!(50.0));
        assert_eq!(net, 120.0); // 100 - 30 (Checking) + 50 (OldCard)
    }

}
