#!/usr/bin/env python3
"""Print the pids in one Wrasse process's tree, one per line.

The tree is the app pid plus WebKit WebContent, GPU and Networking helpers that
belong to it. A helper belongs when any of these is true:

- its parent chain reaches the app
- responsibility_get_pid_responsible_for_pid returns the app
- launchd lists it under that app's services

Other apps' WebKit processes stay out. On a launch from a shell, macOS names
the shell (or Terminal) as the responsible process and launchd as the parent,
so the service list is what still ties the helpers to this app.
"""

import ctypes
import re
import subprocess
import sys

libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
_resp = libc.responsibility_get_pid_responsible_for_pid
_resp.argtypes = [ctypes.c_int32, ctypes.POINTER(ctypes.c_int32)]
_resp.restype = ctypes.c_int32

# A running instance ends in ".<UUID>". The pid-0 lines are the service templates.
_SERVICE = re.compile(
    r"^\s*(\d+)\s+-\s+com\.apple\.WebKit\.(WebContent|GPU|Networking)\.\S+\s*$"
)


def responsible(pid: int) -> int:
    """Responsible pid. This OS returns it and leaves the out-pointer untouched."""
    out = ctypes.c_int32(0)
    rc = _resp(int(pid), ctypes.byref(out))
    if rc > 0:
        return int(rc)
    return int(out.value) if out.value > 0 else 0


def parent_map() -> dict[int, int]:
    text = subprocess.check_output(["ps", "-ax", "-o", "pid=,ppid="], text=True)
    parents: dict[int, int] = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) == 2:
            parents[int(parts[0])] = int(parts[1])
    return parents


def descendants(root: int, parents: dict[int, int]) -> list[int]:
    children: dict[int, list[int]] = {}
    for pid, parent in parents.items():
        children.setdefault(parent, []).append(pid)
    found: list[int] = []
    stack = [root]
    seen = {root}
    while stack:
        cur = stack.pop()
        for kid in children.get(cur, []):
            if kid not in seen:
                seen.add(kid)
                found.append(kid)
                stack.append(kid)
    return found


def reaches(pid: int, root: int, parents: dict[int, int]) -> bool:
    seen: set[int] = set()
    cur = pid
    while cur and cur not in seen and cur not in (0, 1):
        if cur == root:
            return True
        seen.add(cur)
        cur = parents.get(cur, 0)
    return False


def launchd_helpers(root: int) -> list[int]:
    try:
        text = subprocess.check_output(
            ["launchctl", "print", f"pid/{root}"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return []
    pids: list[int] = []
    in_services = False
    for line in text.splitlines():
        if line.strip() == "services = {":
            in_services = True
            continue
        if not in_services:
            continue
        if line.strip() == "}":
            break
        match = _SERVICE.match(line)
        if match and int(match.group(1)) > 0:
            pids.append(int(match.group(1)))
    return pids


def webkit_pids() -> list[int]:
    try:
        text = subprocess.check_output(
            ["pgrep", "-f", r"com.apple.WebKit.(WebContent|GPU|Networking)"],
            text=True,
        )
    except subprocess.CalledProcessError:
        return []
    pids: list[int] = []
    for line in text.split():
        try:
            pids.append(int(line))
        except ValueError:
            continue
    return pids


def tree(root: int) -> list[int]:
    parents = parent_map()
    found = {root}
    found.update(descendants(root, parents))
    found.update(launchd_helpers(root))
    for pid in webkit_pids():
        if pid in found:
            continue
        # Bundled launches name the app itself as the responsible process.
        if responsible(pid) == root or reaches(pid, root, parents):
            found.add(pid)
    return sorted(found)


def main() -> None:
    if len(sys.argv) != 2:
        print("usage: wrasse-tree.py APP_PID", file=sys.stderr)
        sys.exit(2)
    root = int(sys.argv[1])
    for pid in tree(root):
        print(pid)


if __name__ == "__main__":
    main()
