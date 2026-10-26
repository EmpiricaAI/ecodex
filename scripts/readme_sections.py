#!/usr/bin/env python3
"""Render README's "What's New" and "What's Coming" sections, or check that they are current.

What's New is the newest released CHANGELOG entry's author-written `### Highlights` (short lines);
the detail stays in CHANGELOG.md and what is left out is counted and linked, never dropped silently.
What's Coming renders docs/ROADMAP.yaml, a forecast whose items each carry a confidence, the kind of
evidence it rests on, and the observation that would falsify it.

    python3 scripts/readme_sections.py                  # rewrite the two sections in README.md
    python3 scripts/readme_sections.py --check          # exit 1 if stale, or the roadmap is invalid
    python3 scripts/readme_sections.py --preview X.Y.Z  # as releasing [Unreleased] would render

Ported from empirica core's scripts/release.py (MIT License, Copyright (c) 2024-2026 Empirica Team),
so both READMEs follow the same rules: evidence kinds cap confidence, internal ids are refused, a
released forecast cannot be edited without a dated revision, and only the span between the What's
New heading and the next top-level heading is replaced.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from datetime import date
from pathlib import Path

PROJECT = "ecodex"
REPO_ROOT = Path(__file__).resolve().parent.parent

_WHATS_NEW_FALLBACK_MAX = 6
_WHATS_NEW_EARLIER = 2
_SUMMARY_MAX = 160
_ROADMAP_TEXT_FIELDS = ("id", "title", "prediction", "basis", "falsifier")
_ROADMAP_FIELDS = (*_ROADMAP_TEXT_FIELDS, "confidence", "evidence", "status")
_ROADMAP_STATUSES = ("predicted", "shipped", "dropped", "retracted", "superseded")
_RELEASE_NUMBER = re.compile(r"^\d+\.\d+\.\d+$")
_REVISION_KEYS = ("on", "from", "to", "reason")
# Fields whose silent change would let a forecast be adjusted after the fact. Changing one needs a
# dated revision saying why.
_ROADMAP_GUARDED = ("confidence", "prediction", "falsifier", "horizon", "evidence")
# How the basis is known caps the confidence it can carry: a figure a command produced can support
# 0.95, a report from someone else (testimony about code that may have changed since) no more than
# 0.7, and an inference no more than 0.6.
_ROADMAP_EVIDENCE_CEILING = {"measured": 0.95, "reviewed": 0.85, "reported": 0.7, "inferred": 0.6}
# The roadmap is public text. Ids of proposals, records, goals, tasks and commits are internal
# coordinates a reader cannot resolve. A hex run counts as a commit hash only if it mixes letters
# and digits, so dates, counts and ordinary words pass.
_INTERNAL_ID = re.compile(
    r"\b(?:prop|ser)[_-][0-9a-z]{6,}|\b(?:goal|task)[ _-][0-9a-f]{6,}\b"
    r"|\b(?=[0-9a-f]*\d)(?=[0-9a-f]*[a-f])[0-9a-f]{7,40}\b",
    re.IGNORECASE,
)
_NOT_A_SENTENCE_END = r"(?<!e\.g)(?<!i\.e)(?<!\bvs)(?<!\betc)(?<!\bDr)(?<!\bMr)(?<!\bMrs)(?<!\bSt)"
_SENTENCE = re.compile(r"(.+?" + _NOT_A_SENTENCE_END + r"[.!?])(?:\s|$)")
_NEW_HEADING = re.compile(r"^## What[’']s New in ")
_COMING_HEADING = re.compile(r"^## What[’']s Coming\s*$")
# Any heading that names the sections, at any level, so a stale variant elsewhere is refused rather
# than left beside the new block.
_COMING_ANY = re.compile(r"^#{1,6} What[’']s Coming\b", re.IGNORECASE)
_NEW_ANY = re.compile(r"^#{1,6} What[’']s New\b", re.IGNORECASE)
_ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
_ANY_TOP_HEADING = re.compile(r"^#{1,2} ")
_FENCE = re.compile(r"^\s*(```|~~~)")


def _changelog_entries(text: str) -> list[tuple[str, str]]:
    """(version, body) per `## [version]` heading, newest first, without [Unreleased]."""
    parts = re.split(r"^## \[([^\]]+)\][^\n]*\n", text, flags=re.MULTILINE)
    entries = [(parts[i], parts[i + 1]) for i in range(1, len(parts) - 1, 2)]
    return [(v, b) for v, b in entries if v.strip().lower() != "unreleased"]


def _entry_sections(body: str) -> tuple[str, dict[str, str]]:
    """(intro paragraph, {section name: text}) for one CHANGELOG entry."""
    parts = re.split(r"^### (.+)$", body, flags=re.MULTILINE)
    return parts[0].strip(), {parts[i].strip(): parts[i + 1] for i in range(1, len(parts) - 1, 2)}


def _top_level_bullets(block: str) -> list[str]:
    """Each `- ` bullet with its indented continuation lines joined into one line."""
    items: list[str] = []
    current: str | None = None
    for raw in block.split("\n"):
        if raw.startswith("- "):
            if current is not None:
                items.append(current)
            current = raw[2:].strip()
        elif current is not None and raw.startswith("  ") and raw.strip():
            current += " " + raw.strip()
        elif current is not None:
            items.append(current)
            current = None
    if current is not None:
        items.append(current)
    return items


def _bold_title(bullet: str) -> str | None:
    m = re.match(r"\*\*(.+?)\*\*", bullet)
    return m.group(1).strip().rstrip(".") if m else None


def _first_sentence(text: str, cap: int = _SUMMARY_MAX) -> str:
    flat = " ".join(text.split())
    m = _SENTENCE.match(flat)
    out = m.group(1) if m else flat
    return out if len(out) <= cap else out[: cap - 1].rstrip() + "…"


def _highlights(sections: dict[str, str], version: str, strict: bool) -> list[str]:
    """The `### Highlights` bullets of one entry. A block that exists but yields no bullet is an
    error for the release being rendered: falling back quietly would publish the detail the author
    meant to replace and say nothing."""
    name = next((n for n in sections if n.lower().startswith("highlights")), None)
    if name is None:
        return []
    bullets = _top_level_bullets(sections[name])
    if not bullets and strict:
        raise ValueError(
            f"CHANGELOG {version}: `### {name}` has no `- ` bullets, so README's What's New would "
            f"silently fall back to titles"
        )
    return bullets


def newest_release(changelog_text: str) -> str:
    entries = _changelog_entries(changelog_text)
    if not entries:
        raise ValueError("CHANGELOG has no released entry")
    return entries[0][0]


def render_whats_new(changelog_text: str, version: str, earlier: int = _WHATS_NEW_EARLIER) -> str:
    """README's `## What's New in <version>` block: the highlights, a counted link to the rest, and
    the releases before this one."""
    entries = _changelog_entries(changelog_text)
    if not entries or entries[0][0] != version:
        top = entries[0][0] if entries else "none"
        raise ValueError(
            f"CHANGELOG's newest released entry is {top}, not {version}; README's What's New would "
            f"describe the wrong release"
        )
    _, sections = _entry_sections(entries[0][1])
    highlights = _highlights(sections, version, strict=True)
    detail = [
        b
        for name, block in sections.items()
        if not name.lower().startswith("highlights")
        for b in _top_level_bullets(block)
    ]
    if highlights:
        shown = highlights
    else:
        shown = [_bold_title(b) or _first_sentence(b) for b in detail][:_WHATS_NEW_FALLBACK_MAX]
    if not shown:
        raise ValueError(
            f"No bullet items found in the CHANGELOG entry for {version}; README's What's New "
            f"would be synced empty"
        )

    lines = [f"## What's New in {version}", "", *[f"- {item}" for item in shown], ""]
    if highlights:
        lines.append(
            f"All {len(detail)} entries for this release are in the [CHANGELOG](CHANGELOG.md)."
        )
    elif len(detail) > len(shown):
        lines.append(
            f"Plus {len(detail) - len(shown)} more entries in the [CHANGELOG](CHANGELOG.md)."
        )
    else:
        lines.append("The detail is in the [CHANGELOG](CHANGELOG.md).")
    previous = []
    for ver, body in entries[1 : 1 + earlier]:
        intro, secs = _entry_sections(body)
        first_highlight = next(iter(_highlights(secs, ver, strict=False)), "")
        first_title = next(
            (
                t
                for t in (
                    _bold_title(b) for s, blk in secs.items() for b in _top_level_bullets(blk)
                )
                if t
            ),
            "",
        )
        # The line is already bold at the version; a highlight that opens with its own bold title
        # would nest one in the other.
        summary = re.sub(
            r"^\*\*(.+?)\*\*",
            r"\1",
            _first_sentence(first_highlight or intro or first_title),
            count=1,
        )
        if summary:
            previous.append(f"- **{ver}** — {summary}")
    if previous:
        lines += ["", "**Earlier releases**", "", *previous]
    return "\n".join(lines)


def _strings_in(value: object):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for v in value.values():
            yield from _strings_in(v)
    elif isinstance(value, (list, tuple)):
        for v in value:
            yield from _strings_in(v)


def validate_roadmap(data: object) -> dict:
    """The forecast behind What's Coming, or ValueError naming the first thing wrong with it."""
    if not isinstance(data, dict) or not isinstance(data.get("items"), list):
        raise ValueError("roadmap needs an `items` list")
    forecast = data.get("forecast")
    if not isinstance(forecast, dict) or not forecast.get("made_in"):
        raise ValueError("roadmap needs `forecast.made_in`, the release the forecast was made in")
    if not isinstance(forecast["made_in"], str):
        raise ValueError(
            f"forecast.made_in must be quoted text, not {forecast['made_in']!r} (an unquoted 1.10 "
            f"loads as the number 1.1)"
        )
    seen: set[str] = set()
    for n, item in enumerate(data["items"], 1):
        if not isinstance(item, dict):
            raise ValueError(f"roadmap item {n} is not a mapping")
        for field in _ROADMAP_FIELDS:
            if item.get(field) in (None, ""):
                raise ValueError(f"roadmap item {n}: missing {field}")
        for field in _ROADMAP_TEXT_FIELDS:
            if not isinstance(item[field], str):
                raise ValueError(
                    f"roadmap item {n}: {field} must be text, not {type(item[field]).__name__}"
                )
        conf = item["confidence"]
        if isinstance(conf, bool) or not isinstance(conf, (int, float)) or not 0 <= conf <= 1:
            raise ValueError(
                f"roadmap item {n}: confidence must be a number from 0 to 1, not {conf!r}"
            )
        ceiling = _ROADMAP_EVIDENCE_CEILING.get(item["evidence"])
        if ceiling is None:
            raise ValueError(
                f"roadmap item {n}: evidence must be one of "
                f"{', '.join(_ROADMAP_EVIDENCE_CEILING)}, not {item['evidence']!r}"
            )
        if conf > ceiling:
            raise ValueError(
                f"roadmap item {n}: evidence {item['evidence']!r} cannot carry confidence above "
                f"{ceiling}, not {conf}"
            )
        if item["status"] not in _ROADMAP_STATUSES:
            raise ValueError(
                f"roadmap item {n}: status must be one of {', '.join(_ROADMAP_STATUSES)}, not "
                f"{item['status']!r}"
            )
        if item["status"] != "predicted" and not item.get("resolved_in"):
            raise ValueError(
                f"roadmap item {n}: a {item['status']} item needs resolved_in, the release that "
                f"settled it"
            )
        if not isinstance(item.get("steered"), bool):
            raise ValueError(
                f"roadmap item {n}: steered must be true or false (was a goal for it already open "
                f"when it was predicted?)"
            )
        horizon = item.get("horizon")
        if horizon is None and item["status"] == "predicted":
            raise ValueError(
                f"roadmap item {n}: a predicted item needs a horizon, the release by which it will "
                f"be settled"
            )
        if horizon is not None and not (
            isinstance(horizon, str) and _RELEASE_NUMBER.match(horizon)
        ):
            raise ValueError(
                f'roadmap item {n}: horizon must be a quoted release number like "0.165.0", not '
                f"{horizon!r}"
            )
        if item["status"] in ("retracted", "superseded") and not (
            isinstance(item.get("reason"), str) and item["reason"].strip()
        ):
            raise ValueError(
                f"roadmap item {n}: a {item['status']} item needs a reason (why the prediction was "
                f"wrong or replaced)"
            )
        revisions = item.get("revisions")
        if revisions is not None:
            complete = (
                isinstance(revisions, list)
                and revisions
                and all(
                    isinstance(r, dict) and all(r.get(k) not in (None, "") for k in _REVISION_KEYS)
                    for r in revisions
                )
            )
            if not complete:
                hint = (
                    " (an unquoted `on:` key is the boolean true in YAML 1.1: quote the `on` key)"
                    if any(isinstance(r, dict) and True in r for r in revisions)
                    else ""
                )
                raise ValueError(
                    f"roadmap item {n}: revisions need on, from, to and reason in every entry{hint}"
                )
            if not all(_ISO_DATE.match(str(r["on"])) for r in revisions):
                raise ValueError(
                    f"roadmap item {n}: a revision's `on` is the date it was made, as YYYY-MM-DD"
                )
            if revisions[-1]["to"] != conf:
                raise ValueError(
                    f"roadmap item {n}: the last of revisions ends at {revisions[-1]['to']!r} but "
                    f"confidence is {conf!r}"
                )
        if item["id"] in seen:
            raise ValueError(f"roadmap item {n}: duplicate id {item['id']!r}")
        seen.add(item["id"])
    for n, item in enumerate(data["items"], 1):
        if item["status"] == "superseded" and item.get("superseded_by") not in seen:
            raise ValueError(
                f"roadmap item {n}: superseded_by must name another item in the roadmap, not "
                f"{item.get('superseded_by')!r}"
            )
    for text in (*_strings_in(forecast), *_strings_in(data["items"])):
        if leak := _INTERNAL_ID.search(text):
            raise ValueError(
                f"roadmap carries an internal id ({leak.group(0)!r}); the roadmap is public text"
            )
    return data


def parse_roadmap_text(text: str) -> dict:
    """The roadmap as YAML, unvalidated: how a RELEASED copy is read, since a tag cannot be fixed
    when a rule is tightened later."""
    import yaml

    data = yaml.safe_load(text)
    return data if isinstance(data, dict) else {}


def load_roadmap_text(text: str) -> dict:
    return validate_roadmap(parse_roadmap_text(text))


def _cell(text: object) -> str:
    return " ".join(str(text).split()).replace("|", "\\|")


def render_whats_coming(roadmap: dict) -> str:
    """README's `## What's Coming` block, or "" when the forecast has nothing open and nothing
    graded."""
    validate_roadmap(roadmap)
    items = roadmap["items"]
    open_items = [i for i in items if i["status"] == "predicted"]
    count = {s: sum(1 for i in items if i["status"] == s) for s in _ROADMAP_STATUSES}
    unplanned = sum(1 for i in items if i["status"] == "shipped" and not i["steered"])
    if not open_items and not any(v for s, v in count.items() if s != "predicted"):
        return ""
    lines = ["## What's Coming", ""]
    if open_items:
        lines += [
            f"{PROJECT}'s own forecast, made in {roadmap['forecast']['made_in']} from its open "
            f"work and its known weak spots. Each "
            "prediction carries a confidence, the evidence behind it and the observation that "
            "would prove it wrong, and is graded when "
            "it ships or is dropped. The word beside the confidence says how the basis is known: "
            "**measured** by a command, **reviewed** "
            "in the code, **reported** by someone else, or **inferred**; weaker evidence cannot "
            "carry a higher confidence. **planned** "
            "means a goal for it already existed when we predicted it: those are scored "
            "separately, because delivering planned work is "
            "not foresight.",
            "",
            "| Direction | Confidence | Why we expect it | What would change our mind |",
            "|---|---|---|---|",
        ]
        for i in sorted(open_items, key=lambda it: -it["confidence"]):
            lines.append(
                f"| **{_cell(i['title'])}**: {_cell(i['prediction'])} | "
                f"{round(i['confidence'] * 100)}% "
                f"({i['evidence']}{', planned' if i['steered'] else ''}) | {_cell(i['basis'])} | "
                f"{_cell(i['falsifier'])} |"
            )
        lines.append("")
    if any(v for s, v in count.items() if s != "predicted"):
        lines.append(
            f"Forecast record: {count['shipped']} shipped ({unplanned} of them not on the plan "
            f"when predicted), {count['dropped']} "
            f"dropped, {count['retracted']} retracted, {count['superseded']} superseded so far."
        )
    while lines and lines[-1] == "":
        lines.pop()
    return "\n".join(lines)


def _release_tuple(version: str) -> tuple[int, ...]:
    return tuple(int(part) for part in version.split("."))


def items_past_horizon(roadmap: dict, version: str) -> list[dict]:
    """Predicted items whose horizon release has already been cut.

    Nothing stays open indefinitely."""
    return [
        i
        for i in roadmap["items"]
        if i["status"] == "predicted" and _release_tuple(i["horizon"]) < _release_tuple(version)
    ]


def roadmap_silent_edits(released: dict, current: dict) -> list[str]:
    """What changed against the last RELEASED roadmap without leaving a trace, one message per
    problem.

    A forecast that can be adjusted after the fact is not a forecast. A changed confidence,
    prediction, falsifier, horizon or evidence on a predicted item needs a new dated revision saying
    why; a resolved item cannot be rewritten; and an item cannot be deleted, only retracted, dropped
    or superseded, so the record keeps its misses. Drafts before the first release are free (no
    baseline).
    """
    problems = []
    now = {i["id"]: i for i in current.get("items", [])}
    for old in released.get("items") or []:
        if not isinstance(old, dict) or "id" not in old:
            continue
        item = now.get(old["id"])
        if item is None:
            problems.append(
                f"{old['id']}: removed from the roadmap; retract, drop or supersede it instead of "
                f"deleting it"
            )
            continue
        old_revisions, revisions = old.get("revisions") or [], item.get("revisions") or []
        if revisions[: len(old_revisions)] != old_revisions:
            problems.append(
                f"{old['id']}: revisions released earlier were edited; they are the record, append "
                f"a new one instead"
            )
            continue
        if old.get("status", "predicted") != "predicted":
            rewritten = sorted(k for k in {*old, *item} if old.get(k) != item.get(k))
            if rewritten:
                problems.append(
                    f"{old['id']}: {', '.join(rewritten)} changed on an item that was already "
                    f"{old['status']}"
                )
            continue
        # Only fields the released copy has are compared: one a later rule introduced was never
        # released, so adding it is no edit.
        if "steered" in old and old.get("steered") != item.get("steered"):
            problems.append(
                f"{old['id']}: steered changed from {old.get('steered')!r} to "
                f"{item.get('steered')!r}; it records the moment of prediction"
            )
        changed = [f for f in _ROADMAP_GUARDED if f in old and old.get(f) != item.get(f)]
        if not changed:
            continue
        if item["status"] == "predicted":
            if len(revisions) <= len(old_revisions):
                problems.append(
                    f"{old['id']}: {', '.join(changed)} changed with no new revision; append one "
                    f"under `revisions` saying why"
                )
            elif revisions[len(old_revisions)].get("from") != old.get("confidence"):
                problems.append(
                    f"{old['id']}: the new revision starts from "
                    f"{revisions[len(old_revisions)].get('from')!r}, "
                    f"but the released confidence was {old.get('confidence')!r}"
                )
        else:
            problems.append(
                f"{old['id']}: {', '.join(changed)} rewritten after the item was resolved or while "
                f"resolving it"
            )
    return problems


def replace_whats_new_block(readme: str, block: str) -> str:
    """README with its What's New section (and the What's Coming that follows it) replaced by
    `block`.

    The span starts at the `## What's New in` heading and ends at the next level-1 or level-2
    heading that is not What's Coming, or at a `---` divider that does not lead into What's Coming,
    or at the end of the file. Headings inside a code fence do not count. Everything outside the
    span is returned byte for byte. A What's Coming anywhere else is refused, because replacing
    around it would leave two forecasts.
    """
    lines = readme.split("\n")
    in_fence = False
    start = end = None
    coming_elsewhere: list[int] = []
    new_elsewhere: list[int] = []
    for i, line in enumerate(lines):
        if _FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if start is None:
            if _NEW_HEADING.match(line):
                start = i
            elif _COMING_ANY.match(line):
                coming_elsewhere.append(i)
            continue
        if end is not None:
            if _COMING_ANY.match(line):
                coming_elsewhere.append(i)
            elif _NEW_ANY.match(line):
                new_elsewhere.append(i)
            continue
        if line.strip() == "---":
            following = next((ln for ln in lines[i + 1 :] if ln.strip()), "")
            if not _COMING_HEADING.match(following):
                end = i
        elif _ANY_TOP_HEADING.match(line) and not _COMING_HEADING.match(line):
            end = i
    if start is None:
        raise ValueError("no `## What's New in …` heading found")
    if new_elsewhere:
        raise ValueError(
            f"README has a second What's New heading (line {new_elsewhere[0] + 1}) after the one "
            f"being replaced; remove the stale copy"
        )
    if coming_elsewhere:
        raise ValueError(
            f"README has a What's Coming heading outside the What's New block (line "
            f"{coming_elsewhere[0] + 1}); "
            "move it directly under What's New so the sync can replace both"
        )
    after = lines[end:] if end is not None else []
    return "\n".join([*lines[:start], *block.split("\n"), "", *after])


def _released_roadmap_text(root: Path, ref: str | None) -> str | None:
    """docs/ROADMAP.yaml at the last release tag (or `ref`), or None when there is no tag or the
    file did not exist there."""
    git = {"cwd": root, "capture_output": True, "text": True}
    if ref is None:
        tag = subprocess.run(
            ["git", "describe", "--tags", "--abbrev=0", "--match", "v[0-9]*"], **git
        )
        if tag.returncode != 0:
            return None
        ref = tag.stdout.strip()
    elif (
        subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}"], **git
        ).returncode
        != 0
    ):
        # A named baseline that is not there must not read as "no released roadmap": that would turn
        # the check off.
        raise ValueError(f"--baseline-ref {ref!r} does not resolve to a commit")
    shown = subprocess.run(["git", "show", f"{ref}:docs/ROADMAP.yaml"], **git)
    return shown.stdout if shown.returncode == 0 else None


def roll_unreleased(changelog_text: str, version: str, day: str) -> str:
    """The CHANGELOG as release.sh's roll will leave it: the [Unreleased] body under `## [version] -
    day`. For a preview only."""
    match = re.search(
        r"^## \[Unreleased\][^\n]*\n(?P<body>.*?)(?=^## \[)",
        changelog_text,
        flags=re.DOTALL | re.MULTILINE,
    )
    if match is None:
        raise ValueError("CHANGELOG has no `## [Unreleased]` section followed by a release")
    body = match.group("body").strip("\n")
    if not body.strip():
        raise ValueError("CHANGELOG's [Unreleased] block is empty: nothing to release")
    head, rest = changelog_text[: match.start()], changelog_text[match.end() :]
    return f"{head}## [Unreleased]\n\n## [{version}] - {day}\n\n{body}\n\n{rest}"


def build_readme(
    root: Path, version: str | None, baseline_ref: str | None, changelog: str | None = None
) -> tuple[str, str, list[str]]:
    """(current README, rendered README, warnings). Raises ValueError for anything that must stop
    the sync."""
    readme_path, changelog_path, roadmap_path = (
        root / "README.md",
        root / "CHANGELOG.md",
        root / "docs" / "ROADMAP.yaml",
    )
    if changelog is None:
        changelog = changelog_path.read_text(encoding="utf-8")
    version = version or newest_release(changelog)
    block = render_whats_new(changelog, version)
    warnings: list[str] = []
    if roadmap_path.exists():
        roadmap = load_roadmap_text(roadmap_path.read_text(encoding="utf-8"))
        released = _released_roadmap_text(root, baseline_ref)
        if released is not None:
            problems = roadmap_silent_edits(parse_roadmap_text(released), roadmap)
            if problems:
                raise ValueError(
                    "docs/ROADMAP.yaml has edits that left no trace:\n  " + "\n  ".join(problems)
                )
        for item in items_past_horizon(roadmap, version):
            warnings.append(
                f"roadmap item {item['id']!r} is past its horizon {item['horizon']}: adjudicate it "
                f"or move the horizon with a revision"
            )
        coming = render_whats_coming(roadmap)
        if coming:
            block = f"{block}\n\n{coming}"
    current = readme_path.read_text(encoding="utf-8")
    return current, replace_whats_new_block(current, block), warnings


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument(
        "--check", action="store_true", help="exit 1 if README.md is out of date; change nothing"
    )
    parser.add_argument(
        "--version",
        help="release to render What's New for (default: newest released CHANGELOG entry)",
    )
    parser.add_argument(
        "--baseline-ref",
        help="git ref holding the last released roadmap (default: the latest v* tag)",
    )
    parser.add_argument(
        "--preview",
        metavar="VERSION",
        help="render README as releasing [Unreleased] as VERSION would leave it, and change "
        "nothing (release.sh runs this before it bumps)",
    )
    args = parser.parse_args(argv)
    try:
        if args.preview:
            changelog = roll_unreleased(
                (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8"),
                args.preview,
                date.today().isoformat(),
            )
            current, rendered, warnings = build_readme(
                REPO_ROOT, args.preview, args.baseline_ref, changelog
            )
        else:
            current, rendered, warnings = build_readme(REPO_ROOT, args.version, args.baseline_ref)
    except ValueError as error:
        print(f"readme_sections: {error}", file=sys.stderr)
        return 1
    for warning in warnings:
        print(f"readme_sections: warning: {warning}", file=sys.stderr)
    if args.preview:
        print(f"readme_sections: releasing [Unreleased] as {args.preview} renders cleanly")
        return 0
    if args.check:
        if current != rendered:
            print(
                "readme_sections: README.md's What's New / What's Coming are out of date; run "
                "scripts/readme_sections.py",
                file=sys.stderr,
            )
            return 1
        print("readme_sections: README.md is current")
        return 0
    if current != rendered:
        (REPO_ROOT / "README.md").write_text(rendered, encoding="utf-8")
        print("readme_sections: README.md updated")
    else:
        print("readme_sections: README.md already current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
