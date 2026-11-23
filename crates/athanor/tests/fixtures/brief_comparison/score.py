# Copyright 2026 Mark Alan Boykin
# SPDX-License-Identifier: MPL-2.0
"""Score confirmed public workshop proposals against the predeclared P3 rubric.

blind CORPUS OUT.json RECEIPT... writes a producer-blind review packet.
score CORPUS RUBRIC OUT.json RECEIPT... reports blind marks, availability,
and paired comparisons. Give each placement a distinctly named receipt file.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def load(path, inputs):
    events = [json.loads(line) for line in path.read_text(encoding="utf-8-sig").splitlines()]
    if not events or events[0].get("event") != "start":
        raise ValueError(f"{path}: missing start")
    if events[-1] != {"event": "complete", "cases": len(inputs)}:
        raise ValueError(f"{path}: comparison incomplete")
    rows = {}
    for e in events:
        if "event" in e:
            continue
        case = e["case"]
        expected = [p["id"] for p in inputs[case]["passages"]]
        if e["inputs"] != expected or case in rows:
            raise ValueError(f"{path}: duplicate case or input mismatch in {case}")
        if e["ready_for_review"] and not (e.get("selected") and e.get("brief")):
            raise ValueError(f"{path}: missing reviewable proposal in {case}")
        rows[case] = e
    if inputs.keys() != rows.keys():
        raise ValueError(f"{path}: case IDs differ from input")
    return rows


def masked(rows, inputs):
    packet, choices = {}, {}
    for name, data in rows.items():
        choices[name] = {}
        for case, row in data.items():
            if not row["ready_for_review"]:
                choices[name][case] = None
                continue
            ordered = row["selected"]
            sources = {p["id"]: p for p in inputs[case]["passages"]}
            if len(set(ordered)) != len(ordered) or not 2 <= len(ordered) <= len(sources):
                raise ValueError(f"bad selection for {case}")
            text = row["brief"]
            # The actual reviewable document must quote each selected passage in order.
            positions = [text.index("> " + sources[i]["text"] + "\n") for i in ordered]
            if positions != sorted(positions) or text.count("[Open retained source ") != len(ordered):
                raise ValueError(f"brief and citations disagree for {case}")
            proposal = {"case": case, "ask": inputs[case]["ask"], "selected": ordered, "brief": text}
            key = hashlib.sha256(json.dumps(proposal, sort_keys=True).encode()).hexdigest()[:16]
            packet[key] = proposal
            choices[name][case] = key
    return dict(sorted(packet.items())), choices


def marks(proposal, key):
    order = proposal["selected"]
    return {
        "omissions": [i for i in key["required"] if i not in order],
        "inclusions": [i for i in order if i in key["irrelevant"]],
        "inversions": [[a,b] for a,b in key["precedence"]
                       if a in order and b in order and order.index(a) > order.index(b)],
    }


def scores(packet, choices, keys):
    blind = {k: marks(p, keys[p["case"]]) for k,p in packet.items()}
    groups = {}
    for name, rows in choices.items():
        per_brief = {}
        counts = Counter()
        for case, tag in rows.items():
            group = per_brief.setdefault(keys[case]["brief"], {"reviews": 0, "errors": 0})
            if tag is None:
                counts["unavailable"] += 1
                continue
            group["reviews"] += 1
            counts["reviewable"] += 1
            for field, errors in blind[tag].items():
                counts[field] += len(errors)
                group["errors"] += len(errors)
            counts["fully_correct"] += not any(blind[tag].values())
        groups[name] = {"counts": dict(counts), "per_brief": per_brief}
    return {"by_receipt": groups, "blind_marks": blind, "receipt_key": choices}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for mode in ("blind", "score"):
        command = commands.add_parser(mode)
        command.add_argument("corpus", type=Path)
        if mode == "score":
            command.add_argument("rubric", type=Path)
        command.add_argument("output", type=Path)
        command.add_argument("receipts", nargs="+", type=Path)
    args = parser.parse_args()
    inputs = {c["id"]: c for c in read_json(args.corpus)["cases"]}
    rows = {p.stem: load(p, inputs) for p in args.receipts}
    if len(rows) != len(args.receipts):
        parser.error("duplicate receipt names")
    packet, choices = masked(rows, inputs)
    result = packet if args.command == "blind" else scores(packet, choices, read_json(args.rubric))
    with args.output.open("x", encoding="utf-8") as out:
        json.dump(result, out, indent=2)
        out.write("\n")
