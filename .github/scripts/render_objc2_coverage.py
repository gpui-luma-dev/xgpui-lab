"""Validate coverage evidence references and render the reviewed migration matrix."""
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def render():
    data = json.loads((ROOT / "validation/objc2-coverage.json").read_text())
    for key, evidence in data["evidence"].items():
        path = ROOT / evidence["source"]
        if evidence["witness"] not in path.read_text():
            raise ValueError(f"Missing evidence witness {key}: {evidence['source']}")
    lines = ["# objc2 / Metal migration coverage", "", f"Assessed xgpui revision: `{data['assessed_xgpui_revision']}`.", "", data["platform"] + ".", "", "This is a reviewed coverage inventory, not a claim that every path is migrated or that every test ran on this machine. Regenerate with `just coverage-report`; execute native tests and the separate consumer audit to refresh evidence. A source-reference check proves presence, not execution.", "", "## Legend", ""]
    if data.get("assessment_scope"):
        lines[6:6] = [data["assessment_scope"], ""]
    for key, value in data["legend"].items():
        lines.append(f"- **{key}** — {value}.")
    lines += ["", "## Rendering paths and migration boundaries", "", "| Path | Migration boundary | Rendering oracle |", "|---|---|---|"]
    for row in data["paths"]:
        lines.append(f"| {row['path']} | {row['migration']} | {row['visual']} |")
    for section, columns in [("ownership", data["ownership_columns"]), ("reuse", data["reuse_columns"])]:
        lines += ["", "## " + section.title(), "", "| Path | " + " | ".join(c.replace("_", " ") for c in columns) + " |", "|---|" + "---|" * len(columns)]
        for row in data["paths"]:
            cells = []
            for column in columns:
                cell = row[section][column]
                if cell["status"] not in data["legend"]:
                    raise ValueError(f"Unknown status {cell}")
                for key in cell["evidence"]:
                    if key not in data["evidence"]:
                        raise ValueError(f"Unknown evidence {key}")
                cells.append(cell["status"] + (" (" + ", ".join(cell["evidence"]) + ")" if cell["evidence"] else ""))
            lines.append("| " + row["path"] + " | " + " | ".join(cells) + " |")
    lines += ["", "## Scope limits by path", ""]
    for row in data["paths"]:
        lines.append(f"- **{row['path']}:** {row['limits']}")
    lines += ["", "## Evidence catalog", "", "Paths are relative to this report. Test and log names describe the narrow assertion they support.", ""]
    for key, item in data["evidence"].items():
        source = "../" + item["source"]
        lines.append(f"- **{key}:** [{item['title']}]({source}); witness `{item['witness']}`; execution: `{item['run']}`.")
    lines += ["", "## Gates before claiming a complete transition", "", "| Gate | Current status | Required evidence |", "|---|---|---|"]
    for gate in data["release_gates"]:
        lines.append(f"| {gate['id']} | {gate['status']} | {gate['requirement']} |")
    lines += ["", "## Clean consumer evidence", "", "Run `just consumer-audit`. The audit uses a temporary project, fresh target directory and isolated Cargo configuration; it may reuse the registry download/source cache. All dependencies must have registry sources; no `[patch]`, path, or Git dependencies are allowed. The audit performs `cargo build`, saves the lockfile/metadata/logs, and records selected macOS paths to each legacy dependency.", "", "The verified published baseline is xgpui/platform 1.0.0 plus all six extensions 0.2.0. It does not include this checkout's newer migration/fix, gpui-luma 0.2.4, or the entire lab application. A registry build cannot validate unpublished source changes. Candidate-release consumer verification remains a separate gate.", "", "Observed published macOS dependencies: `metal 0.33.0`, `objc 0.2.7`, `block 0.1.6`, `cocoa 0.26.0`, `cocoa-foundation 0.2.1`, and `objc_exception 0.1.2`. Older Core Foundation/Graphics/Text wrappers also remain. These coexist with objc2; a successful build does not prove their removal. See `artifacts/objc2-coverage/consumer-final/report.json` for exact dependency chains and `build.stderr.log` for compiler warnings.", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    text = render()
    output = ROOT / "validation/objc2-coverage.md"
    if args.check:
        if output.read_text() != text:
            raise SystemExit("Coverage report is stale; run just coverage-report")
        revision = subprocess.check_output(["git", "-C", str(ROOT.parent / "xgpui"), "rev-parse", "HEAD"], text=True).strip()
        if revision != json.loads((ROOT / "validation/objc2-coverage.json").read_text())["assessed_xgpui_revision"]:
            raise SystemExit("xgpui revision changed; review the coverage assessment")
    else:
        output.write_text(text)
    print(output)
