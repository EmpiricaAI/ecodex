"""Tests for scripts/readme_sections.py (README What's New / What's Coming)."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

import pytest

_SPEC = importlib.util.spec_from_file_location("readme_sections", Path(__file__).with_name("readme_sections.py"))
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
    return {"forecast": {"made_in": "0.2.0", "made_on": "2026-10-26"}, "items": list(items) or [_item()]}


def test_whats_new_uses_highlights_and_skips_unreleased():
    block = rs.render_whats_new(CHANGELOG, "0.2.0")
    assert block.startswith("## What's New in 0.2.0")
    assert "- **Feature A ships.** It does a thing." in block
    assert "Something not released yet" not in block
    assert "All 2 entries for this release are in the [CHANGELOG](CHANGELOG.md)." in block
    # Earlier releases: a highlight's first sentence, else the first bullet's bold title without its full stop (core's behaviour).
    assert "- **0.1.1** — An older fix\n" in block
    assert block.endswith("- **0.1.0** — First release. Hello.")


def test_whats_new_without_highlights_counts_what_it_left_out():
    one = CHANGELOG.replace("### Highlights\n- **Feature A ships.** It does a thing.\n- **Feature B ships.** It does another thing.\n\n", "")
    block = rs.render_whats_new(one, "0.2.0")
    assert "- Feature A\n" in block and "- Bug C\n" in block
    assert "Plus 0 more" not in block
    assert "The detail is in the [CHANGELOG](CHANGELOG.md)." in block
    many = one.replace("### Fixed\n", "### Fixed\n" + "".join(f"- **Fix {n}.** Detail.\n" for n in range(6)))
    assert "Plus 2 more entries in the [CHANGELOG](CHANGELOG.md)." in rs.render_whats_new(many, "0.2.0")


def test_newest_release_skips_unreleased():
    assert rs.newest_release(CHANGELOG) == "0.2.0"


def test_whats_new_refuses_a_release_that_is_not_the_newest():
    with pytest.raises(ValueError, match="newest released entry is 0.2.0"):
        rs.render_whats_new(CHANGELOG, "0.1.1")


def test_empty_highlights_block_is_an_error():
    text = CHANGELOG.replace("- **Feature A ships.** It does a thing.\n- **Feature B ships.** It does another thing.\n", "\n")
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
    rs.validate_roadmap(_roadmap(_item(basis="Measured 2026-10-25: 400 of 5373 tests, 1234567 bytes.")))


def test_roadmap_requires_quoted_made_in_and_a_horizon():
    with pytest.raises(ValueError, match="quoted text"):
        rs.validate_roadmap({"forecast": {"made_in": 0.2}, "items": [_item()]})
    with pytest.raises(ValueError, match="needs a horizon"):
        rs.validate_roadmap(_roadmap(_item(horizon=None)))


def test_whats_coming_orders_by_confidence_and_marks_planned():
    block = rs.render_whats_coming(_roadmap(_item(id="low", title="Low", confidence=0.5), _item(id="high", title="High", confidence=0.8, steered=False)))
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
    current["items"][0]["revisions"] = [{"on": "2026-10-27", "from": 0.7, "to": 0.6, "reason": "new evidence"}]
    assert rs.roadmap_silent_edits(released, current) == []


def test_deleting_a_released_item_is_refused():
    assert any("removed from the roadmap" in p for p in rs.roadmap_silent_edits(_roadmap(), {"items": []}))


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
