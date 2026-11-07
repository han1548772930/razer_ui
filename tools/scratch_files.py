"""Completed analysis-input files admitted to the existing scratch cleanup plan."""
import json
from pathlib import Path

import code_identity as ci

HISTORY_FIELDS = {"temporary_files_created", "temporary_resources_created"}


def _analysis_file(workspace, name, directory):
    if not isinstance(name, str) or "\\" in name:
        raise ValueError("Scratch proof must use a project-relative POSIX path")
    relative = Path(name)
    if relative.is_absolute() or ".." in relative.parts or relative.parts[:2] != (".analysis", directory):
        raise ValueError("Scratch proof escaped its analysis owner")
    lexical = workspace.ROOT / relative
    for path in (lexical, *lexical.parents):
        if workspace._scratch_link(path):
            raise ValueError("Scratch proof contains a link or reparse point")
        if path == workspace.ROOT:
            break
    path = workspace.safe(lexical)
    if not path.is_file():
        raise ValueError("Scratch proof file is absent")
    return path


def _section(workspace, reference):
    if set(reference) != {"receipt", "pointer", "canonical_sha256"}:
        raise ValueError("Scratch completion requires an exact published section")
    path = _analysis_file(workspace, reference["receipt"], "results")
    pointer = reference["pointer"]
    if pointer == "":
        value = json.loads(path.read_bytes())
        if ci.digest(value) != reference["canonical_sha256"]:
            raise ValueError("Scratch completion canonical identity changed")
        return value
    if not isinstance(pointer, str) or pointer.count("/") != 1:
        raise ValueError("Scratch completion requires a top-level section pointer")
    for name, value in ci.stream_sections(path):
        if name == pointer[1:]:
            if ci.digest(value) != reference["canonical_sha256"]:
                raise ValueError("Scratch completion canonical identity changed")
            return value
    raise ValueError("Scratch completion section is absent")


def candidates(workspace, manifests):
    rows, seen = [], set()
    for name in manifests:
        path = _analysis_file(workspace, name, "inputs")
        document = json.loads(path.read_bytes())
        if set(document) != {"schema_version", "files"} or document["schema_version"] != 1:
            raise ValueError("Unknown owned scratch release contract")
        for item in document["files"]:
            if set(item) != {"path", "bytes", "sha256", "completion", "historical_references", "release_reason"}:
                raise ValueError("Owned scratch file requires identity, completion and release scope")
            target = workspace._clean_path(item["path"])
            if (target.parent != workspace.ROOT / ".work" or target.suffix != ".json"
                    or not target.is_file() or item["path"] in seen
                    or not isinstance(item["release_reason"], str) or not item["release_reason"].strip()):
                raise ValueError("Owned scratch target must be a distinct root JSON intermediate")
            identity = workspace._clean_file_identity(target)
            if any(identity[key] != item[key] for key in ("bytes", "sha256")):
                raise ValueError("Owned scratch input bytes changed")
            completion = _section(workspace, item["completion"])
            if (completion.get("target_execution") is not False or completion.get("failures") != []
                    or completion.get("changed_during_query") != []
                    or completion.get("snapshot_stable") is not True
                    or completion.get("source_slice_complete") is not True):
                raise ValueError("Owned scratch has no completed stable source review")
            for history in item["historical_references"]:
                if set(history) != {"section", "fields"} or not set(history["fields"]) <= HISTORY_FIELDS:
                    raise ValueError("Only explicit temporary-creation fields are historical")
                _section(workspace, history["section"])
            seen.add(item["path"])
            rows.append({"path": item["path"], "kind": "completed_analysis_input_file",
                         "basis": [workspace._clean_file_identity(path)], "owner_receipts": [],
                         "completed_file_manifest": name, "completion": item["completion"],
                         "historical_references": item["historical_references"],
                         "release_reason": item["release_reason"]})
    return rows


def reference_view(document, filename, row):
    """Ignore only creation history bound by the validated completion contract."""
    if row.get("kind") != "completed_analysis_input_file" or not isinstance(document, dict):
        return document
    # The release declaration and the selected file's own contents do not
    # create a future consumer. Every other file remains in the reference scan.
    if filename in {row["completed_file_manifest"], row["path"]}:
        return {}
    result = document
    for history in row["historical_references"]:
        reference = history["section"]
        if reference["receipt"] != filename:
            continue
        key = reference["pointer"][1:]
        value = document if reference["pointer"] == "" else document.get(key)
        if value is None or ci.digest(value) != reference["canonical_sha256"]:
            raise ValueError("Temporary-creation history changed during cleanup")
        if reference["pointer"] == "":
            return {name: value for name, value in document.items() if name not in history["fields"]}
        if result is document:
            result = dict(document)
        result[key] = {name: value for name, value in document[key].items()
                       if name not in history["fields"]}
    return result
