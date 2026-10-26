"""Tests for scripts/readme_sections.py (README What's New / What's Coming)."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

import pytest

_SPEC = importlib.util.spec_from_file_location(
    "readme_sections", Path(__file__).with_name("readme_sections.py")
)
assert _SPEC is not None and _SPEC.loader is not None
rs = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(rs)

CHANGELOG = """# Changelog

## [Unreleased]

### Added
- **Something not released yet.**

## [0.2.0] - 2026-10-25

### Highlights
- **Feature A ships.** It does a thing.
- **Feature B ships.** It does another thing.

### Added
- **Feature A.** Detail.

### Fixed
- **Bug C.** Detail.
  continued on a second line.

## [0.1.1] - 2026-10-24

### Fixed
- **An older fix.** It was fixed.

## [0.1.0] - 2026-10-23

### Highlights
- **First release.** Hello.
"""


def _item(**overrides):
    item = {
        "id": "a-direction",
        "title": "A direction",
        "prediction": "Something happens by the next release.",
        "basis": "Read in the code.",
        "falsifier": "The next release ships without it.",
        "confidence": 0.7,
        "evidence": "reviewed",
        "horizon": "0.3.0",
        "steered": True,
        "status": "predicted",
    }
    item.update(overrides)
    return item


def _roadmap(*items):
    return {
        "forecast": {"made_in": "0.2.0", "made_on": "2026-10-26"},
        "items": list(items) or [_item()],
    }


def test_whats_new_uses_highlights_and_skips_unreleased():
    block = rs.render_whats_new(CHANGELOG, "0.2.0")
    assert block.startswith("## What's New in 0.2.0")
    assert "- **Feature A ships.** It does a thing." in block
    assert "Something not released yet" not in block
    assert "All 2 entries for this release are in the [CHANGELOG](CHANGELOG.md)." in block
    # Earlier releases: a highlight's first sentence, else the first bullet's bold title without its
    # full stop (core's behaviour).
    assert "- **0.1.1** — An older fix\n" in block
    assert block.endswith("- **0.1.0** — First release. Hello.")


def test_whats_new_without_highlights_counts_what_it_left_out():
    one = CHANGELOG.replace(
        "### Highlights\n- **Feature A ships.** It does a thing.\n- **Feature B ships.** It does "
        "another thing.\n\n",
        "",
    )
    block = rs.render_whats_new(one, "0.2.0")
    assert "- Feature A\n" in block and "- Bug C\n" in block
    assert "Plus 0 more" not in block
    assert "The detail is in the [CHANGELOG](CHANGELOG.md)." in block
    many = one.replace(
        "### Fixed\n", "### Fixed\n" + "".join(f"- **Fix {n}.** Detail.\n" for n in range(6))
    )
    assert "Plus 2 more entries in the [CHANGELOG](CHANGELOG.md)." in rs.render_whats_new(
        many, "0.2.0"
    )


def test_newest_release_skips_unreleased():
    assert rs.newest_release(CHANGELOG) == "0.2.0"


def test_whats_new_refuses_a_release_that_is_not_the_newest():
    with pytest.raises(ValueError, match="newest released entry is 0.2.0"):
        rs.render_whats_new(CHANGELOG, "0.1.1")


def test_empty_highlights_block_is_an_error():
    text = CHANGELOG.replace(
        "- **Feature A ships.** It does a thing.\n- **Feature B ships.** It does another thing.\n",
        "\n",
    )
    with pytest.raises(ValueError, match="has no `- ` bullets"):
        rs.render_whats_new(text, "0.2.0")


def test_roadmap_evidence_caps_confidence():
    with pytest.raises(ValueError, match="cannot carry confidence above 0.7"):
        rs.validate_roadmap(_roadmap(_item(evidence="reported", confidence=0.8)))


def test_roadmap_refuses_internal_ids():
    with pytest.raises(ValueError, match="internal id"):
        rs.validate_roadmap(_roadmap(_item(basis="Tracked in goal adc33d26.")))
    with pytest.raises(ValueError, match="internal id"):
        rs.validate_roadmap(_roadmap(_item(basis="Fixed in 5f7f3e3bde.")))


def test_roadmap_allows_dates_and_counts():
    rs.validate_roadmap(
        _roadmap(_item(basis="Measured 2026-10-25: 400 of 5373 tests, 1234567 bytes."))
    )


def test_roadmap_requires_quoted_made_in_and_a_horizon():
    with pytest.raises(ValueError, match="quoted text"):
        rs.validate_roadmap({"forecast": {"made_in": 0.2}, "items": [_item()]})
    with pytest.raises(ValueError, match="needs a horizon"):
        rs.validate_roadmap(_roadmap(_item(horizon=None)))


def test_whats_coming_orders_by_confidence_and_marks_planned():
    block = rs.render_whats_coming(
        _roadmap(
            _item(id="low", title="Low", confidence=0.5),
            _item(id="high", title="High", confidence=0.8, steered=False),
        )
    )
    assert block.index("**High**") < block.index("**Low**")
    assert "80% (reviewed) |" in block
    assert "50% (reviewed, planned) |" in block
    assert block.startswith("## What's Coming")
    assert "ecodex's own forecast, made in 0.2.0" in block


def test_silent_edit_needs_a_revision():
    released = _roadmap()
    current = copy.deepcopy(released)
    current["items"][0]["confidence"] = 0.6
    assert any("no new revision" in p for p in rs.roadmap_silent_edits(released, current))
    current["items"][0]["revisions"] = [
        {"on": "2026-10-27", "from": 0.7, "to": 0.6, "reason": "new evidence"}
    ]
    assert rs.roadmap_silent_edits(released, current) == []


def test_deleting_a_released_item_is_refused():
    assert any(
        "removed from the roadmap" in p for p in rs.roadmap_silent_edits(_roadmap(), {"items": []})
    )


README = """# Title

Intro.

## What's New in 0.1.0

- old

## What's Coming

old table

## Install

```
## What's New in fake
```

Body.
"""


def test_replace_block_keeps_everything_outside_the_span():
    out = rs.replace_whats_new_block(README, "## What's New in 0.2.0\n\n- new")
    assert out.startswith("# Title\n\nIntro.\n\n## What's New in 0.2.0\n\n- new\n\n## Install\n")
    assert "old table" not in out
    assert "```\n## What's New in fake\n```\n\nBody.\n" in out


def test_replace_block_refuses_a_stray_whats_coming():
    stray = README.replace("## Install", "## Install\n\n## What's Coming\n\nsecond")
    with pytest.raises(ValueError, match="outside the What's New block"):
        rs.replace_whats_new_block(stray, "## What's New in 0.2.0")


def test_baseline_roadmap_is_parsed_not_validated_by_todays_rules():
    # A rule tightened after a release must not make the released copy (a tag, which cannot be
    # fixed) fail every later check.
    fields = {
        k: v for k, v in _item().items() if k != "steered"
    }  # as if `steered` became required after the release
    old = (
        "items:\n  - "
        + "\n    ".join(
            f"{k}: {v!r}" if isinstance(v, str) else f"{k}: {v}" for k, v in fields.items()
        )
        + "\n"
    )
    with pytest.raises(ValueError):
        rs.load_roadmap_text(old)
    released = rs.parse_roadmap_text(old)
    assert rs.roadmap_silent_edits(released, _roadmap()) == []


def test_released_revisions_cannot_be_edited_in_place():
    first = {"on": "2026-10-27", "from": 0.7, "to": 0.6, "reason": "new evidence"}
    released = _roadmap(_item(confidence=0.6, revisions=[first]))
    current = copy.deepcopy(released)
    current["items"][0]["revisions"][0]["reason"] = "rewritten later"
    current["items"][0]["confidence"] = 0.5
    current["items"][0]["revisions"].append(
        {"on": "2026-10-28", "from": 0.6, "to": 0.5, "reason": "more"}
    )
    assert any("revisions released" in p for p in rs.roadmap_silent_edits(released, current))


def test_a_new_revision_starts_from_the_released_confidence():
    released = _roadmap()
    current = copy.deepcopy(released)
    current["items"][0]["confidence"] = 0.6
    current["items"][0]["revisions"] = [{"on": "2026-10-27", "from": 0.9, "to": 0.6, "reason": "x"}]
    assert any("starts from 0.9" in p for p in rs.roadmap_silent_edits(released, current))


def test_revision_dates_must_be_dates():
    with pytest.raises(ValueError, match="YYYY-MM-DD"):
        rs.validate_roadmap(
            _roadmap(
                _item(
                    confidence=0.6,
                    revisions=[{"on": "last week", "from": 0.7, "to": 0.6, "reason": "x"}],
                )
            )
        )


def test_an_explicit_baseline_ref_that_does_not_resolve_is_an_error():
    with pytest.raises(ValueError, match="does not resolve"):
        rs._released_roadmap_text(rs.REPO_ROOT, "refs/tags/no-such-release-ref")


def test_roll_unreleased_previews_the_next_release():
    rolled = rs.roll_unreleased(CHANGELOG, "0.3.0", "2026-10-27")
    assert rs.newest_release(rolled) == "0.3.0"
    assert "- Something not released yet\n" in rs.render_whats_new(rolled, "0.3.0")
    with pytest.raises(ValueError, match="nothing to release"):
        rs.roll_unreleased(
            CHANGELOG.replace("### Added\n- **Something not released yet.**\n", ""),
            "0.3.0",
            "2026-10-27",
        )
    empty_highlights = CHANGELOG.replace(
        "## [Unreleased]\n\n", "## [Unreleased]\n\n### Highlights\n\n", 1
    )
    with pytest.raises(ValueError, match="has no `- ` bullets"):
        rs.render_whats_new(rs.roll_unreleased(empty_highlights, "0.3.0", "2026-10-27"), "0.3.0")


def test_replace_block_refuses_a_second_whats_new_or_a_coming_variant():
    second = README.replace("## Install", "## Install\n\n## What's New in 0.0.9\n\n- stale")
    with pytest.raises(ValueError, match="second What's New"):
        rs.replace_whats_new_block(second, "## What's New in 0.2.0")
    variant = README.replace("## Install", "## Install\n\n## What's Coming next\n\nstale forecast")
    with pytest.raises(ValueError, match="outside the What's New block"):
        rs.replace_whats_new_block(variant, "## What's New in 0.2.0")


def test_a_stale_copy_right_after_the_block_is_refused():
    stale_new = README.replace("## Install", "## What's New in 0.0.9\n\n- stale\n\n## Install")
    with pytest.raises(ValueError, match="second What's New"):
        rs.replace_whats_new_block(stale_new, "## What's New in 0.2.0")
    stale_coming = README.replace("## Install", "## What's Coming next\n\nstale\n\n## Install")
    with pytest.raises(ValueError, match="outside the What's New block"):
        rs.replace_whats_new_block(stale_coming, "## What's New in 0.2.0")


def test_an_ordinary_heading_mentioning_whats_new_is_left_alone():
    other = README.replace(
        "## Install", "## Install\n\n### What's new compared with upstream codex\n\nprose"
    )
    out = rs.replace_whats_new_block(other, "## What's New in 0.2.0")
    assert "### What's new compared with upstream codex\n\nprose" in out


def test_a_resolved_item_compares_only_the_fields_it_was_released_with():
    old = {k: v for k, v in _item(status="shipped", resolved_in="0.2.0").items() if k != "steered"}
    released = {"items": [old]}
    assert (
        rs.roadmap_silent_edits(released, _roadmap(_item(status="shipped", resolved_in="0.2.0")))
        == []
    )


def test_new_revisions_chain_from_the_released_confidence_even_without_a_field_change():
    released = _roadmap(_item(confidence=0.6))
    current = copy.deepcopy(released)
    current["items"][0]["revisions"] = [{"on": "2026-10-27", "from": 0.9, "to": 0.6, "reason": "x"}]
    assert any("starts from 0.9" in p for p in rs.roadmap_silent_edits(released, current))
    chained = copy.deepcopy(released)
    chained["items"][0]["confidence"] = 0.5
    chained["items"][0]["revisions"] = [
        {"on": "2026-10-27", "from": 0.6, "to": 0.55, "reason": "x"},
        {"on": "2026-10-28", "from": 0.4, "to": 0.5, "reason": "y"},
    ]
    assert any("starts from 0.4" in p for p in rs.roadmap_silent_edits(released, chained))


def test_quoting_a_released_revision_date_is_not_an_edit():
    import datetime

    rev = {"on": datetime.date(2026, 10, 27), "from": 0.7, "to": 0.6, "reason": "x"}
    released = _roadmap(_item(confidence=0.6, revisions=[rev]))
    current = copy.deepcopy(released)
    current["items"][0]["revisions"][0]["on"] = "2026-10-27"
    assert rs.roadmap_silent_edits(released, current) == []


@pytest.mark.parametrize(
    "stale",
    [
        "## What's coming next\n\nstale",  # case variant
        "# What's Coming\n\nstale",  # level-1 variant
    ],
)
@pytest.mark.parametrize("where", ["before", "after_block", "later"])
def test_a_second_forecast_is_refused_wherever_it_sits(stale, where):
    if where == "before":
        text = README.replace("## What's New in 0.1.0", f"{stale}\n\n## What's New in 0.1.0")
    elif where == "after_block":
        text = README.replace("## Install", f"{stale}\n\n## Install")
    else:
        text = README.replace("Body.", f"Body.\n\n{stale}")
    with pytest.raises(ValueError, match="outside the What's New block"):
        rs.replace_whats_new_block(text, "## What's New in 0.2.0")


@pytest.mark.parametrize("where", ["before", "after_block", "later"])
def test_a_second_whats_new_is_refused_wherever_it_sits(where):
    stale = "# What's new in 0.0.9\n\n- stale"
    if where == "before":
        text = README.replace("## What's New in 0.1.0", f"{stale}\n\n## What's New in 0.1.0")
    elif where == "after_block":
        text = README.replace("## Install", f"{stale}\n\n## Install")
    else:
        text = README.replace("Body.", f"Body.\n\n{stale}")
    with pytest.raises(ValueError, match="second What's New"):
        rs.replace_whats_new_block(text, "## What's New in 0.2.0")


@pytest.mark.parametrize("status", ["predicted", "shipped"])
def test_quoting_a_released_date_is_not_an_edit_in_any_status(status):
    import datetime

    rev = {"on": datetime.date(2026, 10, 27), "from": 0.7, "to": 0.6, "reason": "x"}
    extra = {"resolved_in": "0.2.0"} if status != "predicted" else {}
    released = _roadmap(_item(confidence=0.6, status=status, revisions=[rev], **extra))
    current = copy.deepcopy(released)
    current["items"][0]["revisions"][0]["on"] = "2026-10-27"
    assert rs.roadmap_silent_edits(released, current) == []
