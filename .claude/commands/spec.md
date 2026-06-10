Write an implementation spec. Do NOT edit source files or write code.

**Model:** opus — planning work only.

---

## What to do

1. **Explore** — read relevant source files and grep for affected symbols to ground the spec. Do not speculate about code you haven't read.

2. **Clarify before speccing** — surface any decisions you'd otherwise guess: scope boundaries, behaviour on edge cases, which modules are affected, whether new deps are needed. Ask as concrete questions; fold answers into the spec. If the request is unambiguous, skip this step.

3. **Write the spec** to `docs/specs/YYYY-MM-DD-<slug>.md` (use today's date):

```markdown
# <Title>

- **Date:** YYYY-MM-DD

## Summary
One sentence.

## Motivation
Why this change is needed.

## Scope
In scope and explicitly out of scope.

## Affected files
- `path/to/file.rs` — reason

## Implementation steps
Numbered, each atomic (one file or one concern).

1. **Step title** (`path/to/file.rs`)
   - What to change and why; reference specific struct/function/trait.

## Edge cases & risks

## Acceptance criteria
User-observable outcomes traced through the whole call path — not implementation details.
- [ ] Criterion

## Manual test plan
Exact `cargo run` invocations and expected terminal output.
1. `cargo run -p zanto-cli -- "..."` → expected output
```

4. **Show the spec inline** and ask: "Approve this spec, or tell me what to change."

5. **Wait for approval.** No code until the user explicitly approves.

---

## Rules

- Steps must be granular enough for independent review.
- Do not write around assumptions — ask instead.
- Flag any new crate dependency under Edge cases & risks.
- Acceptance criteria must be verifiable by running the CLI, not by reading code.
