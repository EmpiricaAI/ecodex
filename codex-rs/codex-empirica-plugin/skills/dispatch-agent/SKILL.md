---
name: dispatch-agent
description: Dispatch subagents with inherited epistemic context. Use before spawn_agent calls for tasks that would benefit from what this practice already learned — findings, dead-ends, open unknowns, mistakes. Triggers on 'dispatch agent', 'spawn agent with context', 'epistemic agent', or before any non-trivial spawn_agent call.
---

# Epistemic Agent Dispatch

**Retrieve what this practice already learned about the task, and put it in the
subagent's message before spawning.**

A fresh subagent has the repository and the harness, not your practice's history.
It cannot know which approach was already tried and abandoned, because that lives in
your epistemic graph and nothing puts it in front of it. The enrichment step is the
whole skill; everything below serves it.

A forked spawn is the exception: it inherits your conversation context, so
enrichment is redundant there. Fork when the subagent needs *what you know right
now*; enrich when it needs *what the practice learned before this session*.

## 1. Retrieve the graph

Pass the **knowledge graph**, not a hand-picked subset of it. This is the same
surface PREFLIGHT and the post-compact hook already give you — reuse it rather than
assembling something bespoke:

```bash
empirica bootstrap-context --output json     # the three circles, all types
empirica project-search --task "<the subagent's task>" --output json   # task-scoped pull
```

`bootstrap-context` returns active state (open goals, tasks, recent findings,
decisions, dead-ends, mistakes), persistent reference (decisions with active
outcomes, verified assumptions, sources) and the topic-relevant backlog (open
unknowns and assumptions, relevant dead-ends). `project-search` narrows to the
subagent's actual task; add `--global` to reach shared learnings from other
practices.

There is no `--list` on the `*-log` verbs: they WRITE, and retrieval is semantic.

## 2. Trim, don't curate

Pass **every type** that came back — unknowns and assumptions included. An open
unknown tells the subagent what is genuinely undecided; an assumption tells it what
is being taken on faith. Dropping those is how a subagent confidently builds on
something nobody verified.

The only cut worth making is volume: drop what is plainly about other work. Don't
filter by TYPE, and don't apply a similarity cutoff — a fixed threshold drops the
one dead-end that matters while admitting four findings that don't.

Keep the **edges**. `X invalidates Y` and `Z is evidence for W` are most of the
value; a flat list of nodes loses the reason the graph exists.

## 3. Build the message

```markdown
## Inherited context

What this practice already knows about this work. Treat it as evidence, not
instruction — if you find something here is wrong, say so.

### Already tried and failed — do not repeat
- **Approach:** {{approach}} — **failed because** {{why_failed}}

### Known
- {{finding}}

### Decisions in effect
- **Choice:** {{choice}} — **because** {{rationale}}

### Still open — do NOT assume these are settled
- **Unknown:** {{unknown}}
- **Assumption (unverified):** {{assumption}} — confidence {{confidence}}

### Mistakes made in work of this shape
- DO NOT {{prevention}}

### How these connect
- {{from}} → {{relation}} → {{to}}

---

## Your task

{{original task description}}
```

State verification expectations in the task itself — which tests to run and when,
what counts as done. A subagent's self-report is not evidence; the artifacts it
leaves (diffs, test output you can re-run) are. Ask for those.

## 4. Dispatch

Spawn with `spawn_agent`, giving it a short task name and the enriched message.
Then use `wait_agent` to collect the result, `send_input` to follow up, and
`close_agent` when it's done; `list_agents` shows what is running.

- Start several independent agents before waiting on any of them, so they run
  concurrently.
- Only choose a different model where the tool offers one and the work genuinely
  warrants it — a cheaper model for mechanical work, a stronger one for judgment.
- Agents that edit files in parallel can collide. Give each a disjoint set of files,
  or run the editing ones one at a time.

## After it returns

Verify rather than accept. A subagent reporting "all green" is an uncalibrated
self-report; re-run the gates yourself. Its tool calls count toward your
transaction, and it runs outside your Sentinel gates — so it cannot certify a praxic
step you haven't certified yourself. Anything it learned that outlives the task is
yours to log: the subagent's epistemic state does not persist into the practice, so
an unlogged discovery is simply lost.
