#!/usr/bin/env python3
"""Generate a carapace YAML spec for the gt CLI from gt-cli-docs.json.

Usage:
    python3 scripts/gen-carapace-spec.py [--input path/to/gt-cli-docs.json] [--output path/to/gt.yaml]

Defaults:
    --input  crates/gt-tui/gt-cli-docs.json
    --output ~/.config/carapace/specs/gt.yaml

The generated spec:
- Covers all 363+ gt/bd commands from the Cobra-generated docs
- Uses $() macros for dynamic completions (beads, rigs, agents, mail, etc.)
- Parses both <required> and [optional] positional args (excluding [flags])
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path


# ---------------------------------------------------------------------------
# Arg name → carapace completion macro mapping
# ---------------------------------------------------------------------------

# Shell snippets that produce tab-separated "value\tdescription" lines
BEAD_COMPLETIONS = (
    '$(bd ready --json 2>/dev/null | python3 -c '
    '"import json,sys;[print(f\\"{i[\'id\']}\\\\t{i[\'title\'][:60]}\\")'
    ' for i in json.load(sys.stdin)[:30]]" 2>/dev/null)'
)
IN_PROGRESS_COMPLETIONS = (
    '$(bd list --status=in_progress --json 2>/dev/null | python3 -c '
    '"import json,sys;[print(f\\"{i[\'id\']}\\\\t{i[\'title\'][:60]}\\")'
    ' for i in json.load(sys.stdin)[:30]]" 2>/dev/null)'
)
RIG_COMPLETIONS = "$(gt rig list 2>/dev/null | awk '/^  [a-z]/{print $1}')"
AGENT_COMPLETIONS = (
    "$(gt status --json 2>/dev/null | python3 -c \""
    "import json,sys;"
    "d=json.load(sys.stdin);"
    "aa=d.get('agents',[]);"
    "[aa.extend(r.get('agents',[])) for r in d.get('rigs',[])];"
    "[print(f\\\"{a.get('address',a.get('name',''))}\\\\t{a.get('role','')}\\\") for a in aa]"
    "\" 2>/dev/null)"
)
MAIL_COMPLETIONS = (
    '$(gt mail inbox --json 2>/dev/null | python3 -c '
    '"import json,sys;[print(f\\"{m[\'id\']}\\\\t{m[\'subject\'][:60]}\\")'
    ' for m in json.load(sys.stdin)[:20]]" 2>/dev/null)'
)
CONVOY_COMPLETIONS = (
    '$(gt convoy list --json 2>/dev/null | python3 -c '
    '"import json,sys;[print(f\\"{c[\'id\']}\\\\t{c[\'title\'][:60]}\\")'
    ' for c in json.load(sys.stdin)[:20]]" 2>/dev/null)'
)
FORMULA_COMPLETIONS = (
    "$(gt formula list 2>/dev/null | grep -v '^$' | tail -n +2 | awk '{print $1}')"
)
POLECAT_COMPLETIONS = (
    "$(gt polecat list --all 2>/dev/null | grep -v '^$' | tail -n +2 | awk '{print $1}')"
)


def completion_for_arg(arg_name: str, cmd_path: str) -> str | None:
    """Map an arg name + command context to a carapace completion macro.

    Returns None for free-text args (no dynamic completions available).
    """
    # Direct name matches
    if arg_name in ("rig", "target-prefix"):
        return RIG_COMPLETIONS
    if arg_name in ("polecat", "rig/polecat"):
        return POLECAT_COMPLETIONS
    if arg_name in ("agent", "agent-bead", "member", "role"):
        return AGENT_COMPLETIONS
    if arg_name == "convoy-id":
        return CONVOY_COMPLETIONS
    if arg_name == "bead-or-formula":
        return BEAD_COMPLETIONS

    # Bead-like IDs
    if any(k in arg_name for k in ("bead", "issue", "epic", "mr-id")) or arg_name == "id":
        if "close" in cmd_path or "ack" in cmd_path:
            return IN_PROGRESS_COMPLETIONS
        return BEAD_COMPLETIONS

    # Context-dependent "name"
    if arg_name in ("name", "name..."):
        if "formula" in cmd_path:
            return FORMULA_COMPLETIONS
        if any(k in cmd_path for k in ("crew", "dog", "agent")):
            return AGENT_COMPLETIONS
        return None

    # Context-dependent "target"
    if arg_name == "target":
        if "polecat" in cmd_path:
            return POLECAT_COMPLETIONS
        if any(k in cmd_path for k in ("nudge", "crew")):
            return AGENT_COMPLETIONS
        if "sling" in cmd_path:
            return RIG_COMPLETIONS
        return None

    # Message-related
    if arg_name in ("message-id", "mail-id") or "message-id" in arg_name:
        return MAIL_COMPLETIONS
    if arg_name in ("thread-id",):
        return MAIL_COMPLETIONS

    return None  # FreeText — $files fallback


def parse_positional_args(usage: str) -> list[str]:
    """Extract positional arg names from a usage string.

    Matches both <required> and [optional] args, excluding [flags].
    """
    args = []
    # Required: <name>
    for m in re.finditer(r"<([^>]+)>", usage):
        name = m.group(1).strip()
        if name:
            args.append(name)
    # Optional: [name] but not [flags]
    for m in re.finditer(r"\[([^\]]+)\]", usage):
        name = m.group(1).strip()
        if name and name != "flags" and not name.startswith("-"):
            args.append(name)
    return args


# ---------------------------------------------------------------------------
# YAML generation (manual — avoids PyYAML dependency)
# ---------------------------------------------------------------------------

def yaml_str(s: str) -> str:
    """Escape a string for YAML output."""
    if not s:
        return '""'
    # If it contains characters that need quoting, use double quotes
    if any(c in s for c in (':', '#', '{', '}', '[', ']', ',', '&', '*', '?', '|',
                             '-', '<', '>', '=', '!', '%', '@', '`', '"', "'", '\n')):
        escaped = s.replace("\\", "\\\\").replace('"', '\\"')
        return f'"{escaped}"'
    return s


def build_command_tree(commands: list[dict]) -> dict:
    """Build a nested tree of commands from the flat docs list."""
    root = {"children": {}, "data": None}

    for cmd in commands:
        path = cmd["cmd"].split()
        if not path or path[0] != "gt":
            continue
        parts = path[1:]  # skip "gt" root
        node = root
        for part in parts:
            if part not in node["children"]:
                node["children"][part] = {"children": {}, "data": None}
            node = node["children"][part]
        node["data"] = cmd

    return root


def emit_command(name: str, node: dict, indent: int) -> list[str]:
    """Emit YAML lines for a command node and its children."""
    lines = []
    prefix = "  " * indent
    lines.append(f"{prefix}- name: {yaml_str(name)}")

    data = node.get("data")
    if data:
        desc = data.get("short", "")
        if desc:
            lines.append(f"{prefix}  description: {yaml_str(desc)}")

        # Positional args → completion section
        args = parse_positional_args(data.get("usage", ""))
        if args:
            completions = []
            for arg_name in args:
                macro = completion_for_arg(arg_name, data["cmd"])
                completions.append(macro if macro else "$files")

            lines.append(f"{prefix}  completion:")
            lines.append(f"{prefix}    positional:")
            for comp in completions:
                lines.append(f"{prefix}    - - {comp}")
    else:
        # Group node with no direct data
        pass

    # Recurse into children
    children = node.get("children", {})
    if children:
        lines.append(f"{prefix}  commands:")
        for child_name in sorted(children.keys()):
            lines.extend(emit_command(child_name, children[child_name], indent + 1))

    return lines


def generate_spec(docs: dict) -> str:
    """Generate the full carapace YAML spec."""
    commands = docs.get("commands", [])
    tree = build_command_tree(commands)

    lines = [
        "name: gt",
        "description: Gas Town - Multi-agent workspace manager",
    ]

    children = tree.get("children", {})
    if children:
        lines.append("commands:")
        for name in sorted(children.keys()):
            lines.extend(emit_command(name, children[name], 0))

    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Generate carapace spec for gt CLI")
    parser.add_argument(
        "--input", "-i",
        default="crates/gt-tui/gt-cli-docs.json",
        help="Path to gt-cli-docs.json",
    )
    parser.add_argument(
        "--output", "-o",
        default=os.path.expanduser("~/.config/carapace/specs/gt.yaml"),
        help="Output path for carapace YAML spec",
    )
    args = parser.parse_args()

    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: {input_path} not found", file=sys.stderr)
        sys.exit(1)

    with open(input_path) as f:
        docs = json.load(f)

    spec = generate_spec(docs)

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w") as f:
        f.write(spec)

    total = docs.get("total_commands", "?")
    with_args = sum(
        1 for c in docs.get("commands", [])
        if parse_positional_args(c.get("usage", ""))
    )
    with_dynamic = sum(
        1 for c in docs.get("commands", [])
        if any(
            completion_for_arg(a, c["cmd"]) is not None
            for a in parse_positional_args(c.get("usage", ""))
        )
    )
    print(f"Generated {output_path}")
    print(f"  {total} total commands")
    print(f"  {with_args} with positional args")
    print(f"  {with_dynamic} with dynamic completions")


if __name__ == "__main__":
    main()
