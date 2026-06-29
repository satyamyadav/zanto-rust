import type { Component } from "svelte";
// Generic shared catalogue
import Table from "./blocks/Table.svelte";
import Metric from "./blocks/Metric.svelte";
import Chart from "./blocks/Chart.svelte";
import Markdown from "./blocks/Markdown.svelte";
import List from "./blocks/List.svelte";
import KeyValue from "./blocks/KeyValue.svelte";
import Json from "./blocks/Json.svelte";
import Nba from "./blocks/Nba.svelte";
import Page from "./blocks/Page.svelte";
import Html from "./blocks/Html.svelte";
// App-specific
import TransactionsTable from "./apps/finance/transactions_table.svelte";
import MonthlySummary from "./apps/finance/monthly_summary.svelte";

// The shared block catalogue: component_id -> Svelte component. Three tiers —
// generic (reusable), app-specific, and a chat-specific slot. Unknown ids fall
// back to the Json block in Block.svelte.
export const componentRegistry: Record<string, Component<{ data: any }>> = {
  // generic
  table: Table,
  metric: Metric,
  chart: Chart,
  markdown: Markdown,
  list: List,
  kv: KeyValue,
  json: Json,
  nba: Nba,
  page: Page,
  html: Html,
  // app-specific (finance)
  transactions_table: TransactionsTable,
  monthly_summary: MonthlySummary,
  // chat-specific: (slot)
};
