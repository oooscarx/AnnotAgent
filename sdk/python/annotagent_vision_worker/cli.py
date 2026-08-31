"""CLI entry point for Worker scaffolding."""

from __future__ import annotations

import argparse

from .scaffold import PRESETS, scaffold_worker


def main() -> None:
    parser = argparse.ArgumentParser(prog="annotagent-vision-worker")
    subcommands = parser.add_subparsers(dest="command", required=True)
    scaffold = subcommands.add_parser("scaffold")
    scaffold.add_argument("--name", required=True)
    scaffold.add_argument("--capability")
    scaffold.add_argument("--preset", choices=sorted(PRESETS))
    scaffold.add_argument("--output", default="workers")
    arguments = parser.parse_args()
    if arguments.command == "scaffold":
        target = scaffold_worker(
            arguments.output,
            name=arguments.name,
            capability=arguments.capability,
            preset=arguments.preset,
        )
        print(target)


if __name__ == "__main__":
    main()
