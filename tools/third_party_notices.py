"""Writes THIRD-PARTY-NOTICES.txt: every library built into Etapix's programs, with its license.

The MIT, BSD, Apache and OFL licenses all ask that their notice travel with the binaries, so
the installer and the Linux package carry this file. Run after changing dependencies:

    python -I tools/third_party_notices.py            # rewrite the file
    python -I tools/third_party_notices.py --check    # fail if it is out of date (CI)

Libraries come from `cargo metadata`: the normal (not dev or build) dependencies of the
workspace's programs, on every platform. Each one's license files are copied from its source.
"""
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "THIRD-PARTY-NOTICES.txt"
FONTS = ROOT / "app" / "assets" / "fonts"
LICENSE_FILES = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE")


def shipped_packages(meta):
    """Packages reachable from the workspace through normal dependencies, workspace excluded."""
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    members = set(meta["workspace_members"])
    seen, todo = set(), list(members)
    while todo:
        node = nodes[todo.pop()]
        for dep in node["deps"]:
            normal = any(k["kind"] is None for k in dep["dep_kinds"])
            if normal and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                todo.append(dep["pkg"])
    packages = {p["id"]: p for p in meta["packages"]}
    return sorted((packages[i] for i in seen - members), key=lambda p: (p["name"].lower(), p["version"]))


STANDARD = ROOT / "tools" / "licenses"
# When a library offers several licenses, its notice cites the first of these it offers.
PREFERRED = ("MIT", "Apache-2.0", "Zlib", "BSL-1.0", "CC0-1.0")


def is_license_file(f):
    name = f.name.upper()
    return f.is_file() and (name.startswith(LICENSE_FILES) or "LICENSE" in name or name in ("OFL.TXT", "UFL.TXT"))


def license_texts(package):
    """The license files a library ships, at its root or one folder down (e.g. fonts/OFL.txt)."""
    folder = pathlib.Path(package["manifest_path"]).parent
    files = [f for f in folder.iterdir() if is_license_file(f)]
    files += [f for d in folder.iterdir() if d.is_dir() and d.name not in ("src", "tests", "benches", "examples", "target") for f in d.iterdir() if is_license_file(f)]
    # Sorted by the relative path as text: Windows and Linux order Path objects differently.
    named = sorted((str(f.relative_to(folder)).replace("\\", "/"), f) for f in files)
    return [(name, f.read_text(encoding="utf-8", errors="replace").strip()) for name, f in named]


def standard_license(expression):
    """The license this notice relies on for a library that ships no license file."""
    for spdx in PREFERRED:
        if spdx in (expression or ""):
            return spdx
    return None


def notices():
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT, capture_output=True, text=True, encoding="utf-8", check=True).stdout)
    out = [
        "AVISOS DE TERCEIROS — Etapix",
        "",
        "O Etapix é distribuído sob as licenças MIT ou Apache-2.0 (LICENSE-MIT, LICENSE-APACHE).",
        "Ele inclui as fontes e bibliotecas abaixo, cada uma sob a sua licença.",
        "",
        "=" * 78,
        "FONTES",
        "=" * 78,
    ]
    for ofl in sorted(FONTS.glob("OFL-*.txt"), key=lambda f: f.name):
        out += ["", f"--- {ofl.stem.removeprefix('OFL-')} (SIL Open Font License 1.1) ---", "",
                ofl.read_text(encoding="utf-8", errors="replace").strip()]
    out += ["", "=" * 78, "BIBLIOTECAS", "=" * 78]
    standard_used = set()
    for p in shipped_packages(meta):
        out += ["", f"--- {p['name']} {p['version']} · {p.get('license') or 'licença no arquivo'}"
                    + (f" · {p['repository']}" if p.get("repository") else "") + " ---"]
        texts = license_texts(p)
        for name, text in texts:
            out += ["", f"[{name}]", text]
        if not texts:
            spdx = standard_license(p.get("license"))
            if spdx is None:
                sys.exit(f"{p['name']}: sem arquivo de licença e sem texto padrão para {p.get('license')!r}")
            standard_used.add(spdx)
            authors = ", ".join(p.get("authors") or []) or "os autores do projeto"
            out += ["", f"Copyright (c) {authors}. Distribuído sob {spdx}: texto no apêndice."]
    out += ["", "=" * 78, "APÊNDICE: TEXTOS PADRÃO DAS LICENÇAS", "=" * 78]
    for spdx in PREFERRED:
        if spdx in standard_used:
            source = ROOT / "LICENSE-APACHE" if spdx == "Apache-2.0" else STANDARD / f"{spdx}.txt"
            out += ["", f"--- {spdx} ---", "", source.read_text(encoding="utf-8").strip()]
    return "\n".join(out) + "\n"


def main():
    text = notices()
    if "--check" in sys.argv:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != text:
            old, new = current.splitlines(), text.splitlines()
            line = next((k for k, (a, b) in enumerate(zip(old, new)) if a != b), min(len(old), len(new)))
            shown = lambda lines: lines[line] if line < len(lines) else "(fim do arquivo)"
            sys.exit(f"THIRD-PARTY-NOTICES.txt está desatualizado (linha {line + 1}: {shown(old)!r} -> {shown(new)!r}): "
                     "rode python -I tools/third_party_notices.py")
        return
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"{OUT.name}: {text.count(chr(10) + '--- ') } itens")


if __name__ == "__main__":
    main()
