# Archived agent worktree branches — 2026-06-18

48 single-commit agent feature branches (work consolidated on main).
Recover any with: git branch <name> <sha>  (also in git reflog ~90 days).

| branch | tip sha | subject |
|---|---|---|
| worktree-agent-a02a8c7eca230512d | 1fad3d2 | design(chat): apply Workbench system + agent-activity spine |
| worktree-agent-a08a7aa98c0a38566 | fe11ec9 | feat(workspace): inputs/outputs surface — project dir + toggleable context sources |
| worktree-agent-a107201967235de5f | e2ec6ae | feat(artifacts): downloadable documents — Save a copy + Reveal in folder |
| worktree-agent-a15abb4597f265053 | 115c3ca | D3: session summarization (ContextPolicy::Summarize + summarize_messages) |
| worktree-agent-a194d03d93a562dbc | 0b6ffce | C8: session-persistent infinite scroll for chat history |
| worktree-agent-a20aae8bfe6c4c0be | 7b73679 | fix(chat): thinking block holds reasoning + working narration; thinking & tool blocks collapsed by default |
| worktree-agent-a22ae27f1a4c55d04 | 1bdb452 | F2+F6: finance onboarding + inbuilt workflows |
| worktree-agent-a27005be99445b763 | 7af6a23 | feat(artifacts): browser lists files + pinned views; prompt to set a project dir |
| worktree-agent-a2878a65e5faa2be2 | 2ff0d7c | D2: session archive (+ keep delete) |
| worktree-agent-a2f9313ecdf8c8d97 | 69a5244 | fix(sidebar): app-switch loading + race guard + paginated session list (infinite scroll) |
| worktree-agent-a389fc53e0a2b9c8d | 4ae3ee0 | design(dialogs): apply Workbench system to settings, HITL, and artifact overlays |
| worktree-agent-a3b206fc321a24b56 | fd6b48b | design(finance): Workbench polish — primitives, states, segmented tabs, save/cancel |
| worktree-agent-a3c43167f4b4603ab | 230dc83 | fix(chat): persistent thinking/working block for the whole turn (no vanish on first chunk) |
| worktree-agent-a3ce5aa91dc1950a2 | d4d8ddd | B2: chat decomposition + segment model |
| worktree-agent-a3e19af53c07f44dc | d96bcab | design(shell): apply Workbench system to sidebar + canvas |
| worktree-agent-a44148b4eade8d14b | 617173b | feat(desktop): native polish — window-state, single-instance, os, turn/approval notifications |
| worktree-agent-a4bd280be4db3716e | 9d32ca1 | A1: provider/model/keys foundation |
| worktree-agent-a552d0eba185fbafe | 9219ad0 | fix(chat): reasoning-only thinking block + inline tool calls + persist full turn (reasoning/tools/stopped) |
| worktree-agent-a5bbe292c585c6ca3 | 7d5156a | feat(artifacts): page document artifact (multi-section) + Page.svelte |
| worktree-agent-a5f2c8770791a7f83 | b34ce75 | F1+F5: finance dashboard + quick starts |
| worktree-agent-a5fd028d36d617caa | 1caba3b | C5: tool-call UI block — compact card with status pill, collapsible args/output |
| worktree-agent-a61696962a2d28784 | 6f081cd | A5: structured streaming segments via ChatSink |
| worktree-agent-a61dad0b3cc3f2914 | 160bb0f | A3: artifact store + markdown-as-artifact |
| worktree-agent-a6b01e1c4652d409b | 708854b | A4: context sources + skills/preprompt loader |
| worktree-agent-a70ac4733e5dce0b9 | 0425f28 | C7: file @-tag picker + slash-command menu in composer |
| worktree-agent-a7cef9d6bdbc8fe13 | 5e74c83 | design(blocks): apply Workbench system to block/artifact components |
| worktree-agent-a7e858a6792475886 | aaa7af4 | feat(artifacts): classify artifacts (storage: view\|file) + clarify render vs store |
| worktree-agent-a80182c79fdb38fc0 | ab7c0cc | A2: session archive/summary/per-message metadata + system_info |
| worktree-agent-a83e03060626cf38f | 36ff941 | feat(tools): read_document — extract text from pdf/docx/xlsx/csv/html/text |
| worktree-agent-a8cd66a58cc422c3d | 4d84f7d | E4: global artifact browser (list/read IPC + browser dialog) |
| worktree-agent-a8dc64e0c3ea19265 | 3237973 | D1: persist + restore session artifacts (component blocks) |
| worktree-agent-a8fbd68ba8c5ccb4a | dd13dce | fix(artifacts): render charts as pure SVG (drop Chart.js — reliable in WebKitGTK) |
| worktree-agent-a9e38262150ad8a5f | 25e4ed3 | C6: multi-loop workflow view (group consecutive tool calls) |
| worktree-agent-aa6273f703d379760 | 54e234d | feat(chat): attach files (button + drag-drop) as @path references |
| worktree-agent-aa9927dcec1b6ccac | 127e959 | E1: chart artifact (Chart.js) |
| worktree-agent-aad2e3d756e0a1a17 | 5e9d19b | fix(ui): chart CSS-token render race + /clear only when composer non-empty |
| worktree-agent-ab19ded0b9fc4b87a | 0a95908 | E2: markdown-preview artifact |
| worktree-agent-abcca319045ec7178 | 0d75c9e | B3: provider/keys/model settings UI |
| worktree-agent-ac0b00ba01d636889 | b5ff4ca | F3+F4: finance resources panel + widget/dashboard builder |
| worktree-agent-ac6e3e4cb3354e797 | 7372e26 | E3: web browsing tool (fetch_url) in zanto-core |
| worktree-agent-ac8b4b4cd5df01447 | fa128e4 | C3: inline turn-error surface with retry |
| worktree-agent-ad7e0168b174f8d4c | 93b117e | fix(desktop): wire context sources + user skills + summarization policy |
| worktree-agent-adb1734388ee56879 | f86cf73 | feat(chat): turn interruption (Stop) + message queue |
| worktree-agent-adb8c29cdea44ab7f | 3701c59 | C1: bottom-stacked chat thread + jump-to-latest + role/markdown polish |
| worktree-agent-add7d5b7249a3c846 | 011c530 | C2: copy buttons + paste-expander chips |
| worktree-agent-ae0c57b46396066a6 | d4e9834 | feat(artifacts): pin view+data artifacts to the DB (pin_artifact + read commands) |
| worktree-agent-ae37db7dad3da6f28 | a710cf5 | feat(chat): image attachments as vision input on multimodal providers (graceful degrade otherwise) |
| worktree-agent-aec7aa5b4e6f6ea88 | 5f22afe | feat(panel): links + artifact browser in the canvas panel; user pin button |
