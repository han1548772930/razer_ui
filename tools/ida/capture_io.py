"""Bounded-memory writers shared by static IDA capture datasets."""
import hashlib
import json


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def write_json_array(path, rows, *, progress=None, interval=25000):
    """Publish one valid JSON array without retaining the dataset's rows."""
    temporary = path.with_name(path.name + ".tmp")
    count = 0
    try:
        with temporary.open("w", encoding="utf-8", newline="\n") as stream:
            stream.write("[\n")
            for row in rows:
                if count:
                    stream.write(",\n")
                json.dump(row, stream, ensure_ascii=False)
                count += 1
                if progress is not None and count % interval == 0:
                    stream.flush()
                    progress(count)
            stream.write("\n]\n")
        temporary.replace(path)
        return count
    finally:
        temporary.unlink(missing_ok=True)


def write_function_aggregates(output, rows):
    """Publish full C/ASM files only after every function has been rendered."""
    paths = [output / "all-functions.c", output / "all-functions.asm"]
    temporary = [path.with_name(path.name + ".tmp") for path in paths]
    count = 0
    try:
        with temporary[0].open("w", encoding="utf-8", newline="\n") as pseudocode, \
             temporary[1].open("w", encoding="utf-8", newline="\n") as disassembly:
            for row in rows:
                header = "{} RVA {} VA {}".format(row["name"], row["rva"], row["address"])
                pseudocode.write("/* " + header + " */\n")
                if row["pseudocode"] is not None:
                    for line in row["pseudocode"]["lines"]:
                        pseudocode.write(line["text"] + "\n")
                else:
                    pseudocode.write("/* Hex-Rays did not decompile this function; see assembly and failure record. */\n")
                pseudocode.write("\n")
                disassembly.write("; " + header + "\n")
                for line in row["assembly"] or []:
                    disassembly.write("{}  {}  {}\n".format(line["address"], line["bytes"], line["text"]))
                disassembly.write("\n")
                count += 1
        for source, destination in zip(temporary, paths):
            source.replace(destination)
        return count
    finally:
        for path in temporary:
            path.unlink(missing_ok=True)
