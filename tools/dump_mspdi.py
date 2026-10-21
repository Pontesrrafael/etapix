"""Prints tasks and assignments of MSPDI files compactly: `python -I dump_mspdi.py <files...>`."""
import re
import sys
import xml.etree.ElementTree as ET

NS = {"p": "http://schemas.microsoft.com/project"}
TYPES = {"0": "FU", "1": "FD", "2": "FW"}


def t(el, tag):
    v = el.find(f"p:{tag}", NS)
    return v.text if v is not None else ""


def hours(iso):
    m = re.fullmatch(r"-?PT(\d+)H(\d+)M(\d+)S", iso or "")
    return f"{int(m[1]) + int(m[2]) / 60:g}h" if m else iso


def short(dt):
    return dt[5:16].replace("T", " ") if dt else "-"


for path in sys.argv[1:]:
    root = ET.parse(path).getroot()
    print(f"# {path.replace(chr(92), '/').split('/')[-1]}")
    res = {t(r, "UID"): t(r, "Name") for r in root.findall("p:Resources/p:Resource", NS)}
    names = {}
    for k in root.findall("p:Tasks/p:Task", NS):
        if t(k, "OutlineLevel") == "0":
            continue
        names[t(k, "UID")] = t(k, "Name")
        print(f"  T {'  ' * (int(t(k, 'OutlineLevel') or 1) - 1)}{t(k, 'Name'):<16} "
              f"{TYPES.get(t(k, 'Type'), t(k, 'Type'))} ED={t(k, 'EffortDriven')} sum={t(k, 'Summary')} "
              f"dur={hours(t(k, 'Duration'))} work={hours(t(k, 'Work'))} "
              f"{short(t(k, 'Start'))}..{short(t(k, 'Finish'))} lvl={int(t(k, 'LevelingDelay') or 0) / 14400:g}ed")
        print(f"      ES={short(t(k, 'EarlyStart'))} EF={short(t(k, 'EarlyFinish'))} "
              f"LS={short(t(k, 'LateStart'))} LF={short(t(k, 'LateFinish'))} "
              f"TS={int(t(k, 'TotalSlack') or 0) / 4800:g}d FS={int(t(k, 'FreeSlack') or 0) / 4800:g}d "
              f"crit={t(k, 'Critical')} constr={t(k, 'ConstraintType')}@{short(t(k, 'ConstraintDate'))} "
              f"deadline={short(t(k, 'Deadline'))} preds={[t(l, 'PredecessorUID') for l in k.findall('p:PredecessorLink', NS)]} "
              f"uid={t(k, 'UID')}")
    for a in root.findall("p:Assignments/p:Assignment", NS):
        if t(a, "ResourceUID") == "-65535":
            continue
        print(f"    A {names.get(t(a, 'TaskUID'), '?'):<14} {res.get(t(a, 'ResourceUID'), '?'):<4} "
              f"units={t(a, 'Units')} work={hours(t(a, 'Work'))} {short(t(a, 'Start'))}..{short(t(a, 'Finish'))} "
              f"lvl={int(t(a, 'LevelingDelay') or 0) / 14400:g}ed")
