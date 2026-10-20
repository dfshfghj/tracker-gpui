#!/usr/bin/env python3
"""Create MSVC import libraries from MinGW-built FFmpeg DLLs.

Usage:
    make-msvc-import-libs.py <bin-dir> <lib-dir>

Requires x86_64-w64-mingw32-objdump and llvm-dlltool (or dlltool).
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REQUIRED = ("avcodec", "avformat", "avutil", "swscale", "swresample")


def find_tool(*names: str) -> str:
    for name in names:
        path = shutil.which(name)
        if path:
            return path
    raise SystemExit(f"required tool not found: {', '.join(names)}")


def export_names(objdump: str, dll: Path) -> list[str]:
    output = subprocess.check_output([objdump, "-p", str(dll)], text=True)
    names: list[str] = []
    in_table = False
    for line in output.splitlines():
        if "[Ordinal/Name Pointer] Table" in line:
            in_table = True
            continue
        if not in_table:
            continue
        match = re.match(r"^\s*\[\s*\d+\]\s+(\S+)\s*$", line)
        if match:
            names.append(match.group(1))
        elif names:
            break
    return names


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    bin_dir = Path(sys.argv[1]).resolve()
    lib_dir = Path(sys.argv[2]).resolve()
    lib_dir.mkdir(parents=True, exist_ok=True)

    objdump = find_tool("x86_64-w64-mingw32-objdump", "llvm-objdump", "objdump")
    dlltool = os.environ.get("LLVM_DLLTOOL") or find_tool(
        "llvm-dlltool",
        "llvm-dlltool-18",
        "llvm-dlltool-19",
        "x86_64-w64-mingw32-dlltool",
    )
    is_llvm = "llvm-dlltool" in Path(dlltool).name

    with tempfile.TemporaryDirectory() as temp:
        temp_dir = Path(temp)
        for dll in sorted(bin_dir.glob("*.dll")):
            match = re.match(r"^(%s)-\d+\.dll$" % "|".join(REQUIRED), dll.name)
            if not match:
                continue
            base = match.group(1)
            names = export_names(objdump, dll)
            if not names:
                raise SystemExit(f"no exports found in {dll}")
            def_path = temp_dir / f"{base}.def"
            def_path.write_text(
                f"LIBRARY {dll.name}\nEXPORTS\n" + "\n".join(names) + "\n",
                encoding="utf-8",
            )
            lib_path = lib_dir / f"{base}.lib"
            if is_llvm:
                command = [dlltool, "-m", "i386:x86-64", "-d", str(def_path), "-l", str(lib_path)]
            else:
                command = [dlltool, "-k", "-d", str(def_path), "-l", str(lib_path)]
            subprocess.run(command, check=True)
            print(f"{base}.lib <- {dll.name} ({len(names)} exports)")


if __name__ == "__main__":
    main()
