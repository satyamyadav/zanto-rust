//! Finance app tests. Two layers:
//! - Pure aggregate-function + resolver tests (no storage) — unchanged behavior.
//! - Typed-storage integration tests over the real `App` trait (query/action) with
//!   a temp `DataStore`, plus schema integrity (FK/CHECK) checks.

use super::*;
use crate::app::App;
use std::sync::Arc;
use zanto_core::data::{DataStore, Record};
use zanto_core::rusqlite::params;
use tempfile::TempDir;

// ---- pure aggregate / resolver tests (storage-free) ----

#[test]
fn balance_is_income_minus_expense_with_legacy_default() {
    // income − expense, and a legacy row (no `type`) counts as an expense.
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
    let defaults = default_widgets();
    for w in defaults.as_array().unwrap() {
        let kind = w.get("kind").and_then(|v| v.as_str()).unwrap();
        assert!(WIDGET_KINDS.contains(&kind), "default widget kind '{kind}' is not saveable");
    }
}

#[test]
fn forecast_projects_from_run_rate() {
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
    assert_eq!(f["projected_net_worth"], json!(4300.0)); // 5000 + 0 − (1000−300)
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

    let (status2, over2, _) = compute_budget_status(&budgets, &HashMap::new(), 1.0);
    assert_eq!(status2[0]["spent"], json!(0.0));
    assert!(over2.is_empty());
}

#[test]
fn pace_warning_when_on_track_to_exceed() {
    let budgets = vec![json!({ "category": "dining", "limit": 200 })];
    let mut spent = HashMap::new();
    spent.insert("dining".to_string(), 160.0);
    let (status, over, pace) = compute_budget_status(&budgets, &spent, 0.6);
    assert!(over.is_empty());
    assert_eq!(pace.len(), 1);
    assert_eq!(status[0]["on_track_to_exceed"], json!(true));
}

#[test]
fn orphaned_account_money_is_not_lost() {
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

// ---- typed-storage integration tests ----

fn app_and_store() -> (Arc<dyn App>, DataStore, TempDir) {
    let dir = TempDir::new().unwrap();
    let ds = DataStore::open_at(&dir.path().join("test.db"), "/ws").unwrap();
    (FinanceApp::new(), ds, dir)
}

#[test]
fn schema_created_and_seeds_default_categories() {
    let (app, ds, _dir) = app_and_store();
    // First access ensures schema + seeds.
    app.query(&ds, "profile", json!({})).unwrap();
    let cats = ds
        .with_conn(|c| category_names(c, "/ws"))
        .unwrap();
    for d in schema::DEFAULT_EXPENSE_CATEGORIES {
        assert!(cats.iter().any(|n| n == d), "missing seeded category {d}");
    }
    assert!(cats.iter().any(|n| n == "income"));
}

#[test]
fn fk_rejects_orphan_transaction() {
    let (app, ds, _dir) = app_and_store();
    app.query(&ds, "profile", json!({})).unwrap(); // ensure schema
    // account_id 9999 doesn't exist → FK violation.
    let err = ds.with_conn(|c| {
        c.execute(
            "INSERT INTO fin_transactions
               (workspace, account_id, category_id, amount, transaction_type,
                transaction_date, created_at)
             VALUES ('/ws', 9999, NULL, 10.0, 'expense', '2026-06-01', 0)",
            params![],
        )
    });
    assert!(err.is_err(), "orphan account_id must be rejected by FK");
}

#[test]
fn check_rejects_bad_transaction_type() {
    let (app, ds, _dir) = app_and_store();
    app.query(&ds, "profile", json!({})).unwrap();
    let acc_id: i64 = ds
        .with_conn(|c| {
            c.execute(
                "INSERT INTO fin_accounts (workspace, name, type) VALUES ('/ws','A','cash')",
                params![],
            )?;
            Ok(c.last_insert_rowid())
        })
        .unwrap();
    let err = ds.with_conn(|c| {
        c.execute(
            "INSERT INTO fin_transactions
               (workspace, account_id, category_id, amount, transaction_type,
                transaction_date, created_at)
             VALUES ('/ws', ?1, NULL, 10.0, 'bogus', '2026-06-01', 0)",
            params![acc_id],
        )
    });
    assert!(err.is_err(), "invalid transaction_type must be rejected by CHECK");
}

#[test]
fn add_transaction_returns_legacy_shape_and_categorizes() {
    let (app, ds, _dir) = app_and_store();
    let out = app
        .action(&ds, "add_transaction", json!({ "amount": 42.5, "merchant": "DMart", "category": "groceries", "date": "2026-06-10" }))
        .unwrap();
    assert_eq!(out["status"], "added");
    let rec = &out["record"];
    assert_eq!(rec["type"], "expense");
    assert_eq!(rec["amount"], 42.5);
    assert_eq!(rec["category"], "groceries");
    assert_eq!(rec["merchant"], "DMart");
    assert_eq!(rec["account"], "Cash");

    // An unknown category falls back to uncategorized.
    let out2 = app
        .action(&ds, "add_transaction", json!({ "amount": 5, "category": "zzz-unknown" }))
        .unwrap();
    assert_eq!(out2["record"]["category"], "uncategorized");
}

#[test]
fn query_transactions_filters_by_category_and_month() {
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "add_transaction", json!({ "amount": 10, "category": "groceries", "date": "2026-06-01" })).unwrap();
    app.action(&ds, "add_transaction", json!({ "amount": 20, "category": "dining", "date": "2026-06-02" })).unwrap();
    app.action(&ds, "add_transaction", json!({ "amount": 30, "category": "groceries", "date": "2026-05-15" })).unwrap();

    let by_cat = app.query(&ds, "list_transactions", json!({ "category": "groceries" })).unwrap();
    assert_eq!(by_cat["rows"].as_array().unwrap().len(), 2);

    let by_month = app.query(&ds, "list_transactions", json!({ "month": "2026-06" })).unwrap();
    assert_eq!(by_month["rows"].as_array().unwrap().len(), 2);
    // Rows carry an id for edit/delete.
    assert!(by_month["rows"][0]["id"].is_i64());
}

#[test]
fn update_and_delete_transaction_round_trip() {
    let (app, ds, _dir) = app_and_store();
    let added = app.action(&ds, "add_transaction", json!({ "amount": 10, "category": "dining" })).unwrap();
    let id = added["id"].as_i64().unwrap();

    let upd = app.action(&ds, "update_transaction", json!({ "id": id, "amount": 99, "type": "income" })).unwrap();
    assert_eq!(upd["status"], "updated");
    assert_eq!(upd["record"]["amount"], 99.0);
    assert_eq!(upd["record"]["type"], "income");

    let del = app.action(&ds, "delete_transaction", json!({ "id": id })).unwrap();
    assert_eq!(del["status"], "deleted");
    let rows = app.query(&ds, "list_transactions", json!({})).unwrap();
    assert!(rows["rows"].as_array().unwrap().is_empty());
}

#[test]
fn full_round_trip_overview_categorized() {
    let (app, ds, _dir) = app_and_store();
    // save_profile → save_accounts → add_transaction → overview.
    app.action(&ds, "save_profile", json!({ "currency": "GBP", "monthly_income": 3000, "categories": ["groceries", "rent"] })).unwrap();
    app.action(&ds, "save_accounts", json!({ "accounts": [{ "name": "Checking", "type": "checking", "opening_balance": 1000 }] })).unwrap();

    let m = today_month();
    app.action(&ds, "add_transaction", json!({ "type": "income", "amount": 3000, "account": "Checking", "date": format!("{m}-01") })).unwrap();
    app.action(&ds, "add_transaction", json!({ "amount": 200, "category": "groceries", "account": "Checking", "date": format!("{m}-05") })).unwrap();
    app.action(&ds, "add_transaction", json!({ "amount": 800, "category": "rent", "account": "Checking", "date": format!("{m}-03") })).unwrap();

    let ov = app.query(&ds, "overview", json!({})).unwrap();
    assert_eq!(ov["empty"], false);
    assert_eq!(ov["income"], 3000.0);
    assert_eq!(ov["month_total"], 1000.0); // 200 + 800
    assert_eq!(ov["net_cash_flow"], 2000.0);
    assert_eq!(ov["balance"], 2000.0); // 3000 income − 1000 expense
    assert_eq!(ov["net_worth"], 3000.0); // opening 1000 + 3000 − 1000

    // Top categories reflect the typed→legacy reconstruction.
    let top = ov["top_categories"].as_array().unwrap();
    let rent = top.iter().find(|c| c["category"] == "rent").unwrap();
    assert_eq!(rent["total"], 800.0);
    let groc = top.iter().find(|c| c["category"] == "groceries").unwrap();
    assert_eq!(groc["total"], 200.0);

    // monthly_summary agrees.
    let ms = app.query(&ds, "monthly_summary", json!({ "month": m })).unwrap();
    assert_eq!(ms["total"], 1000.0);
    assert_eq!(ms["income"], 3000.0);
}

#[test]
fn category_rules_resolve_on_add() {
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "save_category_rule", json!({ "merchant_contains": "starbucks", "category": "dining" })).unwrap();
    let rules = app.query(&ds, "category_rules", json!({})).unwrap();
    assert_eq!(rules["rules"][0]["merchant_contains"], "starbucks");
    assert_eq!(rules["rules"][0]["category"], "dining");

    // A txn at STARBUCKS with no category → rule applies.
    let out = app.action(&ds, "add_transaction", json!({ "amount": 6, "merchant": "STARBUCKS #5" })).unwrap();
    assert_eq!(out["record"]["category"], "dining");
}

#[test]
fn budgets_and_goals_round_trip_shapes() {
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "save_budgets", json!({ "budgets": [{ "category": "dining", "limit": 200 }] })).unwrap();
    let b = app.query(&ds, "budgets", json!({})).unwrap();
    assert_eq!(b["budgets"][0]["category"], "dining");
    assert_eq!(b["budgets"][0]["limit"], 200.0);

    app.action(&ds, "save_accounts", json!({ "accounts": [{ "name": "Savings", "type": "savings", "opening_balance": 0 }] })).unwrap();
    app.action(&ds, "save_goals", json!({ "goals": [{ "name": "Emergency", "kind": "savings", "account": "Savings", "target": 1000 }] })).unwrap();
    let g = app.query(&ds, "goals", json!({})).unwrap();
    assert_eq!(g["goals"][0]["name"], "Emergency");
    assert_eq!(g["goals"][0]["kind"], "savings");
    assert_eq!(g["goals"][0]["account"], "Savings");
    assert_eq!(g["goals"][0]["target"], 1000.0);
}

#[test]
fn import_dedupes_and_reports_counts() {
    let (app, ds, _dir) = app_and_store();
    let args = json!({
        "headers": ["Date", "Description", "Amount"],
        "rows": [
            ["2026-06-01", "Cafe", "-12.50"],
            ["2026-06-01", "Cafe", "-12.50"], // dup within batch
            ["2026-06-02", "Payroll", "2000"],
        ],
        "mapping": { "date": "Date", "merchant": "Description", "amount": "Amount" },
        "account": "Checking",
    });
    let r1 = app.action(&ds, "import_transactions", args.clone()).unwrap();
    assert_eq!(r1["inserted"], 2);
    assert_eq!(r1["skipped"], 1);

    // Re-running the same import is a no-op (dedupe against existing rows).
    let r2 = app.action(&ds, "import_transactions", args).unwrap();
    assert_eq!(r2["inserted"], 0);
    assert_eq!(r2["skipped"], 3);
}

#[test]
fn add_transfer_neutral_to_balance_moves_between_accounts() {
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "save_accounts", json!({ "accounts": [
        { "name": "Checking", "type": "checking", "opening_balance": 1000 },
        { "name": "Card", "type": "card", "opening_balance": 0 },
    ] })).unwrap();
    let out = app.action(&ds, "add_transfer", json!({ "amount": 200, "from_account": "Checking", "to_account": "Card" })).unwrap();
    assert_eq!(out["status"], "added");
    assert_eq!(out["record"]["type"], "transfer");

    let ov = app.query(&ds, "overview", json!({})).unwrap();
    let accts = ov["accounts"].as_array().unwrap();
    let bal = |n: &str| accts.iter().find(|a| a["name"] == n).unwrap()["balance"].as_f64().unwrap();
    assert_eq!(bal("Checking"), 800.0);
    assert_eq!(bal("Card"), 200.0);
    assert_eq!(ov["net_worth"], 1000.0); // transfer nets to zero
}

// ---- W2: agent tools run the app (via dispatch_tool, the tool-call path) ----

#[test]
fn agent_tools_set_budget_account_goal_and_categorize() {
    let (app, ds, _dir) = app_and_store();
    let m = today_month();

    // add_account via the agent tool → persists + shows up in accounts.
    app.dispatch_tool(&ds, "add_account", json!({ "name": "Savings", "type": "savings", "opening_balance": 500 }))
        .unwrap()
        .unwrap();
    let accts = app.query(&ds, "accounts", json!({})).unwrap();
    let names: Vec<&str> = accts["accounts"].as_array().unwrap().iter()
        .map(|a| a["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"Savings"), "add_account tool should persist the account");

    // set_budget via the agent tool → upserts one category's limit.
    app.dispatch_tool(&ds, "set_budget", json!({ "category": "groceries", "limit": 400 }))
        .unwrap().unwrap();
    app.dispatch_tool(&ds, "set_budget", json!({ "category": "groceries", "limit": 450 }))
        .unwrap().unwrap(); // re-set replaces, not duplicates
    let budgets = app.query(&ds, "budgets", json!({})).unwrap();
    let groc: Vec<&Value> = budgets["budgets"].as_array().unwrap().iter()
        .filter(|b| b["category"] == "groceries").collect();
    assert_eq!(groc.len(), 1, "set_budget should upsert, not duplicate");
    assert_eq!(groc[0]["limit"], 450.0);

    // set_goal via the agent tool.
    app.dispatch_tool(&ds, "set_goal", json!({ "name": "Rainy day", "kind": "savings", "account": "Savings", "target": 5000 }))
        .unwrap().unwrap();
    let goals = app.query(&ds, "goals", json!({})).unwrap();
    assert!(goals["goals"].as_array().unwrap().iter().any(|g| g["name"] == "Rainy day"));

    // categorize_transactions: add an uncategorized txn (on a NEW account via the
    // agent tool — don't replace-set accounts, which would FK-block deleting the
    // goal-referenced "Savings"), then bulk-categorize it by id.
    app.dispatch_tool(&ds, "add_account", json!({ "name": "Checking", "type": "checking", "opening_balance": 0 }))
        .unwrap().unwrap();
    let added = app.dispatch_tool(&ds, "add_transaction", json!({ "amount": 43, "merchant": "MYSTERY LLC", "account": "Checking", "date": format!("{m}-10") }))
        .unwrap().unwrap();
    let id = match added { AppResult::Data(v) => v["id"].as_i64(), _ => None }.unwrap();
    app.dispatch_tool(&ds, "categorize_transactions", json!({ "ids": [id], "category": "groceries" }))
        .unwrap().unwrap();
    let list = app.query(&ds, "list_transactions", json!({})).unwrap();
    let row = list["rows"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap();
    assert_eq!(row["category"], "groceries", "categorize_transactions should set the category");
}

#[test]
fn overview_emits_financev1_fields() {
    let (app, ds, _dir) = app_and_store();
    let m = today_month();
    app.action(&ds, "save_profile", json!({ "currency": "AED", "monthly_income": 3000 })).unwrap();
    app.action(&ds, "save_accounts", json!({ "accounts": [{ "name": "Checking", "type": "checking", "opening_balance": 0 }] })).unwrap();
    app.action(&ds, "add_transaction", json!({ "type": "income", "amount": 3000, "account": "Checking", "date": format!("{m}-01") })).unwrap();
    app.action(&ds, "add_transaction", json!({ "amount": 200, "category": "groceries", "account": "Checking", "date": format!("{m}-05") })).unwrap();

    let ov = app.query(&ds, "overview", json!({})).unwrap();
    // The additive FinanceV1 fields exist with the right shapes.
    assert_eq!(ov["currency"], "AED");
    assert_eq!(ov["spent"], 200.0);
    assert_eq!(ov["net"], 2800.0); // 3000 − 200
    assert!(ov["safe_to_spend"].as_f64().unwrap() >= 0.0);
    assert!(ov["monthly"].as_array().unwrap().len() == 6, "6-month cashflow series");
    // each monthly entry has income/spend/savings
    let last = ov["monthly"].as_array().unwrap().last().unwrap();
    assert!(last.get("income").is_some() && last.get("spend").is_some() && last.get("savings").is_some());
    assert!(ov["trend_months"].as_array().unwrap().len() == 6);
    assert!(ov["category_breakdown"].is_array());
    assert!(ov["subscriptions"].is_array());
}

// ---- review fixes: subscriptions key, categorize honesty, transfer id safety ----

#[test]
fn overview_subscriptions_populated_from_recurring() {
    // Regression: compute_overview read `.get("recurring")` but compute_recurring
    // returns `{items:[...]}`, so subscriptions were ALWAYS empty on the real backend.
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "save_accounts", json!({ "accounts": [{ "name": "Checking", "type": "checking", "opening_balance": 0 }] })).unwrap();
    // ≥3 charges across ≥3 consecutive months with a ~monthly (25–35d) gap → recurring.
    for date in ["2026-03-03", "2026-04-03", "2026-05-03"] {
        app.action(&ds, "add_transaction", json!({ "amount": 9.99, "merchant": "Netflix", "account": "Checking", "date": date })).unwrap();
    }
    let ov = app.query(&ds, "overview", json!({})).unwrap();
    let subs = ov["subscriptions"].as_array().unwrap();
    assert!(
        subs.iter().any(|s| s["merchant"] == "Netflix"),
        "subscriptions must surface the detected recurring charge (was always [] before the key fix)"
    );
}

#[test]
fn categorize_reports_resolved_not_requested_category() {
    // Regression: agent_categorize returned the REQUESTED category and counted every
    // row as updated, even when an unknown category silently fell back to uncategorized.
    let (app, ds, _dir) = app_and_store();
    let m = today_month();
    app.dispatch_tool(&ds, "add_account", json!({ "name": "Checking", "type": "checking", "opening_balance": 0 })).unwrap().unwrap();
    let added = app.dispatch_tool(&ds, "add_transaction", json!({ "amount": 5, "merchant": "X", "account": "Checking", "date": format!("{m}-10") })).unwrap().unwrap();
    let id = match added { AppResult::Data(v) => v["id"].as_i64(), _ => None }.unwrap();

    // A bogus category with no matching rule resolves to "uncategorized".
    let res = match app.dispatch_tool(&ds, "categorize_transactions", json!({ "ids": [id], "category": "not_a_real_category" })).unwrap().unwrap() {
        AppResult::Data(v) => v,
        _ => panic!("expected data"),
    };
    assert_eq!(res["updated"], 0, "no row actually landed in the bogus category");
    let list = app.query(&ds, "list_transactions", json!({})).unwrap();
    let row = list["rows"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap();
    assert_eq!(row["category"], "uncategorized", "row stays uncategorized, not falsely reported");
}

#[test]
fn transfer_rows_carry_namespaced_id() {
    // Regression: transfers (separate AUTOINCREMENT table) surfaced in the txn list
    // with a bare numeric id that could collide with a fin_transactions id; the UI
    // could then update/delete the wrong row. Transfer ids are now strings ("tr:N").
    let (app, ds, _dir) = app_and_store();
    app.action(&ds, "save_accounts", json!({ "accounts": [
        { "name": "Checking", "type": "checking", "opening_balance": 1000 },
        { "name": "Card", "type": "credit", "opening_balance": 0 },
    ] })).unwrap();
    app.action(&ds, "add_transfer", json!({ "amount": 200, "from_account": "Checking", "to_account": "Card" })).unwrap();
    let list = app.query(&ds, "list_transactions", json!({})).unwrap();
    let transfer = list["rows"].as_array().unwrap().iter().find(|r| r["type"] == "transfer").unwrap();
    assert!(transfer["id"].is_string(), "transfer id must be a namespaced string, not a bare integer");
    assert!(transfer["id"].as_str().unwrap().starts_with("tr:"));
    // And a transaction keeps its integer id (editable).
    assert!(list["rows"].as_array().unwrap().iter().filter(|r| r["type"] != "transfer").all(|r| r["id"].is_i64()));
}

/// Current month as `YYYY-MM` (matches the app's `today()`), for month-scoped tests.
fn today_month() -> String {
    today()[..7].to_string()
}
