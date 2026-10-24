import json
import glob

for path in sorted(glob.glob("docs/parity/*.json")):
    if "results.json" in path:
        continue
    with open(path, "r", encoding="utf-8") as f:
        data = json.load(f)
    for case in data.get("cases", []):
        status = case.get("status")
        if status in ["untested", "deferred", "mismatched"]:
            print(f"{case.get('id'):<45} | {status:<10} | {case.get('scenario')}")
