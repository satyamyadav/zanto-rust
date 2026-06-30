//! Personal Finance — the first micro-app. Full-stack: this Rust backend (stores,
//! deterministic flows, agent tools, component decls) + a Svelte frontend slice.
//! Aggregation is deterministic Rust (never the LLM).
//!
//! Storage (W5): typed SQLite `fin_*` tables (see `schema.rs`), not the schemaless
//! JSON `DataStore`. Handlers read/write typed rows but RECONSTRUCT each
//! transaction as a legacy-shaped `Record` before feeding the unchanged aggregate
//! functions in `aggregate.rs`, so the business logic and JSON output shapes are
//! byte-identical to the JSON-store era.

use crate::app::{App, AppManifest, ComponentDecl, StartAction};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use zanto_core::chat::{AppResult, GenaiTool, Target};
use zanto_core::data::{DataStore, Record};
use zanto_core::rusqlite::{self, params, Connection, OptionalExtension};
use zanto_core::session::{format_ts_display, unix_now_pub};

mod aggregate;
mod import;
mod schema;
use aggregate::*;
pub(crate) use import::suggest_mapping;
use import::{coerce_amount, import_hash, import_row_to_args};

/// Dashboard widget layout — the one remaining JSON store. Widgets are pure UI
/// layout (no integrity to enforce, no `fin_*` table), so they stay in the
/// schemaless `DataStore` rather than typed SQLite.
const WIDGETS_STORE: &str = "finance_widgets";

/// Account a transaction belongs to when none is given (also the seeded account).
const DEFAULT_ACCOUNT: &str = "Cash";

/// Every dashboard widget kind the UI can render. The save validator and
/// `default_widgets()` both draw from this list so they can't drift (the default
/// layout previously shipped kinds `do_save_widgets` rejected → save dropped them).
const WIDGET_KINDS: &[&str] = &[
    "kpi",
    "chart",
    "table",
    "budget",
    "subscriptions",
    "accounts",
    "goals",
    "forecast",
];

/// Default expense categories seeded when a profile omits them.
const DEFAULT_CATEGORIES: &[&str] = &[
    "groceries",
    "dining",
    "transport",
    "utilities",
    "rent",
    "entertainment",
    "other",
];

pub struct FinanceApp {
    manifest: AppManifest,
    /// Set once the typed schema has been ensured this process (see `ensure_store`).
    migrated: AtomicBool,
}

impl FinanceApp {
    #[allow(clippy::new_ret_no_self)] // reason: factory returns Arc<dyn App>, not FinanceApp
    pub fn new() -> Arc<dyn App> {
        let manifest = AppManifest {
            id: "finance".to_string(),
            name: "Personal Finance".to_string(),
            description: "Track expenses and view spending summaries.".to_string(),
            // Finance data lives in typed `fin_*` SQLite tables (see schema.rs); the
            // only JSON `DataStore` store left is the dashboard widget layout.
            stores: vec![WIDGETS_STORE.to_string()],
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
        Arc::new(FinanceApp {
            manifest,
            migrated: AtomicBool::new(false),
        })
    }

    /// Ensure the typed `fin_*` schema exists + default categories are seeded for
    /// this workspace. The per-process `migrated` flag short-circuits the (idempotent)
    /// DDL after the first finance access; the seed itself is idempotent too.
    fn ensure_store(&self, data: &DataStore) -> Result<(), String> {
        if self.migrated.load(Ordering::Acquire) {
            return Ok(());
        }
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            schema::ensure_schema(c)?;
            schema::seed_default_categories(c, &ws)?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        self.migrated.store(true, Ordering::Release);
        Ok(())
    }

    // ---- deterministic flows (shared by agentic + manual paths) ----

    fn do_add_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        // Amount is stored POSITIVE; the sign is carried by `type` so aggregation
        // is unambiguous regardless of how the model phrased the value.
        let kind = txn_kind_str(args.get("type"));
        let amount = coerce_amount(args.get("amount")).abs();
        let merchant = args
            .get("merchant")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        // Enforce categories: keep a requested category only if it's known; else try
        // merchant rules; else "uncategorized" (review queue).
        let category = self.resolve_category(
            data,
            &merchant,
            args.get("category").and_then(|v| v.as_str()),
        );
        let note = args
            .get("note")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
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
        let import_h =
            (source == "import").then(|| import_hash(&date, amount, &merchant, &account));

        let ws = data.workspace().to_string();
        let result = data
            .with_conn(|c| {
                if let Some(h) = &import_h {
                    let exists: bool = c
                        .query_row(
                            "SELECT 1 FROM fin_transactions WHERE workspace = ?1 AND import_hash = ?2 LIMIT 1",
                            params![ws, h],
                            |_| Ok(()),
                        )
                        .optional()?
                        .is_some();
                    if exists {
                        return Ok(json!({ "status": "duplicate_skipped", "import_hash": h }));
                    }
                }
                let account_id = ensure_account_id(c, &ws, &account)?;
                let category_id = category_id_for(c, &ws, &category)?;
                let now = unix_now_pub() as i64;
                c.execute(
                    "INSERT INTO fin_transactions
                       (workspace, account_id, category_id, amount, transaction_type,
                        merchant, notes, transaction_date, source, import_hash, created_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    params![
                        ws,
                        account_id,
                        category_id,
                        amount,
                        kind,
                        merchant,
                        note,
                        date,
                        source,
                        import_h,
                        now
                    ],
                )?;
                let id = c.last_insert_rowid();
                // Reconstruct the SAME JSON record the JSON store returned.
                let mut record = json!({
                    "type": kind, "date": date, "amount": amount, "merchant": merchant,
                    "category": category, "note": note, "source": source, "account": account,
                });
                if let Some(h) = &import_h {
                    record["import_hash"] = json!(h);
                }
                Ok(json!({ "status": "added", "id": id, "record": record }))
            })
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    /// Edit a transaction by id. Only the provided fields are changed; amount is
    /// re-coerced to a positive number, type is normalized.
    fn do_update_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "update_transaction requires an integer `id`".to_string())?;

        // Load the current record (legacy shape) so we apply the SAME field-merge +
        // category re-resolution the JSON path did.
        let ws = data.workspace().to_string();
        let existing = data
            .with_conn(|c| load_one_txn_record(c, &ws, id))
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no transaction with id {id}"))?;
        let mut rec = existing.data;
        {
            let obj = rec
                .as_object_mut()
                .ok_or_else(|| "transaction record is not an object".to_string())?;
            if args.get("type").is_some() {
                obj.insert("type".into(), json!(txn_kind_str(args.get("type"))));
            }
            if args.get("amount").is_some() {
                obj.insert(
                    "amount".into(),
                    json!(coerce_amount(args.get("amount")).abs()),
                );
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
            let merchant = rec
                .get("merchant")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let requested = rec
                .get("category")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let resolved = self.resolve_category(data, &merchant, requested.as_deref());
            rec["category"] = json!(resolved);
        }

        // Write the merged fields back to the typed row.
        let kind = txn_kind_str(rec.get("type"));
        let amount = coerce_amount(rec.get("amount")).abs();
        let merchant = rec.get("merchant").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let note = rec.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let date = rec.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let category = rec.get("category").and_then(|v| v.as_str()).unwrap_or("uncategorized").to_string();
        data.with_conn(|c| {
            let category_id = category_id_for(c, &ws, &category)?;
            c.execute(
                "UPDATE fin_transactions SET transaction_type=?1, amount=?2, merchant=?3,
                   notes=?4, transaction_date=?5, category_id=?6 WHERE id=?7 AND workspace=?8",
                params![kind, amount, merchant, note, date, category_id, id, ws],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "status": "updated", "id": id, "record": rec }))
    }

    /// Delete a transaction by id. Idempotent at the store layer.
    fn do_delete_transaction(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "delete_transaction requires an integer `id`".to_string())?;
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            c.execute(
                "DELETE FROM fin_transactions WHERE id = ?1 AND workspace = ?2",
                params![id, ws],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "status": "deleted", "id": id }))
    }

    // ---- category enforcement + rules (v0.2) ----

    /// The known category names (the seeded/typed `fin_categories` for this
    /// workspace), or the defaults when somehow empty — so category enforcement
    /// still works before onboarding.
    fn profile_categories(&self, data: &DataStore) -> Vec<String> {
        let ws = data.workspace().to_string();
        let cats = data
            .with_conn(|c| category_names(c, &ws))
            .unwrap_or_default();
        if cats.is_empty() {
            DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect()
        } else {
            cats
        }
    }

    /// All saved merchant→category rules, each carrying its `id` and the category
    /// NAME (the resolver works over names).
    fn get_category_rules(&self, data: &DataStore) -> Result<Vec<Value>, String> {
        self.ensure_store(data)?;
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT r.id, r.merchant_contains, cat.name
                 FROM fin_category_rules r
                 JOIN fin_categories cat ON cat.id = r.category_id
                 WHERE r.workspace = ?1 ORDER BY r.id",
            )?;
            let rows = stmt.query_map(params![ws], |r| {
                Ok(json!({
                    "id": r.get::<_, i64>(0)?,
                    "merchant_contains": r.get::<_, String>(1)?,
                    "category": r.get::<_, String>(2)?,
                }))
            })?;
            rows.collect::<rusqlite::Result<Vec<Value>>>()
        })
        .map_err(|e| e.to_string())
    }

    /// Resolve a category: a requested known category wins; else a matching merchant
    /// rule; else "uncategorized".
    fn resolve_category(
        &self,
        data: &DataStore,
        merchant: &str,
        requested: Option<&str>,
    ) -> String {
        let cats = self.profile_categories(data);
        let rules = self.get_category_rules(data).unwrap_or_default();
        resolve_category_pure(merchant, requested, &cats, &rules)
    }

    fn do_save_category_rule(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let merchant_contains = args
            .get("merchant_contains")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if merchant_contains.is_empty() || category.is_empty() {
            return Err("a rule needs a non-empty merchant_contains and category".to_string());
        }
        let ws = data.workspace().to_string();
        let id = data
            .with_conn(|c| {
                // A rule needs a real category row (FK). Create it (expense) if new.
                let category_id = ensure_category_id(c, &ws, &category)?;
                c.execute(
                    "INSERT INTO fin_category_rules (workspace, merchant_contains, category_id)
                     VALUES (?1, ?2, ?3)",
                    params![ws, merchant_contains, category_id],
                )?;
                Ok(c.last_insert_rowid())
            })
            .map_err(|e| e.to_string())?;
        let record = json!({ "merchant_contains": merchant_contains, "category": category });
        Ok(json!({ "status": "saved", "id": id, "record": record }))
    }

    fn do_delete_category_rule(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let id = args
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "delete_category_rule requires an integer `id`".to_string())?;
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            c.execute(
                "DELETE FROM fin_category_rules WHERE id = ?1 AND workspace = ?2",
                params![id, ws],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "status": "deleted", "id": id }))
    }

    fn compute_transactions(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let cat_filter = args.get("category").and_then(|v| v.as_str()).map(str::to_string);
        let month_filter = args.get("month").and_then(|v| v.as_str()).map(str::to_string);
        let all = self.load_txn_records(data)?;
        // Include each row's `id` so the UI can edit/delete a specific transaction.
        // Filter in Rust over the legacy-shaped records to match the JSON-store
        // semantics exactly (category Eq; month substring-of-date).
        let rows: Vec<Value> = all
            .into_iter()
            .filter(|r| {
                cat_filter
                    .as_deref()
                    .map(|c| r.data.get("category").and_then(|v| v.as_str()) == Some(c))
                    .unwrap_or(true)
            })
            .filter(|r| {
                month_filter
                    .as_deref()
                    .map(|m| {
                        r.data
                            .get("date")
                            .and_then(|v| v.as_str())
                            .map(|d| d.contains(m))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
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

        let all = self.load_txn_records(data)?;
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
        let by_category: Vec<Value> = by_cat
            .into_iter()
            .map(|(c, t)| json!({ "category": c, "total": t }))
            .collect();
        Ok(
            json!({ "month": month, "income": income, "total": total, "net": income - total, "by_category": by_category }),
        )
    }

    /// Dashboard overview: lifetime balance, this-month spend, top categories
    /// (this month), and a 6-month spend series. `empty: true` when no
    /// transactions exist. All aggregation is deterministic Rust.
    fn compute_overview(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = self.load_txn_records(data)?;
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
        // Per-month INCOME (for the stacked cashflow chart's income/savings).
        let mut income_by_month: HashMap<String, f64> = HashMap::new();

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
            if matches!(t.kind, TxnKind::Income) && t.date.len() >= 7 {
                *income_by_month.entry(t.date[..7].to_string()).or_insert(0.0) += t.amount;
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
        let day_of_month: f64 = today()
            .get(8..10)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let year: i64 = this_month
            .get(..4)
            .and_then(|s| s.parse().ok())
            .unwrap_or(2000);
        let month_n: u32 = this_month
            .get(5..7)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
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
        let series: Vec<f64> = months
            .iter()
            .map(|m| *by_month.get(m).unwrap_or(&0.0))
            .collect();

        // Month-over-month change in spend (this vs previous month).
        let n = series.len();
        let mom_delta = if n >= 2 {
            series[n - 1] - series[n - 2]
        } else {
            0.0
        };
        let mom_prev = if n >= 2 { series[n - 2] } else { 0.0 };
        let mom_pct = if mom_prev > 0.0 {
            mom_delta / mom_prev
        } else {
            0.0
        };

        // Rest-of-month run-rate forecast (v0.5): projected end-of-month net worth,
        // using the 3 complete months before this one as the run-rate baseline.
        let prev3 = &months[months.len().saturating_sub(4)..months.len().saturating_sub(1)];
        let projected_net_worth = compute_forecast_data(&all, net_worth, &this_month, prev3)
            .get("projected_net_worth")
            .and_then(|v| v.as_f64())
            .unwrap_or(net_worth);

        // ── FinanceV1 (4-tab dashboard) additive fields. The legacy keys above
        //    stay for the old Dashboard; these power the new UI. ──
        // Currency from the profile (single-currency view).
        let profile = self.get_profile(data).unwrap_or_else(|_| json!({}));
        let currency = profile
            .get("currency")
            .and_then(|v| v.as_str())
            .unwrap_or("USD")
            .to_string();
        let monthly_income_setting = profile.get("monthly_income").and_then(|v| v.as_f64());

        // Short month labels (Jan…) aligned to the 6-month `months` window.
        const MON: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let trend_months: Vec<String> = months
            .iter()
            .map(|m| {
                m.get(5..7)
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|n| MON.get(n.saturating_sub(1)))
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| m.clone())
            })
            .collect();

        // Stacked cashflow per month: income / spend / savings(=income−spend, ≥0).
        let monthly: Vec<Value> = months
            .iter()
            .zip(trend_months.iter())
            .map(|(ym, label)| {
                let inc = *income_by_month.get(ym).unwrap_or(&0.0);
                let sp = *by_month.get(ym).unwrap_or(&0.0);
                json!({ "month": label, "income": inc, "spend": sp, "savings": (inc - sp).max(0.0) })
            })
            .collect();

        // Safe-to-spend (W3): income this month (or the monthly_income setting if
        // larger/known) minus what's already spent minus what's still budgeted but
        // unspent. Deterministic; never negative.
        let base_income = monthly_income_setting.filter(|v| *v > 0.0).unwrap_or(income);
        let budget_remaining: f64 = budget_status
            .iter()
            .filter_map(|b| {
                let lim = b.get("limit").and_then(|v| v.as_f64())?;
                let sp = b.get("spent").and_then(|v| v.as_f64()).unwrap_or(0.0);
                Some((lim - sp).max(0.0))
            })
            .sum();
        let safe_to_spend = (base_income - month_total - budget_remaining).max(0.0);

        // This-month category breakdown (donut). Detected subscriptions.
        let category_breakdown = top_categories.clone();
        let subscriptions = self
            .compute_recurring(data)
            .ok()
            .and_then(|v| v.get("recurring").cloned())
            .unwrap_or_else(|| json!([]));

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
            // FinanceV1 additive fields:
            "currency": currency,
            "spent": month_total,
            "net": income - month_total,
            "safe_to_spend": safe_to_spend,
            "trend_months": trend_months,
            "monthly": monthly,
            "category_breakdown": category_breakdown,
            "subscriptions": subscriptions,
        }))
    }

    // ---- transaction loading (typed → legacy `Record` shape) ----

    /// Load every transaction (and transfer) for this workspace, reconstructing the
    /// EXACT legacy JSON `Record` shape the aggregate functions consume:
    /// `{ type, amount, category, date, merchant, account, source, import_hash, .. }`.
    /// Transfers are emitted as `type:"transfer"` rows carrying `account` (from) and
    /// `to_account` (to), matching what `compute_account_balances` reads. category_id
    /// NULL → "uncategorized".
    fn load_txn_records(&self, data: &DataStore) -> Result<Vec<Record>, String> {
        let ws = data.workspace().to_string();
        data.with_conn(|c| load_txn_records_conn(c, &ws))
            .map_err(|e| e.to_string())
    }

    // ---- onboarding / profile (F2) ----

    /// The saved onboarding profile, or `{ "setup": false }` when none exists.
    /// Reconstructs the SAME shape the JSON store returned (setup/currency/
    /// monthly_income/categories), reading the typed `fin_profile` + `fin_categories`.
    fn get_profile(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            let row: Option<(String, Option<f64>, i64)> = c
                .query_row(
                    "SELECT currency, monthly_income, setup FROM fin_profile WHERE workspace = ?1",
                    params![ws],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            match row {
                None => Ok(json!({ "setup": false })),
                Some((currency, monthly_income, setup)) => {
                    let categories = category_names(c, &ws)?;
                    Ok(json!({
                        "setup": setup != 0,
                        "currency": currency,
                        "monthly_income": monthly_income,
                        "categories": categories,
                    }))
                }
            }
        })
        .map_err(|e| e.to_string())
    }

    /// Persist the onboarding profile into `fin_profile` (one row per workspace) and
    /// ensure each supplied category exists in `fin_categories`. Returns the SAME
    /// profile JSON shape as before. Categories default when omitted.
    fn do_save_profile(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;

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

        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            for name in &categories {
                ensure_category_id(c, &ws, name)?;
            }
            c.execute(
                "INSERT INTO fin_profile (workspace, currency, monthly_income, setup)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(workspace) DO UPDATE SET
                   currency = excluded.currency,
                   monthly_income = excluded.monthly_income,
                   setup = 1",
                params![ws, currency, monthly_income],
            )?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

        Ok(json!({
            "setup": true,
            "currency": currency,
            "monthly_income": monthly_income,
            "categories": categories,
            "saved_at": unix_now_pub(),
        }))
    }

    // ---- dashboard widgets (F4) ----
    // Widgets remain in the JSON store: they're pure UI layout (no integrity to
    // enforce) and out of W5's typed-storage scope.

    fn ensure_widgets_store(&self, data: &DataStore) -> Result<(), String> {
        data.create_store(WIDGETS_STORE).map_err(|e| e.to_string())
    }

    /// The saved dashboard widget list. Picks the row with the greatest
    /// `saved_at` (last write wins). Returns `{ widgets: [...] }`. When none has
    /// been saved, returns a sensible default layout mirroring the fixed F1
    /// dashboard (balance + this-month KPIs, the 6-month chart, top categories).
    fn get_widgets(&self, data: &DataStore) -> Result<Value, String> {
        match latest_singleton(data, WIDGETS_STORE, "widgets") {
            Some(widgets) => Ok(json!({ "widgets": widgets })),
            None => Ok(json!({ "widgets": default_widgets() })),
        }
    }

    /// Persist the dashboard widget list. Only the recognized fields (`kind`,
    /// `title`, `source`) of each widget are kept.
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
    /// args: `{ headers: [..], rows: [[..]], mapping, account }`. Same parse/map/
    /// dedup/category-resolution as the JSON era; rows are inserted typed.
    fn do_import_transactions(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let headers: Vec<String> = args
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let rows = args
            .get("rows")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mapping = args.get("mapping").cloned().unwrap_or_else(|| json!({}));
        let account = args
            .get("account")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_ACCOUNT)
            .to_string();

        // Resolve category-enforcement inputs ONCE (not per row).
        let cats = self.profile_categories(data);
        let rules = self.get_category_rules(data).unwrap_or_default();

        // Parsed rows ready to insert, plus their resolved category names.
        struct Pending {
            kind: &'static str,
            amount: f64,
            merchant: String,
            category: String,
            date: String,
            hash: String,
        }

        let mut pending: Vec<Pending> = Vec::new();
        let mut errors = Vec::new();
        for (i, row_v) in rows.iter().enumerate() {
            let row: Vec<String> = row_v
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| x.as_str().unwrap_or("").to_string())
                        .collect()
                })
                .unwrap_or_default();
            let targs = match import_row_to_args(&headers, &row, &mapping, &account) {
                Some(t) => t,
                None => {
                    errors.push(json!({ "row": i, "reason": "no amount — map a debit/credit or amount column" }));
                    continue;
                }
            };
            let kind = txn_kind_str(targs.get("type"));
            let amount = coerce_amount(targs.get("amount")).abs();
            let merchant = targs
                .get("merchant")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let category = resolve_category_pure(
                &merchant,
                targs.get("category").and_then(|v| v.as_str()),
                &cats,
                &rules,
            );
            let date = targs
                .get("date")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(today);
            let hash = import_hash(&date, amount, &merchant, &account);
            pending.push(Pending {
                kind,
                amount,
                merchant,
                category,
                date,
                hash,
            });
        }

        let ws = data.workspace().to_string();
        let (inserted, skipped) = data
            .with_conn(|c| {
                // Existing import hashes for this workspace (dedupe against prior runs).
                let mut seen: std::collections::HashSet<String> = {
                    let mut stmt = c.prepare(
                        "SELECT import_hash FROM fin_transactions
                         WHERE workspace = ?1 AND import_hash IS NOT NULL",
                    )?;
                    let rows = stmt.query_map(params![ws], |r| r.get::<_, String>(0))?;
                    rows.collect::<rusqlite::Result<std::collections::HashSet<String>>>()?
                };
                let account_id = ensure_account_id(c, &ws, &account)?;
                let now = unix_now_pub() as i64;
                // All-or-nothing batch (B3-4): one transaction wraps every insert.
                let tx = c.unchecked_transaction()?;
                let mut inserted = 0u64;
                let mut skipped = 0u64;
                {
                    let mut ins = tx.prepare(
                        "INSERT INTO fin_transactions
                           (workspace, account_id, category_id, amount, transaction_type,
                            merchant, notes, transaction_date, source, import_hash, created_at)
                         VALUES (?1,?2,?3,?4,?5,?6,'',?7,'import',?8,?9)",
                    )?;
                    for p in &pending {
                        // Dedupe against existing rows AND earlier rows in this batch.
                        if !seen.insert(p.hash.clone()) {
                            skipped += 1;
                            continue;
                        }
                        let category_id = category_id_for(&tx, &ws, &p.category)?;
                        ins.execute(params![
                            ws,
                            account_id,
                            category_id,
                            p.amount,
                            p.kind,
                            p.merchant,
                            p.date,
                            p.hash,
                            now
                        ])?;
                        inserted += 1;
                    }
                }
                tx.commit()?;
                Ok((inserted, skipped))
            })
            .map_err(|e| e.to_string())?;

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
        let all = self.load_txn_records(data)?;
        Ok(json!({ "items": detect_recurring(&all, &today()[..7]) }))
    }

    /// Rest-of-this-month cash-flow forecast (run-rate + 3-month averages).
    fn compute_forecast(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = self.load_txn_records(data)?;
        let (_, net_worth) = compute_account_balances(&self.accounts_vec(data), &all);
        let this_month = today()[..7].to_string();
        let months = last_n_months(&this_month, 4);
        let prev: Vec<String> = months.iter().take(3).cloned().collect();
        Ok(compute_forecast_data(&all, net_worth, &this_month, &prev))
    }

    /// Per-category 6-month expense trends + overall month-over-month change.
    fn compute_trends(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let all = self.load_txn_records(data)?;
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
        let last = if months.len() >= 2 {
            *by_month.get(&months[months.len() - 2]).unwrap_or(&0.0)
        } else {
            0.0
        };
        let mom_delta = this - last;
        let mom_pct = if last > 0.0 { mom_delta / last } else { 0.0 };
        if let Value::Object(o) = &mut trends {
            o.insert("mom_delta".into(), json!(mom_delta));
            o.insert("mom_pct".into(), json!(mom_pct));
        }
        Ok(trends)
    }

    // ---- budgets (v0.3) ----

    /// The saved per-category budgets as `{ budgets: [{category, limit}] }`, joining
    /// the category name. Empty list when none.
    fn get_budgets(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let ws = data.workspace().to_string();
        let budgets = data
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT cat.name, b.amount
                     FROM fin_budgets b
                     JOIN fin_categories cat ON cat.id = b.category_id
                     WHERE b.workspace = ?1 ORDER BY b.id",
                )?;
                let rows = stmt.query_map(params![ws], |r| {
                    Ok(json!({ "category": r.get::<_, String>(0)?, "limit": r.get::<_, f64>(1)? }))
                })?;
                rows.collect::<rusqlite::Result<Vec<Value>>>()
            })
            .map_err(|e| e.to_string())?;
        Ok(json!({ "budgets": budgets }))
    }

    /// The budget list as a plain array (for aggregation).
    fn budgets_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_budgets(data)
            .ok()
            .and_then(|b| b.get("budgets").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    // ── W2 agent-friendly wrappers: single-item upserts over the existing typed
    //    save handlers (which take the full list). They read current state, merge,
    //    and save — so "budget groceries at 400" works from chat. ──

    fn agent_set_budget(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or("category is required")?
            .to_string();
        let limit = coerce_amount(args.get("limit")).abs();
        let current = self.get_budgets(data)?;
        let mut budgets: Vec<Value> = current
            .get("budgets")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|b| b.get("category").and_then(|c| c.as_str()) != Some(category.as_str()))
            .collect();
        budgets.push(json!({ "category": category, "limit": limit }));
        self.do_save_budgets(data, json!({ "budgets": budgets }))
    }

    fn agent_add_account(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("name is required")?
            .to_string();
        let typ = args.get("type").and_then(|v| v.as_str()).unwrap_or("cash");
        let opening = coerce_amount(args.get("opening_balance"));
        let current = self.get_accounts(data)?;
        let mut accounts: Vec<Value> = current
            .get("accounts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.get("name").and_then(|n| n.as_str()) != Some(name.as_str()))
            .collect();
        accounts.push(json!({ "name": name, "type": typ, "opening_balance": opening }));
        self.do_save_accounts(data, json!({ "accounts": accounts }))
    }

    fn agent_set_goal(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("name is required")?
            .to_string();
        let kind = args.get("kind").and_then(|v| v.as_str()).unwrap_or("savings");
        let account = args.get("account").and_then(|v| v.as_str()).unwrap_or("");
        let target = coerce_amount(args.get("target")).abs();
        let target_date = args.get("target_date").and_then(|v| v.as_str()).unwrap_or("");
        let current = self.get_goals(data)?;
        let mut goals: Vec<Value> = current
            .get("goals")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|g| g.get("name").and_then(|n| n.as_str()) != Some(name.as_str()))
            .collect();
        goals.push(json!({
            "name": name, "kind": kind, "account": account,
            "target": target, "target_date": target_date
        }));
        self.do_save_goals(data, json!({ "goals": goals }))
    }

    fn agent_categorize(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        let ids: Vec<i64> = args
            .get("ids")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_i64()).collect())
            .unwrap_or_default();
        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .ok_or("category is required")?
            .to_string();
        let mut updated = 0u64;
        for id in ids {
            if self
                .do_update_transaction(data, json!({ "id": id, "category": category }))
                .is_ok()
            {
                updated += 1;
            }
        }
        Ok(json!({ "updated": updated, "category": category }))
    }

    /// Persist per-category budgets (replace-set). Keeps only entries with a
    /// non-empty `category` and a positive `limit` (coerced from number/string).
    fn do_save_budgets(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let budgets: Vec<Value> = match args.get("budgets").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|b| {
                    let category = b
                        .get("category")
                        .and_then(|v| v.as_str())?
                        .trim()
                        .to_string();
                    let limit = coerce_amount(b.get("limit")).abs();
                    if category.is_empty() || limit <= 0.0 {
                        return None;
                    }
                    Some(json!({ "category": category, "limit": limit }))
                })
                .collect(),
            None => Vec::new(),
        };
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            // Replace-set semantics: clear this workspace's budgets, then insert.
            c.execute("DELETE FROM fin_budgets WHERE workspace = ?1", params![ws])?;
            for b in &budgets {
                let category = b["category"].as_str().unwrap_or("");
                let limit = b["limit"].as_f64().unwrap_or(0.0);
                let category_id = ensure_category_id(c, &ws, category)?;
                c.execute(
                    "INSERT INTO fin_budgets (workspace, category_id, amount) VALUES (?1, ?2, ?3)
                     ON CONFLICT(workspace, category_id) DO UPDATE SET amount = excluded.amount",
                    params![ws, category_id, limit],
                )?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "budgets": budgets, "saved_at": unix_now_pub() }))
    }

    // ---- accounts + transfers (v0.4) ----

    /// The user's accounts (`{ accounts: [{name, type, opening_balance}] }`),
    /// defaulting to a single seeded "Cash" when none are saved.
    fn get_accounts(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let ws = data.workspace().to_string();
        let accounts = data
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT name, type, opening_balance FROM fin_accounts
                     WHERE workspace = ?1 ORDER BY id",
                )?;
                let rows = stmt.query_map(params![ws], |r| {
                    Ok(json!({
                        "name": r.get::<_, String>(0)?,
                        "type": r.get::<_, String>(1)?,
                        "opening_balance": r.get::<_, f64>(2)?,
                    }))
                })?;
                rows.collect::<rusqlite::Result<Vec<Value>>>()
            })
            .map_err(|e| e.to_string())?;
        if accounts.is_empty() {
            Ok(json!({ "accounts": default_accounts() }))
        } else {
            Ok(json!({ "accounts": accounts }))
        }
    }

    fn accounts_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_accounts(data)
            .ok()
            .and_then(|a| a.get("accounts").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    /// Persist the account list (replace-set). Keeps entries with a non-empty name;
    /// type defaults to "cash", opening_balance coerced to a number (default 0).
    fn do_save_accounts(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let accounts: Vec<Value> = match args.get("accounts").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|a| {
                    let name = a.get("name").and_then(|v| v.as_str())?.trim().to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let typ = a
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("cash")
                        .to_string();
                    let opening = coerce_amount(a.get("opening_balance"));
                    Some(json!({ "name": name, "type": typ, "opening_balance": opening }))
                })
                .collect(),
            None => Vec::new(),
        };
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            // Replace-set: upsert each supplied account, then drop accounts no longer
            // listed BUT only when they have no transactions/transfers referencing
            // them (FK safety) — orphaned-but-referenced accounts are kept so their
            // money still aggregates.
            let kept: std::collections::HashSet<String> = accounts
                .iter()
                .filter_map(|a| a["name"].as_str().map(str::to_string))
                .collect();
            for a in &accounts {
                let name = a["name"].as_str().unwrap_or("");
                let typ = normalize_account_type(a["type"].as_str().unwrap_or("cash"));
                let opening = a["opening_balance"].as_f64().unwrap_or(0.0);
                c.execute(
                    "INSERT INTO fin_accounts (workspace, name, type, opening_balance)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(workspace, name) DO UPDATE SET
                       type = excluded.type, opening_balance = excluded.opening_balance",
                    params![ws, name, typ, opening],
                )?;
            }
            // Remove accounts no longer in the set, skipping any still referenced.
            let mut stmt = c.prepare(
                "SELECT id, name FROM fin_accounts WHERE workspace = ?1",
            )?;
            let existing: Vec<(i64, String)> = stmt
                .query_map(params![ws], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for (id, name) in existing {
                if kept.contains(&name) {
                    continue;
                }
                let referenced: bool = c
                    .query_row(
                        "SELECT 1 FROM fin_transactions WHERE account_id = ?1
                         UNION SELECT 1 FROM fin_transfers
                           WHERE from_account_id = ?1 OR to_account_id = ?1 LIMIT 1",
                        params![id],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                if !referenced {
                    c.execute("DELETE FROM fin_accounts WHERE id = ?1", params![id])?;
                }
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "accounts": accounts, "saved_at": unix_now_pub() }))
    }

    /// Record a transfer between two of the user's accounts. Stored in
    /// `fin_transfers`; surfaced to aggregation as a single legacy `type:transfer`
    /// record (neutral to income/expense; moved in `compute_account_balances`).
    fn do_add_transfer(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
        let amount = coerce_amount(args.get("amount")).abs();
        let from = args
            .get("from_account")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_ACCOUNT)
            .to_string();
        let to = args
            .get("to_account")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if to.is_empty() || from == to {
            return Err("a transfer needs distinct from/to accounts".to_string());
        }
        let date = args
            .get("date")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(today);
        let note = args
            .get("note")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let ws = data.workspace().to_string();
        let id = data
            .with_conn(|c| {
                let from_id = ensure_account_id(c, &ws, &from)?;
                let to_id = ensure_account_id(c, &ws, &to)?;
                c.execute(
                    "INSERT INTO fin_transfers (workspace, from_account_id, to_account_id, amount, transfer_date)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![ws, from_id, to_id, amount, date],
                )?;
                Ok(c.last_insert_rowid())
            })
            .map_err(|e| e.to_string())?;
        // Same record shape as before (the JSON store inserted exactly this).
        let record = json!({
            "type": "transfer", "amount": amount, "account": from, "to_account": to,
            "category": "transfer", "date": date, "note": note, "source": "manual",
        });
        Ok(json!({ "status": "added", "id": id, "record": record }))
    }

    // ---- goals (v0.5) ----

    /// The user's savings/debt goals (`{ goals: [{name, kind, account, target,
    /// target_date}] }`), default empty.
    fn get_goals(&self, data: &DataStore) -> Result<Value, String> {
        self.ensure_store(data)?;
        let ws = data.workspace().to_string();
        let goals = data
            .with_conn(|c| {
                let mut stmt = c.prepare(
                    "SELECT g.name, g.kind, COALESCE(a.name, ''), g.target_amount, g.target_date
                     FROM fin_goals g
                     LEFT JOIN fin_accounts a ON a.id = g.account_id
                     WHERE g.workspace = ?1 ORDER BY g.id",
                )?;
                let rows = stmt.query_map(params![ws], |r| {
                    let target_date: Option<String> = r.get(4)?;
                    Ok(json!({
                        "name": r.get::<_, String>(0)?,
                        "kind": r.get::<_, String>(1)?,
                        "account": r.get::<_, String>(2)?,
                        "target": r.get::<_, f64>(3)?,
                        "target_date": target_date.map(Value::String).unwrap_or(Value::Null),
                    }))
                })?;
                rows.collect::<rusqlite::Result<Vec<Value>>>()
            })
            .map_err(|e| e.to_string())?;
        Ok(json!({ "goals": goals }))
    }

    fn goals_vec(&self, data: &DataStore) -> Vec<Value> {
        self.get_goals(data)
            .ok()
            .and_then(|g| g.get("goals").and_then(|v| v.as_array()).cloned())
            .unwrap_or_default()
    }

    /// Persist goals (replace-set). Keeps entries with a non-empty name; kind
    /// defaults to savings, target coerced to a number.
    fn do_save_goals(&self, data: &DataStore, args: Value) -> Result<Value, String> {
        self.ensure_store(data)?;
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
        let ws = data.workspace().to_string();
        data.with_conn(|c| {
            c.execute("DELETE FROM fin_goals WHERE workspace = ?1", params![ws])?;
            for g in &goals {
                let name = g["name"].as_str().unwrap_or("");
                let kind = g["kind"].as_str().unwrap_or("savings");
                let account = g["account"].as_str().unwrap_or("");
                let target = g["target"].as_f64().unwrap_or(0.0);
                let target_date = g["target_date"].as_str();
                // Account is optional; link by name if it exists (else NULL).
                let account_id: Option<i64> = if account.is_empty() {
                    None
                } else {
                    Some(ensure_account_id(c, &ws, account)?)
                };
                c.execute(
                    "INSERT INTO fin_goals (workspace, name, kind, account_id, target_amount, target_date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![ws, name, kind, account_id, target, target_date],
                )?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        Ok(json!({ "goals": goals, "saved_at": unix_now_pub() }))
    }
}

// ---- typed-storage SQL helpers (run inside `with_conn`) ----

/// Map an arbitrary stored account-type string onto the CHECK-allowed set
/// (`checking|savings|card|cash`). Anything unrecognized → `cash`, matching the
/// JSON era's lenient default.
fn normalize_account_type(t: &str) -> &'static str {
    match t.to_lowercase().as_str() {
        "checking" => "checking",
        "savings" => "savings",
        "card" => "card",
        _ => "cash",
    }
}

/// Resolve an account id by name for this workspace, creating it (default type
/// `cash`) if missing — mirrors the JSON era where any referenced account name was
/// valid. The seeded default account is "Cash".
fn ensure_account_id(conn: &Connection, ws: &str, name: &str) -> rusqlite::Result<i64> {
    let name = if name.is_empty() { DEFAULT_ACCOUNT } else { name };
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM fin_accounts WHERE workspace = ?1 AND name = ?2",
            params![ws, name],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let typ = if name == DEFAULT_ACCOUNT { "cash" } else { "checking" };
    conn.execute(
        "INSERT INTO fin_accounts (workspace, name, type) VALUES (?1, ?2, ?3)",
        params![ws, name, typ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The category id for a known category NAME (case-insensitive), or NULL when the
/// name is "uncategorized"/empty/unknown — matching "uncategorized → no category".
fn category_id_for(conn: &Connection, ws: &str, name: &str) -> rusqlite::Result<Option<i64>> {
    if name.is_empty() || name.eq_ignore_ascii_case("uncategorized") {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM fin_categories WHERE workspace = ?1 AND name = ?2 COLLATE NOCASE",
        params![ws, name],
        |r| r.get::<_, i64>(0),
    )
    .optional()
}

/// Resolve a category id by name, CREATING it (expense type) if missing. Used where
/// a name must become a real FK target (rules, budgets, profile categories). Returns
/// None only for "uncategorized"/empty.
fn ensure_category_id(conn: &Connection, ws: &str, name: &str) -> rusqlite::Result<Option<i64>> {
    if name.is_empty() || name.eq_ignore_ascii_case("uncategorized") {
        return Ok(None);
    }
    if let Some(id) = category_id_for(conn, ws, name)? {
        return Ok(Some(id));
    }
    conn.execute(
        "INSERT OR IGNORE INTO fin_categories (workspace, name, type) VALUES (?1, ?2, 'expense')",
        params![ws, name],
    )?;
    category_id_for(conn, ws, name)
}

/// All category names for a workspace, ordered by id (insertion order).
fn category_names(conn: &Connection, ws: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT name FROM fin_categories WHERE workspace = ?1 ORDER BY id")?;
    let rows = stmt.query_map(params![ws], |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// Build the legacy-shaped `Record` set for all transactions + transfers in a
/// workspace (see `load_txn_records`).
fn load_txn_records_conn(conn: &Connection, ws: &str) -> rusqlite::Result<Vec<Record>> {
    let mut out: Vec<Record> = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT t.id, t.transaction_type, t.amount, COALESCE(cat.name, 'uncategorized'),
                t.transaction_date, COALESCE(t.merchant, ''), acc.name,
                t.source, t.import_hash, t.notes, t.created_at
         FROM fin_transactions t
         JOIN fin_accounts acc ON acc.id = t.account_id
         LEFT JOIN fin_categories cat ON cat.id = t.category_id
         WHERE t.workspace = ?1
         ORDER BY t.id",
    )?;
    let rows = stmt.query_map(params![ws], |r| {
        let id: i64 = r.get(0)?;
        let kind: String = r.get(1)?;
        let amount: f64 = r.get(2)?;
        let category: String = r.get(3)?;
        let date: String = r.get(4)?;
        let merchant: String = r.get(5)?;
        let account: String = r.get(6)?;
        let source: String = r.get(7)?;
        let import_hash: Option<String> = r.get(8)?;
        let note: String = r.get(9)?;
        let created_at: i64 = r.get(10)?;
        let mut data = json!({
            "type": kind, "amount": amount, "category": category, "date": date,
            "merchant": merchant, "account": account, "source": source, "note": note,
        });
        if let Some(h) = import_hash {
            data["import_hash"] = json!(h);
        }
        Ok(Record {
            id,
            data,
            created_at: created_at as u64,
        })
    })?;
    for r in rows {
        out.push(r?);
    }

    // Transfers become legacy `type:transfer` records carrying from→to account
    // names, exactly as `do_add_transfer` once inserted into the JSON store.
    let mut tstmt = conn.prepare(
        "SELECT tr.id, fa.name, ta.name, tr.amount, tr.transfer_date
         FROM fin_transfers tr
         JOIN fin_accounts fa ON fa.id = tr.from_account_id
         JOIN fin_accounts ta ON ta.id = tr.to_account_id
         WHERE tr.workspace = ?1
         ORDER BY tr.id",
    )?;
    let trows = tstmt.query_map(params![ws], |r| {
        let id: i64 = r.get(0)?;
        let from: String = r.get(1)?;
        let to: String = r.get(2)?;
        let amount: f64 = r.get(3)?;
        let date: String = r.get(4)?;
        Ok(Record {
            id,
            data: json!({
                "type": "transfer", "amount": amount, "account": from, "to_account": to,
                "category": "transfer", "date": date, "note": "", "source": "manual",
            }),
            created_at: 0,
        })
    })?;
    for r in trows {
        out.push(r?);
    }

    Ok(out)
}

/// Load a single transaction by id as a legacy-shaped `Record` (transfers excluded —
/// updates only touch regular transactions).
fn load_one_txn_record(conn: &Connection, ws: &str, id: i64) -> rusqlite::Result<Option<Record>> {
    conn.query_row(
        "SELECT t.id, t.transaction_type, t.amount, COALESCE(cat.name, 'uncategorized'),
                t.transaction_date, COALESCE(t.merchant, ''), acc.name,
                t.source, t.import_hash, t.notes, t.created_at
         FROM fin_transactions t
         JOIN fin_accounts acc ON acc.id = t.account_id
         LEFT JOIN fin_categories cat ON cat.id = t.category_id
         WHERE t.workspace = ?1 AND t.id = ?2",
        params![ws, id],
        |r| {
            let id: i64 = r.get(0)?;
            let kind: String = r.get(1)?;
            let amount: f64 = r.get(2)?;
            let category: String = r.get(3)?;
            let date: String = r.get(4)?;
            let merchant: String = r.get(5)?;
            let account: String = r.get(6)?;
            let source: String = r.get(7)?;
            let import_hash: Option<String> = r.get(8)?;
            let note: String = r.get(9)?;
            let created_at: i64 = r.get(10)?;
            let mut data = json!({
                "type": kind, "amount": amount, "category": category, "date": date,
                "merchant": merchant, "account": account, "source": source, "note": note,
            });
            if let Some(h) = import_hash {
                data["import_hash"] = json!(h);
            }
            Ok(Record { id, data, created_at: created_at as u64 })
        },
    )
    .optional()
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
        "You manage the user's personal finances and can DO things, not just describe them. \
         Record a transaction with add_transaction; list with query_transactions; spending \
         totals with monthly_summary — never compute totals yourself. You can also run the \
         app on the user's behalf: set_budget (a category's monthly limit), add_account, \
         set_goal (savings/debt), add_category_rule (teach 'merchant contains X → category Y' \
         so future entries auto-categorize), categorize_transactions (bulk-fix categories by \
         id after query_transactions), and add_transfer. When the user asks to set a budget, \
         add an account, create a goal, or fix categories, CALL THE TOOL — don't just explain \
         how. Categories must be ones that exist; if unsure, infer the closest existing one. \
         When the user asks to open/show a panel or dashboard, pass target=\"canvas\"; \
         otherwise omit it (defaults to inline).\n\n\
         Inbuilt multi-step workflows — run the whole sequence in one turn (don't stop after \
         the first tool):\n\
         - Import & categorize a statement: for each line item call add_transaction with an \
         inferred category, then query_transactions and monthly_summary for the month so the \
         user can review; offer to add_category_rule for any merchant you had to guess.\n\
         - Monthly review: monthly_summary for the month, then query_transactions for it, and \
         finish with a short written takeaway."
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
            // ── W2: the agent can now DO what the UI tabs do (not just talk). ──
            GenaiTool::new("set_budget")
                .with_description("Set or update the monthly spending limit for ONE category (e.g. 'budget groceries at 400'). Upserts — replaces an existing limit for that category.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "An existing expense category" },
                        "limit": { "type": "number", "description": "Monthly limit, a positive number" }
                    },
                    "required": ["category", "limit"]
                })),
            GenaiTool::new("add_account")
                .with_description("Add one of the user's accounts (checking/savings/card/cash) with an optional opening balance.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string", "enum": ["checking", "savings", "card", "cash"], "description": "Defaults to cash" },
                        "opening_balance": { "type": "number", "description": "Current/opening balance; defaults to 0" }
                    },
                    "required": ["name"]
                })),
            GenaiTool::new("set_goal")
                .with_description("Create or update a savings or debt-payoff goal funded by one account.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "kind": { "type": "string", "enum": ["savings", "debt"], "description": "Defaults to savings" },
                        "account": { "type": "string", "description": "Which account funds/tracks the goal" },
                        "target": { "type": "number", "description": "Target amount" },
                        "target_date": { "type": "string", "description": "YYYY-MM-DD (optional)" }
                    },
                    "required": ["name", "target"]
                })),
            GenaiTool::new("add_category_rule")
                .with_description("Teach the app to always categorize transactions whose merchant contains a phrase (e.g. 'STARBUCKS' → dining). Future imports/entries auto-apply it.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "merchant_contains": { "type": "string", "description": "Case-insensitive substring of the merchant" },
                        "category": { "type": "string", "description": "An existing category to assign" }
                    },
                    "required": ["merchant_contains", "category"]
                })),
            GenaiTool::new("categorize_transactions")
                .with_description("Set the category on one or more transactions by id (bulk recategorize). Use after query_transactions to fix categories.")
                .with_schema(json!({
                    "type": "object",
                    "properties": {
                        "ids": { "type": "array", "items": { "type": "integer" }, "description": "Transaction ids to update" },
                        "category": { "type": "string", "description": "The category to assign to all of them" }
                    },
                    "required": ["ids", "category"]
                })),
        ]
    }

    fn dispatch_tool(
        &self,
        data: &DataStore,
        name: &str,
        args: Value,
    ) -> Option<Result<AppResult, String>> {
        let target = target_of(&args);
        match name {
            "add_transaction" => Some(self.do_add_transaction(data, args).map(AppResult::Data)),
            "query_transactions" => {
                Some(
                    self.compute_transactions(data, args)
                        .map(|d| AppResult::Block {
                            component_id: "transactions_table".to_string(),
                            data: d,
                            target,
                        }),
                )
            }
            "monthly_summary" => {
                Some(
                    self.compute_monthly_summary(data, args)
                        .map(|d| AppResult::Block {
                            component_id: "monthly_summary".to_string(),
                            data: d,
                            target,
                        }),
                )
            }
            "update_transaction" => {
                Some(self.do_update_transaction(data, args).map(AppResult::Data))
            }
            "delete_transaction" => {
                Some(self.do_delete_transaction(data, args).map(AppResult::Data))
            }
            "add_transfer" => Some(self.do_add_transfer(data, args).map(AppResult::Data)),
            // W2 — the agent runs the app:
            "set_budget" => Some(self.agent_set_budget(data, args).map(AppResult::Data)),
            "add_account" => Some(self.agent_add_account(data, args).map(AppResult::Data)),
            "set_goal" => Some(self.agent_set_goal(data, args).map(AppResult::Data)),
            "add_category_rule" => {
                Some(self.do_save_category_rule(data, args).map(AppResult::Data))
            }
            "categorize_transactions" => {
                Some(self.agent_categorize(data, args).map(AppResult::Data))
            }
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
            "category_rules" => self
                .get_category_rules(data)
                .map(|rules| json!({ "rules": rules })),
            // The category list (for the UI's category pickers). Returns a bare
            // JSON array of names — what FinanceV1's `categories` query expects.
            "categories" => Ok(Value::Array(
                self.profile_categories(data)
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            )),
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
            // FinanceV1's bulk-recategorize calls this via runAppAction (the UI
            // path), the same handler the agent's tool uses.
            "categorize_transactions" => self.agent_categorize(data, args),
            "migrate_transactions" => {
                // No-op since W5: storage is typed from the first write, so there is
                // nothing to backfill. Kept for action-name compatibility.
                self.ensure_store(data)?;
                Ok(json!({ "migrated": 0 }))
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
/// known list wins (normalized to the stored casing); else the first merchant rule
/// whose `merchant_contains` is a case-insensitive substring of the merchant; else
/// "uncategorized".
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
        let sub = rule
            .get("merchant_contains")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let cat = rule.get("category").and_then(|v| v.as_str()).unwrap_or("");
        if !sub.is_empty() && !cat.is_empty() && ml.contains(&sub.to_lowercase()) {
            return cat.to_string();
        }
    }
    "uncategorized".to_string()
}

/// Read the latest value stored under `key` in a latest-wins singleton JSON store
/// (widgets only, post-W5). Returns the `key` field of the row with the greatest
/// `saved_at`, or None when the store is empty.
fn latest_singleton(data: &DataStore, store: &str, key: &str) -> Option<Value> {
    data.create_store(store).ok()?;
    let rows = data.query(store, &Query::default()).ok()?;
    rows.into_iter()
        .max_by_key(|r| r.data.get("saved_at").and_then(|v| v.as_u64()).unwrap_or(0))
        .and_then(|r| r.data.get(key).cloned())
}

/// Persist `value` under `key` in a singleton JSON store (widgets only, post-W5),
/// UPDATING the existing row in place (and pruning older duplicates).
fn save_singleton(data: &DataStore, store: &str, key: &str, value: Value) -> Result<Value, String> {
    data.create_store(store).map_err(|e| e.to_string())?;
    let record = json!({ key: value, "saved_at": unix_now_pub() });
    let rows = data
        .query(store, &Query::default())
        .map_err(|e| e.to_string())?;
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

// `Query` is still used by the widgets singleton helpers.
use zanto_core::data::Query;

#[cfg(test)]
mod tests;
