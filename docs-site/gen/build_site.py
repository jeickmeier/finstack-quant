"""Run the local documentation publication gate, then build the static site."""

from __future__ import annotations

from datetime import UTC, datetime
import subprocess
import sys
import time

from common import BUILD, SITE, write_json


def main() -> int:
    """Execute lessons and labs before checking their publication evidence."""
    commands = [
        [sys.executable, str(SITE / "gen" / "build_references.py")],
        [sys.executable, str(SITE / "gen" / "run_lesson_snippets.py"), "--require-all"],
        [sys.executable, str(SITE / "gen" / "build_labs.py")],
        [sys.executable, str(SITE / "gen" / "verify_prerequisites.py")],
        [sys.executable, str(SITE / "gen" / "check_coverage.py")],
        [sys.executable, str(SITE / "gen" / "check_curriculum.py"), "--check-api", "--require-evidence"],
        ["npm", "run", "build"],
        [sys.executable, str(SITE / "gen" / "check_links.py")],
    ]
    report = {"started_utc": datetime.now(UTC).isoformat(), "status": "running", "stages": []}
    started = time.monotonic()
    write_json(BUILD / "release.json", report)
    for command in commands:
        stage_started = time.monotonic()
        completed = subprocess.run(command, cwd=SITE, check=False)
        report["stages"].append({
            "command": command[1:] if command[0] == sys.executable else command,
            "seconds": time.monotonic() - stage_started,
            "exit_code": completed.returncode,
        })
        write_json(BUILD / "release.json", report)
        if completed.returncode:
            report.update(
                status="failed", finished_utc=datetime.now(UTC).isoformat(), seconds=time.monotonic() - started
            )
            write_json(BUILD / "release.json", report)
            return completed.returncode
    report.update(status="passed", finished_utc=datetime.now(UTC).isoformat(), seconds=time.monotonic() - started)
    write_json(BUILD / "release.json", report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
