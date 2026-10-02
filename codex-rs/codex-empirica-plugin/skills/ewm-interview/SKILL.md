---
name: ewm-interview
description: "Use when the user says '/ewm-interview', 'run EWM interview', 'create workflow protocol', 'set up my workflow', 'interview me for EWM', or wants to create a personalized AI collaboration protocol. Guided multiple-choice interview that produces workflow-protocol.yaml, summarises it as a readable working agreement, provisions the practices it implies, and lays them out as ecodex instances in an empirica cockpit."
version: 0.3.0
---

# EWM Interview — Epistemic Workflow Manager

Produce a **workflow-protocol.yaml** describing how this person wants to work with
AI — then actually set up what it implies: the practices their work needs, and a
cockpit that opens one ecodex in each.

## The rule that governs the whole interview

**Ask with options, never with a blank.** Every question offers concrete, pickable
options. "What are your main constraints?" asked into a void gets a shrug or a
paragraph you can't map to a field. The same question as a handful of options gets an
answer in one pick, and the options themselves TEACH what the field means.

**How to ask in ecodex.** Use `request_user_input` where the current mode offers it:
up to three questions per call, two or three options per question, the recommended
option first with "(Recommended)" at the end of its label. The client adds a
free-form "Other" option itself — don't add one. Where the tool isn't available in
the current mode, ask the same questions in one message as short numbered options and
let the user answer with numbers; "pick all that apply" works there for the
multi-answer questions.

Three rules for the options you write:

- **Make every option genuinely pickable.** An option nobody could choose is filler;
  it makes the list look considered while narrowing the real choice.
- **Put a recommendation first** when there is a sane default, and say so:
  `"Collaborative — check in on approach (Recommended)"`.
- **Multi-answer questions** (domains, tools, non-negotiables) either go through the
  plain-text form, or become a narrower question with the three likeliest options —
  "Other" still catches the rest.

Batch related questions. Six or seven calls cover the whole interview.

---

## Phase 1 — Goals

| Ask | Shape | Options to offer |
|---|---|---|
| What are you mainly trying to accomplish right now? | single | Ship a product / feature · Research & understand a problem space · Grow an organisation or practice · Operate & maintain something live |
| What does "done" look like for that? | single | A shipped, working thing · A decision I can defend · A repeatable process · Learning I can reuse |
| What is your binding constraint? | multi | Time · Money / headcount · Missing knowledge · Regulatory / compliance · Dependence on other people |
| Horizon? | single | This week · This quarter · This year · Open-ended |

Capture into `goals.primary[]` (description, success_criteria, timeline) and
`goals.secondary[]`.

## Phase 2 — Domains & expertise

| Ask | Shape | Options |
|---|---|---|
| Which domains are you expert in? | multi | Domains inferred from the repository and their answers so far, plus generic ones |
| Which are you actively learning? | multi | same list |
| Where do you want AI to carry the most weight? | multi | same list |

**Infer the option list — don't ask people to type their own field.** Read the
repository first: languages present, `docs/` topics, the project's own
`.empirica/project.yaml`. Offer what you found. This is the single biggest friction
reduction in the interview.

Capture `domains.expert[]`, `domains.learning[]`, `domains.novice[]`.

## Phase 3 — Tools & connections

| Ask | Shape | Options |
|---|---|---|
| Which of these do you actually use daily? | multi | Detected MCP servers + GitHub / Forgejo · Slack · Google Drive · Linear / Jira · Notion |
| Where does your code live? | single | GitHub · Forgejo (self-hosted) · GitLab · Local only |
| Is this practice on the Cortex mesh? | single | Yes — registered · Yes — should be, not yet · No, standalone |

**Detect before asking.** Run `ecodex mcp list` (or read the `[mcp_servers.*]`
tables in `~/.codex/config.toml`) and offer what is configured rather than a generic
menu. Mark detected entries so the user is confirming, not recalling.

The last two questions are load-bearing — they feed Phase 6.

## Phase 4 — Work preferences

| Ask | Shape | Options |
|---|---|---|
| How should we split the work? | single | Equal partners — check in on approach (Recommended) · You lead, I execute · I run autonomously, you review outcomes |
| When should I act without asking? | multi | Research & investigation · Code implementation once you have said go · Refactors & cleanup · Anything reversible |
| What must ALWAYS wait for you? | multi | Anything sent outside the org · Architecture with business impact · Spending money · Legal / contractual · Deleting things |
| When I think you are wrong? | single | Direct and factual, no hedging (Recommended) · Gentle reframe · Socratic questions |

Capture `work_preferences.*` and the three `task_splitting` lists.

Note the asymmetry: **"act without asking" is a floor, "always wait" is a
ceiling.** If an item appears in both, the ceiling wins — say so rather than
silently resolving it.

## Phase 5 — Trust & non-negotiables

| Ask | Shape | Options |
|---|---|---|
| How should trust start? | single | Start collaborative, widen as it is earned (Recommended) · Start restricted · Start open, pull back if needed |
| What earns more autonomy from you? | multi | Accuracy that holds up · Flagging its own gaps · Catching problems unprompted · Admitting mistakes fast |
| Absolute non-negotiables? | multi | Never act against my interests · Never hide uncertainty behind agreeable language · Never take irreversible actions unasked · Never send anything externally without approval |

Capture `trust_building.*`.

---

## Phase 6 — Provision what the answers imply

An interview that produces a YAML file and stops has described a setup rather than
performed one.

`empirica provision-practice` does the whole chain, idempotently: create the
directory → `project-init` → patch `.empirica/project.yaml` (ai_id / tenant / org /
substrate) → `project-register` with Cortex → optional Forgejo backup remote. Safe to
re-run; every step no-ops if already done.

From Phases 1–3, propose the practices their answers imply — a separate practice per
distinct domain of work, which is the model Empirica is built on (a practice is an
epistemic specialization, not a folder).

**Decide where they live before proposing them.** `provision-practice` creates
practices under `~/empirica` unless told otherwise. Look at the current practice's
parent directory: if its siblings are practices too (they have a `.empirica/`
folder), that is where this person keeps practices, and new ones belong beside them.

> Where should the new practices live? · Beside this one, in `<parent dir>`
> (Recommended when the siblings are practices) · `~/empirica` (provision-practice's
> default)

**Always dry-run first, and show the output before doing anything:**

```bash
empirica provision-practice <name> [--base-path <dir>] --dry-run --output json
```

Then ask — with options, like everything else:

> Provision these? — `empirica-research`, `empirica-outreach`
> · Yes, both · Just the first · Let me adjust the names

Only on an affirmative:

```bash
empirica provision-practice <name> \
  --tenant <tenant> --org <org> \
  [--base-path <dir>] \
  [--forgejo-owner <owner> --forgejo-host <ssh-url>] \
  [--no-cortex]
```

Where the flags come from the answers:

| Answer | Flag |
|---|---|
| Practices live beside this one | `--base-path <parent dir>` |
| Practices live in `~/empirica` | omit `--base-path` |
| Code lives on Forgejo | `--forgejo-owner` + `--forgejo-host` (ask for both — they are not guessable) |
| Code lives on GitHub / GitLab / local | omit the Forgejo flags |
| Not on the mesh / standalone | `--no-cortex` |
| On the mesh | omit `--no-cortex`; `--tenant` / `--org` default from the current directory's project.yaml |

These commands write outside the workspace, so ecodex asks for approval before each
one runs. Say that before the first one, so the prompt is expected.

**Keep each practice's `proj_dir`** from the JSON output. It is the directory the
practice was actually created in, and Phase 7 uses it as-is.

**Report per practice what actually happened** — provisioned, already existed, or
failed and why. A rollup "done!" over a partial failure is the shape this whole
system exists to prevent.

---

## Phase 7 — Open one ecodex per practice

A provisioned practice is a folder. To work in it the user needs an ecodex running
there, one per practice, each with its own history and calibration. The empirica
cockpit does that in one command: it opens a terminal layout with one pane per
practice, each starting ecodex in its own directory.

Skip this phase when Phase 6 left no practices (none provisioned and none already
existed), or when `empirica cockpit --help` fails: that empirica has no cockpit.

**Detect before asking.**

- **Existing profiles:** `ls ~/.empirica/cockpit/config*.yaml`. A profile is one
  `config-NAME.yaml`, launched with `--profile NAME`. Never overwrite one. Propose a
  name that is free (`ecodex-<slug of their primary goal>`); if they want to replace
  an existing profile, show what it holds first.
- **Terminal:** `command -v ghostty`, then `command -v alacritty`. Either one gives
  the layout its own window, which they can pin to a taskbar slot. Without both,
  the surface is `tmux` and the layout opens in the terminal they launch it from.

Then ask:

> Open these practices as ecodex instances? — profile `ecodex-launch`, 2 practices,
> one ghostty window · Yes, write the profile (Recommended) · Change the name or
> layout · Not now

**The draft.** A cockpit profile is short, so show it in full before writing:

```yaml
# ~/.empirica/cockpit/config-<profile>.yaml — written by the EWM interview
session_name: cockpit-<profile>      # unique per profile
surface: ghostty                     # detected: ghostty | alacritty | tmux
attach_on_launch: true               # only matters for the tmux surface

projects:
  - name: <ai_id>
    path: <proj_dir from Phase 6>
    launch: bash -ic 'ecodex; exec bash'
  - name: <ai_id 2>
    path: <proj_dir from Phase 6>
    launch: bash -ic 'ecodex; exec bash'

groups:
  - name: <domain>
    split: horizontal                # side by side; vertical stacks them
    panes:
      - {project: <ai_id>}
      - {project: <ai_id 2>}
```

What each choice is for:

- **One project per practice, with `path` copied from `proj_dir`.** Never rebuild
  the path from the name; a practice that was already there may live elsewhere.
- **`launch: bash -ic 'ecodex; exec bash'`.** `-i` loads the user's shell profile,
  which is usually what puts `empirica` on `PATH`. `exec bash` leaves a shell behind
  when ecodex exits, so the pane stays open; `ecodex resume --last` picks the
  session back up in place.
- **No `instance_id:`.** ecodex sets its own practitioner identity, its thread id,
  for every shell and hook it runs, so an id bound by the cockpit would be replaced
  anyway.
- **Two panes per group.** Each group is one window or tab. With more than four
  practices, make more groups and name each after the domain it holds.

**Write only on a yes**, to `~/.empirica/cockpit/config-<profile>.yaml`. The file
lives outside the workspace, so the write asks for approval. Then check that the
cockpit can read what you wrote. This parses the profile without launching
anything:

```bash
empirica cockpit status --profile <profile> --output json
```

It must return `ok: true` with an empty `problems` list, and `configured_projects`
must list every practice with the right path. `status` catches a YAML error, a pane
naming a project that isn't under `projects:`, a group without a name, and a group
left with no usable pane; it exits non-zero and lists them under `problems`. A
practice directory that doesn't exist yet is only a `warnings` entry. If `ok` is
false or a practice is missing, show the problems and fix the draft. Don't report
success over it.

On an empirica older than 1.14.5, `status` does not look inside `groups:` and its
output has no `ok` field. There, read the file back and match every
`{project: …}` pane against a `name:` under `projects:` yourself.

**Hand the launch to the user.** Launching opens a new window or takes over a
terminal, and the user should be the one to do that. Give them the commands:

```bash
empirica cockpit launch --profile <profile>   # bring it up (attaches if it is already running)
empirica cockpit kill   --profile <profile>   # tear it down
```

The rest (refresh, surfaces, recovering after a crash) is in empirica's
`docs/guides/COCKPIT.md`.

---

## Output

### Where to write it — ask, don't assume

The loader searches **the project directory first, then `~/.empirica/`**. That
ordering has a consequence worth stating out loud, because it is silent:

> **A project-local protocol SHADOWS the user's global one for that project.**

That is a feature when someone wants different working agreements on a client
repository, and a trap when they meant to update their global profile and quietly
stopped using it everywhere else.

So ask, with the default first:

> Where should this live? · `~/.empirica/workflow-protocol.yaml` — applies everywhere
> (Recommended) · This project only — overrides your global protocol here

If they pick project-local and a global one already exists, **say what will be
shadowed** before writing.

### Shape

```yaml
# Epistemic Workflow Protocol
# Generated by EWM Interview v0.3.0 — {date}

user_profile:
  name: "{name}"
  role: "{role}"
  created: "{date}"
  last_updated: "{date}"

goals:
  primary:
    - description: "{goal}"
      success_criteria: ["{criterion}"]
      timeline: "{timeline}"
  secondary: ["{goal}"]

domains:
  expert: ["{domain}"]
  learning: ["{domain}"]
  novice: ["{domain}"]

tools:
  {category}: "{name}"        # Maps to: {mcp_server}

mesh:                          # Phase 3 — omit the block entirely if standalone
  cortex: true
  tenant: "{tenant}"
  org: "{org}"
  forgejo:
    owner: "{owner}"
    host: "{ssh_url}"

practices:                     # Phase 6 — what was actually provisioned
  - name: "{ai_id}"
    path: "{proj_dir}"
    provisioned: true
    domain: "{domain}"

cockpit:                       # Phase 7 — omit the block if no profile was written
  profile: "{profile}"
  config: "~/.empirica/cockpit/config-{profile}.yaml"
  surface: "{ghostty|alacritty|tmux}"

work_preferences:
  ai_autonomy_level: "{autonomous|collaborative_with_checkpoints|assistant_mode}"
  uncertainty_surfacing: "{always_explicit|when_material|minimal}"
  pushback_style: "{direct_and_factual|gentle_reframe|socratic}"
  task_splitting:
    ai_autonomous: ["{task}"]
    ai_with_checkpoint: ["{task}"]
    human_only: ["{task}"]

trust_building:
  current_level: "{establishing|building|established|high_trust}"
  autonomy_earned_through: ["{demonstration}"]
  non_negotiables: ["{boundary}"]

modules:
  active: []
  available: []
```

Omit blocks with nothing in them. An empty `mesh:` or `cockpit:` block is a claim
that the step was performed, which would be false.

### Show it as a working agreement, not raw YAML

Don't paste 80 lines of YAML into the terminal and ask "does this look right?" —
nobody proofreads YAML there. Summarise it as the working agreement in plain prose,
grouped by section (goals, domains, how we split work, what always waits for you,
non-negotiables), short enough to read in a minute. Mention where the full YAML will
be written so anyone who wants the raw form can open it.

Then ask for corrections **before** writing the file, with options:

> · Looks right, save it · Change my autonomy settings · Change the non-negotiables

---

## After saving

1. **Save** to the location chosen above.
2. **Report**, per practice, what Phase 6 provisioned and where, and which cockpit
   profile Phase 7 wrote. Say plainly if a step was skipped or failed.
3. **Log it** — `empirica finding-log` with what the protocol covers (N goals,
   N domains, N practices provisioned, cockpit profile written or not).
4. **Say how to change it** — run the interview again, or edit the YAML directly; it
   is theirs and it is plain text. The cockpit profile is plain YAML too.
5. **End with the launch command** from Phase 7, when a profile was written. It is
   the next thing they will do.

---

## Interview discipline

- **5–10 minutes, not 30.** If you are on your eighth round of questions, you are
  interviewing rather than helping.
- **Infer before asking.** Repository languages, configured MCP servers, an existing
  `project.yaml`, existing cockpit profiles, the terminals on `PATH` — every one of
  those is a question you don't have to ask.
- **Never ask what you can detect.** Confirming is cheap; recalling is not.
- **Hedging is a signal, not an answer.** "It's complicated", "kind of", "I guess"
  is CONTEXTUAL pushback — ask a narrower question with narrower options rather than
  accepting the vagueness into a field. Don't mirror hedged language back.
- **Hold structure against emotional pushback; update on genuine new context.** Full
  framing in the `epistemic-persistence-protocol` skill.
- Once Phase 4 captures `pushback_style`, **use it for the rest of the interview.**
  The protocol starts applying the moment it is known.

## Design principles

1. **Options over blanks** — the option list teaches the field.
2. **Detect over ask** — inference is the friction reduction that matters.
3. **Provision, don't describe** — Phases 6 and 7 are why this exists: the interview
   ends with practices on disk and a cockpit ready to open them.
4. **Transparent** — they see it, own it, and can edit it as plain text.
5. **Evolvable** — re-runnable. `provision-practice` is idempotent, and Phase 7 never
   overwrites an existing profile, so re-running is safe.
