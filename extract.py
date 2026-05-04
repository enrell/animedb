import re
import sys


def extract_functions(source_file, func_names, dest_file):
    with open(source_file, "r") as f:
        text = f.read()

    extracted = []

    for name in func_names:
        pattern = r"(?:pub )?fn " + name + r"\b(?:.*?)\{"
        match = re.search(pattern, text, re.DOTALL)
        if not match:
            print(f"Function {name} not found")
            continue

        start_idx = match.start()

        # find matching closing brace
        brace_count = 0
        in_string = False
        idx = start_idx

        while text[idx] != "{":
            idx += 1

        idx += 1
        brace_count = 1

        while idx < len(text) and brace_count > 0:
            if text[idx] == '"' and text[idx - 1] != "\\":
                in_string = not in_string
            elif not in_string and text[idx : idx + 2] == "//":
                while idx < len(text) and text[idx] != "\n":
                    idx += 1
                continue
            elif not in_string and text[idx : idx + 2] == "/*":
                while idx < len(text) and text[idx : idx + 2] != "*/":
                    idx += 1
                idx += 2
                continue

            if not in_string:
                if text[idx] == "{":
                    brace_count += 1
                elif text[idx] == "}":
                    brace_count -= 1

            idx += 1

        end_idx = idx

        # grab the doc comment if any
        # walk backwards from start_idx
        doc_start = start_idx
        while doc_start > 0:
            if text[doc_start - 1] in ("\n", " ", "\t"):
                doc_start -= 1
            elif text[doc_start - 3 : doc_start] == "///":
                doc_start -= 3
                while doc_start > 0 and text[doc_start - 1] != "\n":
                    doc_start -= 1
            elif text[doc_start - 4 : doc_start] == "#[":
                doc_start -= 4
                while doc_start > 0 and text[doc_start - 1] != "\n":
                    doc_start -= 1
            else:
                break

        func_text = text[doc_start:end_idx]
        extracted.append(func_text)

        text = text[:doc_start] + text[end_idx:]

    with open(source_file, "w") as f:
        f.write(text)

    with open(dest_file, "a") as f:
        for ex in extracted:
            # indent appropriately? No, they were at module level or inside impl block.
            # If they were inside impl AnimeDb, they had 4 spaces indent. Let's unindent.
            lines = ex.split("\n")
            unindented = [
                line[4:] if line.startswith("    ") else line for line in lines
            ]
            f.write("\n".join(unindented) + "\n\n")


if __name__ == "__main__":
    src = "crates/animedb/src/db.rs"
    dest = sys.argv[1]
    funcs = sys.argv[2:]
    extract_functions(src, funcs, dest)
