"""Preserve IM's source path/circle/rect geometry without executing its JS.

prepare-resources.py calls geometry_for for each extracted groupList key. The
standalone entry point reruns preparation so hashes and embedding stay in sync.
"""
import json
from pathlib import Path
import re


def path_commands(source):
    tokens = re.findall(r"[A-Za-z]|[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?", source)
    commands = []
    index = 0
    current = [0.0, 0.0]
    start = current[:]
    command = None

    def coordinate(relative):
        nonlocal index
        point = [float(tokens[index]), float(tokens[index + 1])]
        index += 2
        if relative:
            point = [point[axis] + current[axis] for axis in (0, 1)]
        return [round(value, 6) for value in point]

    while index < len(tokens):
        if tokens[index].isalpha():
            command = tokens[index]
            index += 1
        if command is None or command not in "MmLlHhVvCcZz":
            raise ValueError(f"Unaudited SVG command {command!r}")
        relative = command.islower()
        if command.upper() == "Z":
            commands.append({"op": "close"})
            current = start[:]
            command = None
        elif command.upper() in ("M", "L"):
            current = coordinate(relative)
            commands.append({"op": "move" if command.upper() == "M" else "line", "points": current[:]})
            if command.upper() == "M":
                start = current[:]
                command = "l" if relative else "L"
        elif command.upper() in ("H", "V"):
            axis = 0 if command.upper() == "H" else 1
            value = float(tokens[index])
            index += 1
            current[axis] = round(value + (current[axis] if relative else 0), 6)
            commands.append({"op": "line", "points": current[:]})
        elif command.upper() == "C":
            points = [coordinate(relative) for _ in range(3)]
            commands.append({"op": "curve", "points": points})
            current = points[-1][:]
    return commands


def geometry_for(key):
    if "d" in key:
        return {"kind": "path", "commands": path_commands(key["d"])}
    if "r" in key:
        return {"kind": "circle", "center": [key["cx"], key["cy"]], "radius": key["r"]}
    if all(name in key for name in ("x", "y", "width", "height")):
        if key.get("rx") or key.get("ry") or key.get("transform"):
            raise ValueError("Audit the new rounded/transformed rectangle before generating it")
        return {"kind": "rect", "origin": [key["x"], key["y"]], "size": [key["width"], key["height"]]}
    raise ValueError(f"No IM geometry for {key['buttonKey']}")


if __name__ == "__main__":
    import runpy
    runpy.run_path(str(Path(__file__).with_name("prepare-resources.py")), run_name="__main__")
