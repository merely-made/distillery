# Copyright 2026 Mark Alan Boykin
# SPDX-License-Identifier: MPL-2.0
"""Write separate public workshop inputs and scoring keys for the P3 trial."""

import argparse
import json
from pathlib import Path

# Each case supplies its question, passages, and rubric. A precedence edge
# matters only when both passages appear. Tied passages may appear either way.
BRIEFS = {
    "closure": (
        "Which observations establish that the footbridge closed on Monday? Cite the direct observation before independent corroboration.",
        [
            ("notice", "Maintenance calendar", "A footbridge inspection was planned for the next month. The calendar gives no closure date."),
            ("bus", "Route log", "On Monday the 09:10 bus took the signed detour because the footbridge was closed."),
            ("gate", "Keeper's log", "On Monday at 08:45 I locked both footbridge gates and posted the closure signs."),
        ],
        ["gate", "bus"], [("gate", "bus")],
    ),
    "archive": (
        "Which checks did the archivist actually complete? Include every completed check. Their order is immaterial.",
        [
            ("labels", "Intake sheet", "I checked each box label against the signed intake list."),
            ("seals", "Condition sheet", "I checked the wax seals and recorded their condition in the ledger."),
            ("count", "Inventory sheet", "I counted the manuscripts and verified the count against the accession record."),
        ],
        ["labels", "seals", "count"], [],
    ),
    "repair": (
        "Which actions returned power to the lantern? Give the repair actions in the order they occurred.",
        [
            ("closure", "Evening notice", "The board approved a new closing time for the lantern museum."),
            ("restore", "Electrician's log", "At 15:00 I restored power after the cable passed inspection."),
            ("cable", "Work order", "At 14:00 I replaced the broken cable after the circuit had been isolated."),
            ("isolate", "Shift log", "At 13:00 I isolated the lantern circuit before the cable repair."),
        ],
        ["isolate", "cable", "restore"], [("isolate", "cable"), ("cable", "restore"), ("isolate", "restore")],
    ),
    "bank": (
        "Which passages document erosion at the river bank? Include each independent observation. Either order is acceptable.",
        [
            ("stakes", "Survey notes", "At the river bend the bank edge had retreated 18 centimetres from our fixed stakes."),
            ("branch", "Branch report", "The bank closed its riverside branch after loan demand fell."),
            ("roots", "Site photograph", "Exposed willow roots hang above a fresh void where the river cut into the bank."),
            ("rates", "Rate table", "The credit union lowered the rate on its bank loans."),
        ],
        ["stakes", "roots"], [],
    ),
    "recall": (
        "Which accounts bear on whether the spare key was returned? Include evidence on both sides, with the signed receipt first.",
        [
            ("tag", "Key catalogue", "The spare key carries a round brass tag."),
            ("denial", "Evening statement", "The duty librarian says she did not find the spare key in the returns box that evening."),
            ("receipt", "Dated receipt", "The clerk and borrower both signed the receipt recording return of the spare key at noon."),
        ],
        ["receipt", "denial"], [("receipt", "denial")],
    ),
    "water": (
        "Which records establish how the water sample moved from collection through analysis? Give the custody sequence.",
        [
            ("draw", "Field log", "At sunrise the collector drew sample W7 into a sterile bottle and sealed it."),
            ("handoff", "Courier slip", "At noon the courier took sealed sample W7 from the collector to the laboratory."),
            ("assay", "Laboratory log", "At dusk the analyst opened sample W7 at the laboratory and measured its pH."),
        ],
        ["draw", "handoff", "assay"], [("draw", "handoff"), ("handoff", "assay"), ("draw", "assay")],
    ),
}


def materialize():
    cases, keys = [], {}
    for brief, (ask, passages, required, precedence) in BRIEFS.items():
        sources = [row[0] for row in passages]
        assert len(set(sources)) == len(sources) and 2 <= len(sources) <= 4
        assert set(required) <= set(sources) and len(required) >= 2
        assert all(a in required and b in required and a != b for a, b in precedence)
        # Counterbalance the ends, including where the strongest evidence appears.
        for label, rows in (("a", passages), ("b", list(reversed(passages)))):
            name = f"{brief}-{label}"
            cases.append({"id": name, "ask": ask, "passages": [
                {"id": key, "title": title, "text": body} for key, title, body in rows
            ]})
            keys[name] = {"brief": brief, "required": required,
                          "irrelevant": sorted(set(sources) - set(required)),
                          "precedence": precedence}
    return {"cases": cases}, keys


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for name, data in zip(("corpus", "rubric"), materialize()):
        (args.output / f"{name}.json").write_text(
            json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
        )
