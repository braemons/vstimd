"""Standalone compatibility checker: wonderlamp_client.visual vs psychopy.visual.

Usage
-----
    # Print human-readable report
    python compat/check_compat.py

    # Generate pytest fixture file
    python compat/check_compat.py --output-pytest-fixtures tests/_compat_fixtures.py

Exit code 0 = no missing items; 1 = at least one missing item found.
"""

from __future__ import annotations

import argparse
import importlib
import inspect
import sys
from dataclasses import dataclass, field
from typing import Any


CHECKED_CLASSES = ["Window", "Circle", "Rect", "Polygon", "Line", "ShapeStim"]

# Parameters that are wonderlamp_client extensions (not expected in psychopy)
_WONDERLAMP_EXTENSIONS = {"address", "deferred", "inline_limit_kb"}


# ---------------------------------------------------------------------------
# Data structures
# ---------------------------------------------------------------------------

@dataclass
class ClassReport:
    name: str
    missing_params: list[str] = field(default_factory=list)
    missing_methods: list[str] = field(default_factory=list)
    extensions: list[str] = field(default_factory=list)  # present in wonderlamp, absent in psychopy

    @property
    def ok(self) -> bool:
        return not self.missing_params and not self.missing_methods


# ---------------------------------------------------------------------------
# Introspection helpers
# ---------------------------------------------------------------------------

def _public_methods(cls: type) -> set[str]:
    return {
        name for name, obj in inspect.getmembers(cls, predicate=inspect.isfunction)
        if not name.startswith("_")
    }


def _init_params(cls: type) -> dict[str, inspect.Parameter]:
    sig = inspect.signature(cls.__init__)
    params = dict(sig.parameters)
    params.pop("self", None)
    return params


def _compare(
    class_name: str,
    psychopy_cls: type | None,
    wonderlamp_cls: type | None,
) -> ClassReport:
    report = ClassReport(name=class_name)

    if wonderlamp_cls is None:
        report.missing_params.append("<class missing entirely>")
        return report

    if psychopy_cls is None:
        # psychopy not installed — can't compare, skip silently
        return report

    # --- Parameters ---
    psychopy_params = set(_init_params(psychopy_cls).keys())
    wonderlamp_params = set(_init_params(wonderlamp_cls).keys())

    for p in sorted(psychopy_params - wonderlamp_params):
        report.missing_params.append(p)

    for p in sorted(wonderlamp_params - psychopy_params - _WONDERLAMP_EXTENSIONS):
        report.extensions.append(p)

    for p in sorted(wonderlamp_params & _WONDERLAMP_EXTENSIONS):
        report.extensions.append(p)

    # --- Methods ---
    psychopy_methods = _public_methods(psychopy_cls)
    wonderlamp_methods = _public_methods(wonderlamp_cls)
    for m in sorted(psychopy_methods - wonderlamp_methods):
        report.missing_methods.append(m)

    return report


# ---------------------------------------------------------------------------
# Main logic
# ---------------------------------------------------------------------------

def run_check() -> tuple[list[ClassReport], bool]:
    """Return (reports, any_missing)."""
    try:
        import psychopy.visual as pv  # type: ignore[import]
        psychopy_visual: Any = pv
    except ImportError:
        psychopy_visual = None
        print("WARNING: psychopy not installed — cannot compare signatures.\n")

    import wonderlamp_client.visual as vv

    reports: list[ClassReport] = []
    any_missing = False

    for class_name in CHECKED_CLASSES:
        psychopy_cls = getattr(psychopy_visual, class_name, None) if psychopy_visual else None
        wonderlamp_cls = getattr(vv, class_name, None)
        report = _compare(class_name, psychopy_cls, wonderlamp_cls)
        reports.append(report)
        if not report.ok:
            any_missing = True

    return reports, any_missing


def print_report(reports: list[ClassReport]) -> None:
    col = 18
    for r in reports:
        ext_str = f"  ({len(r.extensions)} extensions: {', '.join(r.extensions)})" if r.extensions else ""
        if r.ok:
            print(f"  {r.name:<{col}} OK{ext_str}")
        else:
            print(f"  {r.name:<{col}} WARN:")
            if r.missing_params:
                print(f"    missing params:   {', '.join(r.missing_params)}")
            if r.missing_methods:
                print(f"    missing methods:  {', '.join(r.missing_methods)}")
            if r.extensions:
                print(f"    extensions:       {', '.join(r.extensions)}")


def write_pytest_fixtures(reports: list[ClassReport], output_path: str) -> None:
    """Write a Python module with CHECKED_CLASSES, REQUIRED_PARAMS, ALL_METHODS."""
    import wonderlamp_client.visual as vv

    # Collect params that exist in wonderlamp (used for parametrized param existence tests)
    required_params: list[tuple[str, str]] = []
    all_methods: list[tuple[str, str]] = []

    for r in reports:
        wonderlamp_cls = getattr(vv, r.name, None)
        if wonderlamp_cls is None:
            continue
        for param in _init_params(wonderlamp_cls):
            required_params.append((r.name, param))
        for method in _public_methods(wonderlamp_cls):
            all_methods.append((r.name, method))

    lines = [
        "# Auto-generated by compat/check_compat.py — do not edit manually.",
        "# Regenerate: python compat/check_compat.py --output-pytest-fixtures tests/_compat_fixtures.py",
        "",
        f"CHECKED_CLASSES = {CHECKED_CLASSES!r}",
        "",
        "REQUIRED_PARAMS = [",
    ]
    for item in required_params:
        lines.append(f"    {item!r},")
    lines += [
        "]",
        "",
        "ALL_METHODS = [",
    ]
    for item in all_methods:
        lines.append(f"    {item!r},")
    lines += ["]", ""]

    with open(output_path, "w") as f:
        f.write("\n".join(lines))
    print(f"Wrote pytest fixtures to: {output_path}")


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Check wonderlamp_client.visual API compatibility with psychopy.visual"
    )
    parser.add_argument(
        "--output-pytest-fixtures",
        metavar="PATH",
        help="Write parametrized pytest fixture data to PATH",
    )
    args = parser.parse_args()

    print("wonderlamp_client.visual  ←→  psychopy.visual  compatibility check")
    print("=" * 62)

    reports, any_missing = run_check()
    print_report(reports)
    print()

    if args.output_pytest_fixtures:
        write_pytest_fixtures(reports, args.output_pytest_fixtures)

    sys.exit(1 if any_missing else 0)


if __name__ == "__main__":
    main()
