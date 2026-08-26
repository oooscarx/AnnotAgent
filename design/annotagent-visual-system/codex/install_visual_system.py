#!/usr/bin/env python3
"""Install the AnnotAgent visual-system package into an existing repository.

This script deliberately does not edit application source, AGENTS.md, Git config, or secrets.
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path


def copy_tree(src: Path, dst: Path, force: bool) -> None:
    if dst.exists():
        if not force:
            raise FileExistsError(f"destination already exists: {dst} (use --force to replace it)")
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", required=True, help="Path to the AnnotAgent repository root")
    parser.add_argument("--force", action="store_true", help="Replace an existing installed visual-system directory/skill")
    args = parser.parse_args()

    package_root = Path(__file__).resolve().parents[1]
    repo = Path(args.repo).expanduser().resolve()
    if not repo.is_dir():
        print(f"error: repository directory not found: {repo}", file=sys.stderr)
        return 2

    git_marker = repo / ".git"
    cargo_marker = repo / "Cargo.toml"
    if not git_marker.exists() and not cargo_marker.exists():
        print("warning: target does not look like a Git or Cargo repository", file=sys.stderr)

    design_target = repo / "design" / "annotagent-visual-system"
    skill_source = package_root / "codex" / "skill" / "annotagent-visual-system"
    skill_target = repo / ".agents" / "skills" / "annotagent-visual-system"

    try:
        copy_tree(package_root, design_target, args.force)
        skill_target.parent.mkdir(parents=True, exist_ok=True)
        copy_tree(skill_source, skill_target, args.force)
    except FileExistsError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 3

    print(f"installed visual system: {design_target}")
    print(f"installed Codex skill:   {skill_target}")
    print("next:")
    print(f"  cd {repo}")
    print("  start a new Codex session")
    print("  invoke: $annotagent-visual-system")
    print("  ask it to read design/annotagent-visual-system/codex/CODEX-PROMPT.md and execute the task")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
