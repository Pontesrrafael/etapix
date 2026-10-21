"""Prints resource, task and assignment cost fields of MSPDI files."""
import glob
import os
import re
import sys


def field(block, tag):
    m = re.search(rf"<{tag}>([^<]*)", block)
    return m.group(1) if m else ""


for path in sys.argv[1:] or sorted(glob.glob("tests/golden/b17/*.xml")):
    s = open(path, encoding="utf-8").read()
    print("#", os.path.basename(path))
    for r in re.findall(r"<Resource>.*?</Resource>", s, re.S):
        if field(r, "UID") != "0":
            print("  R", field(r, "Name"), "type", field(r, "Type"), "rate", field(r, "StandardRate"),
                  "ot", field(r, "OvertimeRate"), "use", field(r, "CostPerUse"), "cost", field(r, "Cost"))
    for t in re.findall(r"<Task>.*?</Task>", s, re.S):
        print("  T", field(t, "Name"), "cost", field(t, "Cost"), "fixed", field(t, "FixedCost"),
              "accr", field(t, "FixedCostAccrual"), "work", field(t, "Work"))
    for a in re.findall(r"<Assignment>.*?</Assignment>", s, re.S):
        if field(a, "ResourceUID") != "-65535":
            print("  A task", field(a, "TaskUID"), "res", field(a, "ResourceUID"), "units", field(a, "Units"),
                  "work", field(a, "Work"), "ot", field(a, "OvertimeWork"), "cost", field(a, "Cost"),
                  "otcost", field(a, "OvertimeCost"))
