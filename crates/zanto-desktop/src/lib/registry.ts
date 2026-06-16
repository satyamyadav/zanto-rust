import type { Component } from "svelte";
import TransactionsTable from "./apps/finance/transactions_table.svelte";
import MonthlySummary from "./apps/finance/monthly_summary.svelte";

// component_id -> Svelte component. Apps own their components; unknown ids fall
// back to a raw-JSON view in Block.svelte.
export const componentRegistry: Record<string, Component<{ data: any }>> = {
  transactions_table: TransactionsTable,
  monthly_summary: MonthlySummary,
};
