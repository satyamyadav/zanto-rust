<script lang="ts">
  import DashboardTab from "./v1/DashboardTab.svelte";
  import TransactionsTab from "./v1/TransactionsTab.svelte";
  import AccountsTab from "./v1/AccountsTab.svelte";
  import ImportTab from "./v1/ImportTab.svelte";
  import LayoutDashboard from "@lucide/svelte/icons/layout-dashboard";
  import ListChecks from "@lucide/svelte/icons/list-checks";
  import Landmark from "@lucide/svelte/icons/landmark";
  import Upload from "@lucide/svelte/icons/upload";

  type Tab = "dashboard" | "transactions" | "accounts" | "import";

  let tab = $state<Tab>("dashboard");
  // Filter the Transactions tab opens with (set by the dashboard's nudge).
  let txFilter = $state<"all" | "uncategorized">("all");

  function reviewUncategorized() {
    txFilter = "uncategorized";
    tab = "transactions";
  }

  const TABS: { id: Tab; label: string; Icon: typeof LayoutDashboard }[] = [
    { id: "dashboard", label: "Dashboard", Icon: LayoutDashboard },
    { id: "transactions", label: "Transactions", Icon: ListChecks },
    { id: "accounts", label: "Accounts", Icon: Landmark },
    { id: "import", label: "Import", Icon: Upload },
  ];

  function tabClass(active: boolean): string {
    return [
      "inline-flex shrink-0 items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
      active
        ? "bg-background text-foreground shadow-sm ring-1 ring-primary"
        : "text-muted-foreground hover:text-foreground",
    ].join(" ");
  }
</script>

<div class="space-y-4">
  <div
    role="tablist"
    aria-label="Finance"
    class="inline-flex items-center gap-1 overflow-x-auto rounded-lg border border-border bg-muted/40 p-0.5"
  >
    {#each TABS as t (t.id)}
      <button
        type="button"
        role="tab"
        aria-selected={tab === t.id}
        class={tabClass(tab === t.id)}
        onclick={() => (tab = t.id)}
      >
        <t.Icon class="size-4" />
        {t.label}
      </button>
    {/each}
  </div>

  {#if tab === "dashboard"}
    <DashboardTab onReviewUncategorized={reviewUncategorized} />
  {:else if tab === "transactions"}
    {#key txFilter}
      <TransactionsTab initialFilter={txFilter} />
    {/key}
  {:else if tab === "accounts"}
    <AccountsTab />
  {:else if tab === "import"}
    <ImportTab />
  {/if}
</div>
