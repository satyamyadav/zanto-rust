import type { ChatTurn } from "$lib/ipc";

export type ScenarioEvent = { event: string; payload: unknown };
// `blocking`: if true, send_message parks after emitting events and waits for
// interrupt_turn — simulating a long-running turn that the user stops early.
export type Scenario = { trigger: string; events: ScenarioEvent[]; response: ChatTurn; blocking?: boolean; throws?: boolean };

const chartBlock = {
  kind: "component",
  component_id: "chart",
  // ApexCharts schema confirmed working in the checklist (R-1). Align with Chart.svelte if it expects a different shape.
  data: { type: "bar", labels: ["Mon", "Tue", "Wed"], datasets: [{ data: [120, 200, 150], label: "Weekly Values" }] },
  target: "inline",
};

// A self-contained interactive HTML page, target=canvas. The inline script wires
// a counter button (proves allow-scripts runs) and attempts a fetch (which the
// injected CSP must block — proving no network egress).
const htmlBlock = {
  kind: "component",
  component_id: "html",
  data: {
    title: "Sandbox demo",
    // Adversarially shaped: a COMMENTED <head> + a self-supplied CSP meta in body
    // position — both would have defeated the old regex injection. The fixed
    // wrapper ignores agent markup entirely, so network must still be blocked.
    content: `<!-- <head> --><meta http-equiv="Content-Security-Policy" content="default-src *">
<h2>Counter</h2>
<button id="b">clicked 0</button>
<p id="net">network: pending…</p>
<script>
  let n = 0;
  const b = document.getElementById('b');
  b.onclick = () => { n++; b.textContent = 'clicked ' + n; };
  fetch('https://example.com')
    .then(() => document.getElementById('net').textContent = 'network: ALLOWED (bad!)')
    .catch(() => document.getElementById('net').textContent = 'network: blocked ✓');
<\/script>`,
  },
  target: "canvas",
};

const summaryBlock = {
  kind: "component",
  component_id: "monthly_summary",
  // Shape matches monthly_summary.svelte: month string, income, total (not spent),
  // net, and by_category as an array of { category, total } objects.
  data: {
    month: "June 2026",
    income: 2000,
    total: 12.5,
    net: 1987.5,
    by_category: [{ category: "Dining", total: 12.5 }],
  },
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
  { trigger: "html page", events: [
      { event: "chat_block", payload: { block: htmlBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [htmlBlock as any] } },
  // Emits one chunk so streamIdx is set (enabling the Stopped marker), then parks
  // until interrupt_turn is called — simulating a turn stopped mid-stream.
  { trigger: "silent stop", blocking: true, events: [
      { event: "chat_chunk", payload: { text: "" } },
    ], response: { blocks: [] } },
  // throws: one-shot error (first attempt throws, retry recovers via defaultScenario).
  { trigger: "trigger error", throws: true, events: [], response: { blocks: [] } },
  // partial stop: emits one chunk then parks until interrupted.
  { trigger: "partial stop", blocking: true, events: [
      { event: "chat_chunk", payload: { text: "Partial answer so far" } },
    ], response: { blocks: [] } },
  // think: reasoning + tool call/result + final chunk.
  { trigger: "think", events: [
      { event: "chat_reasoning", payload: { text: "Considering options" } },
      { event: "chat_tool_call", payload: { id: "t1", name: "read_file", args: { path: "/x" } } },
      { event: "chat_tool_result", payload: { id: "t1", output: "ok", ok: true } },
      { event: "chat_chunk", payload: { text: "Done." } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [{ kind: "markdown", text: "Done." }] } },
  // workflow: two tool calls in sequence + final chunk.
  { trigger: "workflow", events: [
      { event: "chat_tool_call", payload: { id: "w1", name: "list_directory", args: { path: "/" } } },
      { event: "chat_tool_result", payload: { id: "w1", output: "a\nb", ok: true } },
      { event: "chat_tool_call", payload: { id: "w2", name: "read_file", args: { path: "/a" } } },
      { event: "chat_tool_result", payload: { id: "w2", output: "hello", ok: true } },
      { event: "chat_chunk", payload: { text: "Done." } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [{ kind: "markdown", text: "Done." }] } },
  // link: a URL in the text for link-promotion tests.
  { trigger: "link", events: [
      { event: "chat_chunk", payload: { text: "See https://example.com for details." } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [{ kind: "markdown", text: "See https://example.com for details." }] } },
  // file-path: assistant text with backticked absolute path, relative path, and bare prose slash.
  // Used by C-14 tests to verify path-linkification rules.
  { trigger: "file path test", events: [
      { event: "chat_chunk", payload: { text: "See `/home/user/project/src/main.rs` for details. Also `src/relative.rs` and /bare/prose/path." } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [{ kind: "markdown", text: "See `/home/user/project/src/main.rs` for details. Also `src/relative.rs` and /bare/prose/path." }] } },
  // hitl form: emits an interaction_request (kind "form") so HitlForm renders.
  // Resolves with empty blocks; the respond mock handler records the submission.
  { trigger: "hitl form", events: [
      { event: "interaction_request", payload: {
          id: "req-1",
          kind: "form",
          title: "Tell me about your project",
          steps: [
            { fields: [{ name: "name", label: "Project name", type: "text" }] },
            { fields: [{ name: "lang", label: "Language", type: "select", options: ["rust", "ts"] }] },
          ],
        } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [] } },
];

/** Pick the first scenario whose trigger is a case-insensitive substring of the message, else default. */
export function pickScenario(text: string): Scenario {
  const t = text.toLowerCase();
  return scenarios.find((s) => s.trigger && t.includes(s.trigger)) ?? defaultScenario;
}
