"""Compare MSPDI exports produced by run_scenarios.ps1 (before/after leveling)."""
import glob, os, re, sys
import xml.etree.ElementTree as ET

NS = {"p": "http://schemas.microsoft.com/project"}
TENTHS_PER_DAY = 4800  # slack/leveling delay are tenths of minutes; 480 min per working day


def txt(el, tag):
    v = el.find(f"p:{tag}", NS)
    return v.text if v is not None else None


def dur_days(iso):
    # PT40H0M0S -> working days (8h/day)
    m = re.fullmatch(r"-?PT(\d+)H(\d+)M(\d+)S", iso or "")
    if not m:
        return iso
    h = int(m[1]) + int(m[2]) / 60
    return f"{'-' if iso.startswith('-') else ''}{h / 8:g}d"


def tenths_days(v):
    return f"{int(v) / TENTHS_PER_DAY:g}d" if v not in (None, "") else "-"


def elapsed_days(v):
    # LevelingDelay is exported with LevelingDelayFormat 8 (elapsed days): 1440 min * 10 per day
    return f"{int(v) / 14400:g}ed" if v not in (None, "") else "-"


def short(dt):
    return dt[5:16].replace("T", " ") if dt else "-"


def load(path):
    root = ET.parse(path).getroot()
    tasks = {}
    for t in root.findall("p:Tasks/p:Task", NS):
        if txt(t, "OutlineLevel") == "0":
            continue  # project summary task
        uid = txt(t, "UID")
        tasks[uid] = dict(
            name=txt(t, "Name"), start=txt(t, "Start"), finish=txt(t, "Finish"),
            dur=dur_days(txt(t, "Duration")), es=txt(t, "EarlyStart"), ef=txt(t, "EarlyFinish"),
            ls=txt(t, "LateStart"), lf=txt(t, "LateFinish"), tslack=txt(t, "TotalSlack"),
            fslack=txt(t, "FreeSlack"), crit=txt(t, "Critical"), lvl=txt(t, "LevelingDelay"),
            prio=txt(t, "Priority"), segs=[],
        )
    for a in root.findall("p:Assignments/p:Assignment", NS):
        uid = txt(a, "TaskUID")
        if uid not in tasks or txt(a, "ResourceUID") in (None, "-65535"):
            continue
        segs = []
        for td in a.findall("p:TimephasedData", NS):
            if txt(td, "Type") != "1":  # 1 = assignment remaining work
                continue
            s, f, v = txt(td, "Start"), txt(td, "Finish"), txt(td, "Value")
            if v and v != "PT0H0M0S":
                if segs and segs[-1][1] == s:
                    segs[-1][1] = f
                else:
                    segs.append([s, f])
        tasks[uid]["segs"] = segs
    return tasks


def table(tasks, title):
    print(f"  [{title}]")
    print(f"  {'tarefa':<15}{'dur':>5} {'inicio':>12} {'termino':>12} {'IniCedo':>12} {'IniTarde':>12} "
          f"{'FolgaT':>7}{'FolgaL':>7} {'crit':>4} {'AtrNiv':>7} {'prio':>5}  trechos")
    for t in tasks.values():
        segs = " | ".join(f"{short(s)}..{short(f)}" for s, f in t["segs"]) or "-"
        print(f"  {t['name']:<15}{t['dur']:>5} {short(t['start']):>12} {short(t['finish']):>12} "
              f"{short(t['es']):>12} {short(t['ls']):>12} {tenths_days(t['tslack']):>7}"
              f"{tenths_days(t['fslack']):>7} {t['crit']:>4} {elapsed_days(t['lvl']):>7} {t['prio']:>5}  {segs}")


def main(folder):
    for before in sorted(glob.glob(os.path.join(folder, "*-antes.xml"))):
        name = os.path.basename(before)[: -len("-antes.xml")]
        print(f"\n=== {name}")
        a = load(before)
        table(a, "antes")
        after = before.replace("-antes.xml", "-depois.xml")
        if os.path.exists(after):
            b = load(after)
            table(b, "depois do nivelamento")
            print("  [mudancas]")
            for uid, t in a.items():
                u = b[uid]
                diffs = [f"{k}: {t[k]} -> {u[k]}" for k in ("start", "finish", "lvl", "tslack", "crit") if t[k] != u[k]]
                if len(u["segs"]) != len(t["segs"]):
                    diffs.append(f"trechos: {len(t['segs'])} -> {len(u['segs'])}")
                print(f"  {t['name']:<15}{'; '.join(diffs) or 'sem mudanca'}")


main(sys.argv[1])
