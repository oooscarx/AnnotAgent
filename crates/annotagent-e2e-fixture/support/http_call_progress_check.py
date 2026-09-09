#!/usr/bin/env python3
"""Check UIAPI-008 fields in a fresh real HTTP fixture's recorded responses."""
import argparse
import json
from pathlib import Path


def receipts(value):
    if isinstance(value, dict):
        if {"id", "task_id", "request_hash", "status", "evidence"} <= value.keys():
            yield value
        for child in value.values():
            yield from receipts(child)
    elif isinstance(value, list):
        for child in value:
            yield from receipts(child)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    path = args.manifest.resolve()
    assert (path.parent / "FIXTURE_ONLY").read_text() == "AnnotAgent HTTP integration fixture\n"
    manifest = json.loads(path.read_text())
    trace = json.loads((path.parent / "HTTP_TRACE.json").read_text())
    found = []
    for row in trace["requests"]:
        route = row["path"].split("?", 1)[0]
        response = row.get("response")
        if route.endswith("/workspace") and isinstance(response, dict):
            found.extend(receipts(response.get("calls", [])))
        elif route.endswith(("/calls", "/schema-proposals")):
            found.extend(receipts(response))
    assert found, "No actual HTTP call receipts"
    for call in found:
        assert call["started_at"]
        if call["status"] == "reserved":
            assert call["stage"] in ["reserved", "provider_request", "response_received"]
            assert call["completed_at"] is None and call["duration_ms"] is None
        else:
            assert call["stage"] == "settled"
            assert call["completed_at"] and call["duration_ms"] >= 0
    unknown = [c for c in found if c["status"] == "in_doubt"]
    assert unknown, "Fixture must exercise actual unknown call settlement"
    assert all(c["failure"] and c["failure"]["category"] in ["cancelled", "interrupted"] for c in unknown)
    result = {"base_url": manifest["base_url"], "receipt_observations": len(found), "call_ids": sorted({c["id"] for c in found}), "stages_observed": sorted({c["stage"] for c in found}), "unknown_example": unknown[0]}
    output = path.parent / "UIAPI-008_HTTP.json"
    output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    print(output)


if __name__ == "__main__":
    main()
