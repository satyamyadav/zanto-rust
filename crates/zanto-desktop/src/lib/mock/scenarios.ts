import type { ChatTurn } from "$lib/ipc";

export type ScenarioEvent = { event: string; payload: unknown };
// `blocking`: if true, send_message parks after emitting events and waits for
// interrupt_turn — simulating a long-running turn that the user stops early.
export type Scenario = { trigger: string; events: ScenarioEvent[]; response: ChatTurn; blocking?: boolean };

const chartBlock = {
  kind: "component",
  component_id: "chart",
  // ApexCharts schema confirmed working in the checklist (R-1). Align with Chart.svelte if it expects a different shape.
  data: { type: "bar", labels: ["Mon", "Tue", "Wed"], datasets: [{ data: [120, 200, 150], label: "Weekly Values" }] },
  target: "inline",
};

const summaryBlock = {
  kind: "component",
  component_id: "monthly_summary",
  // Shape to match monthly_summary.svelte — discover the real fields and adjust.
  data: { income: 2000, spent: 12.5, net: 1987.5, by_category: { dining: 12.5 } },
  target: "inline",
};

// Default: plain markdown stream (mirrors the original send_message.json behavior).
export const defaultScenario: Scenario = {
  trigger: "",
  events: [
    { event: "chat_chunk", payload: { text: "Hi " } },
    { event: "chat_chunk", payload: { text: "there." } },
    { event: "chat_done", payload: null },
  ],
  response: { blocks: [{ kind: "markdown", text: "Hi there." }] },
};

export const scenarios: Scenario[] = [
  { trigger: "chart with toolcall", events: [
      { event: "chat_tool_call", payload: { id: "t1", name: "render_artifact", args: { id: "chart", target: "inline" } } },
      { event: "chat_block", payload: { block: chartBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [chartBlock as any] } },
  { trigger: "chart", events: [
      { event: "chat_block", payload: { block: chartBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [chartBlock as any] } },
  { trigger: "finance summary", events: [
      { event: "chat_block", payload: { block: summaryBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [summaryBlock as any] } },
  // Emits one chunk so streamIdx is set (enabling the Stopped marker), then parks
  // until interrupt_turn is called — simulating a turn stopped mid-stream.
  { trigger: "silent stop", blocking: true, events: [
      { event: "chat_chunk", payload: { text: "" } },
    ], response: { blocks: [] } },
];

/** Pick the first scenario whose trigger is a case-insensitive substring of the message, else default. */
export function pickScenario(text: string): Scenario {
  const t = text.toLowerCase();
  return scenarios.find((s) => s.trigger && t.includes(s.trigger)) ?? defaultScenario;
}
