#!/usr/bin/env python3
"""Package a Claude skill into a .skill file."""

import json
import os
import shutil
import sys
import zipfile
from pathlib import Path


def package_skill(skill_dir: str, output_file: str = None) -> str:
    """
    Package a Claude skill directory into a .skill file (ZIP format).

    Args:
        skill_dir: Path to the skill directory containing SKILL.md
        output_file: Output file path (default: <skill-name>.skill)

    Returns:
        Path to the created .skill file
    """
    skill_path = Path(skill_dir).resolve()

    if not skill_path.is_dir():
        raise ValueError(f"Skill directory not found: {skill_dir}")

    skill_md = skill_path / "SKILL.md"
    if not skill_md.exists():
        raise ValueError(f"SKILL.md not found in {skill_dir}")

    # Extract skill name from frontmatter
    skill_name = skill_path.name
    with open(skill_md) as f:
        for line in f:
            if line.startswith("name:"):
                skill_name = line.split(":", 1)[1].strip()
                break

    if not output_file:
        output_file = f"{skill_name}.skill"

    output_path = Path(output_file).resolve()

    # Create ZIP file with the skill contents
    with zipfile.ZipFile(output_path, "w", zipfile.ZIP_DEFLATED) as zf:
        for file_path in skill_path.rglob("*"):
            if file_path.is_file():
                arcname = file_path.relative_to(skill_path)
                zf.write(file_path, arcname)

    print(f"✓ Packaged skill to: {output_path}")
    return str(output_path)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python package_skill.py <skill_dir> [output_file]")
        sys.exit(1)

    skill_dir = sys.argv[1]
    output_file = sys.argv[2] if len(sys.argv) > 2 else None

    try:
        result = package_skill(skill_dir, output_file)
        print(f"Skill packaged successfully: {result}")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)
