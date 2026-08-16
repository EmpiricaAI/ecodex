"""Guard ecodex's ``empirica`` subprocess argv against the real CLI parser.

The wrapper surface is source-discovered rather than mirrored here: Python
hook argv lists are extracted with ``ast`` and Rust ``Command::new`` chains are
extracted from source. Every discovered command path and option is then checked
against ``create_argument_parser()`` from the installed Empirica package.

This catches a renamed or removed command/flag before a broad best-effort hook
handler turns argparse rejection into a silent capability loss. The test skips
when Empirica core is unavailable, matching the other real-schema/parser guards
in this suite.
"""

import argparse
import ast
import re
from dataclasses import dataclass
from pathlib import Path

import pytest

PLUGIN_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = PLUGIN_ROOT.parents[1]
HOOKS = PLUGIN_ROOT / "assets" / "hooks_scripts" / "hooks"
RUST_ROOT = REPO_ROOT / "codex-rs"

_RUST_COMMAND = re.compile(
    r"(?:tokio::process::)?Command::new\(\"empirica\"\)(?P<chain>.*?);",
    re.DOTALL,
)
_RUST_STRING = re.compile(r'"((?:\\.|[^"\\])*)"')


@dataclass
class Invocation:
    source: Path
    line: int
    tokens: list[str]

    def label(self) -> str:
        return f"{self.source.relative_to(REPO_ROOT)}:{self.line}"


def _string_literals(node: ast.AST) -> list[str]:
    if not isinstance(node, (ast.List, ast.Tuple)):
        return []
    return [
        element.value
        for element in node.elts
        if isinstance(element, ast.Constant) and isinstance(element.value, str)
    ]


def _scope_of(node: ast.AST, parents: dict[ast.AST, ast.AST]) -> ast.AST:
    current = node
    while current in parents:
        current = parents[current]
        if isinstance(current, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Module)):
            return current
    raise AssertionError(f"AST node at line {getattr(node, 'lineno', '?')} has no scope")


def _assigned_name(node: ast.Assign | ast.AnnAssign) -> str | None:
    targets = node.targets if isinstance(node, ast.Assign) else [node.target]
    if len(targets) == 1 and isinstance(targets[0], ast.Name):
        return targets[0].id
    return None


def _python_invocations(path: Path) -> list[Invocation]:
    tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    parents = {child: parent for parent in ast.walk(tree) for child in ast.iter_child_nodes(parent)}
    invocations: list[Invocation] = []
    assignments: dict[tuple[ast.AST, str], list[Invocation]] = {}

    for node in ast.walk(tree):
        if not isinstance(node, (ast.List, ast.Tuple)):
            continue
        tokens = _string_literals(node)
        if not tokens or tokens[0] != "empirica":
            continue
        invocation = Invocation(path, node.lineno, tokens)
        invocations.append(invocation)

        parent = parents.get(node)
        if isinstance(parent, (ast.Assign, ast.AnnAssign)) and parent.value is node:
            name = _assigned_name(parent)
            if name is not None:
                assignments.setdefault((_scope_of(parent, parents), name), []).append(invocation)

    # Capture conditionally appended flags such as
    # ``cmd += ["--project-id", project_id]`` without maintaining a map of
    # which hook commands happen to be assembled incrementally today.
    for node in ast.walk(tree):
        name: str | None = None
        fragment: ast.AST | None = None
        if isinstance(node, ast.AugAssign) and isinstance(node.target, ast.Name):
            name = node.target.id
            fragment = node.value
        elif (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and node.func.attr in {"append", "extend"}
            and node.args
        ):
            name = node.func.value.id
            fragment = node.args[0]
        if name is None or fragment is None:
            continue

        candidates = assignments.get((_scope_of(node, parents), name), [])
        prior = [candidate for candidate in candidates if candidate.line < node.lineno]
        if not prior:
            continue
        target = max(prior, key=lambda candidate: candidate.line)
        if isinstance(fragment, ast.Constant) and isinstance(fragment.value, str):
            target.tokens.append(fragment.value)
        else:
            target.tokens.extend(_string_literals(fragment))

    return invocations


def _rust_invocations(path: Path) -> list[Invocation]:
    source = path.read_text(encoding="utf-8")
    invocations: list[Invocation] = []
    for match in _RUST_COMMAND.finditer(source):
        tokens = ["empirica"]
        for literal in _RUST_STRING.findall(match.group("chain")):
            tokens.append(ast.literal_eval(f'"{literal}"'))
        invocations.append(Invocation(path, source.count("\n", 0, match.start()) + 1, tokens))
    return invocations


def _all_invocations() -> list[Invocation]:
    invocations = [
        invocation
        for path in sorted(HOOKS.glob("*.py"))
        for invocation in _python_invocations(path)
    ]
    invocations.extend(
        invocation
        for path in sorted(RUST_ROOT.rglob("*.rs"))
        for invocation in _rust_invocations(path)
    )
    return invocations


def _subcommands(parser: argparse.ArgumentParser) -> dict[str, argparse.ArgumentParser]:
    actions = [
        action
        for action in parser._actions
        if isinstance(action, argparse._SubParsersAction)
    ]
    assert len(actions) <= 1, f"unexpected multiple subparser actions in {parser.prog}"
    return actions[0].choices if actions else {}


def _resolve_command(
    root: argparse.ArgumentParser, invocation: Invocation
) -> tuple[argparse.ArgumentParser | None, list[str], str | None]:
    parser = root
    command_path: list[str] = []
    remaining = invocation.tokens[1:]
    while choices := _subcommands(parser):
        if not remaining:
            return None, command_path, "missing subcommand token"
        token = remaining.pop(0)
        if token not in choices:
            return None, command_path, token
        command_path.append(token)
        parser = choices[token]
    return parser, command_path, None


def _flags(tokens: list[str]) -> set[str]:
    return {
        token.split("=", 1)[0]
        for token in tokens
        if token.startswith("-") and token != "-"
    }


def test_every_empirica_subprocess_invocation_matches_real_cli():
    try:
        from empirica.cli.cli_core import create_argument_parser
    except (ImportError, ModuleNotFoundError):
        pytest.skip("empirica core unavailable — cannot introspect the real CLI parser")

    invocations = _all_invocations()
    assert invocations, "no ecodex empirica subprocess invocations discovered"
    assert any(invocation.source.suffix == ".rs" for invocation in invocations), (
        "no Rust empirica spawn discovered — the extractor or wrapper surface changed"
    )
    assert any(invocation.source.suffix == ".py" for invocation in invocations), (
        "no vendored-hook empirica spawn discovered — the extractor or wrapper surface changed"
    )

    root = create_argument_parser()
    failures: list[str] = []
    for invocation in invocations:
        parser, command_path, missing_command = _resolve_command(root, invocation)
        if parser is None:
            prefix = " ".join(command_path) or "<root>"
            failures.append(
                f"{invocation.label()}: command drift after `{prefix}`: "
                f"{missing_command!r} is not a real subcommand"
            )
            continue

        valid_flags = {
            option
            for action in parser._actions
            for option in action.option_strings
        }
        missing_flags = sorted(_flags(invocation.tokens) - valid_flags)
        if missing_flags:
            command = " ".join(command_path)
            failures.append(
                f"{invocation.label()}: `empirica {command}` references flags "
                f"the real parser does not accept: {missing_flags}"
            )

    assert not failures, (
        "ecodex's empirica subprocess wrapper surface drifted from the real CLI:\n  - "
        + "\n  - ".join(failures)
    )
