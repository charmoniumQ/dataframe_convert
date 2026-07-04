import textwrap
import sys
import pathlib


target_file = pathlib.Path(sys.argv[1])
marker_start = "\n" + sys.argv[2] + "\n"
marker_end = "\n" + sys.argv[3] + "\n"
indentation = sys.argv[4]
block = pathlib.Path(sys.argv[5])

content = target_file.read_text()
if marker_start not in content:
    print(f"{marker_start!r} not in {target_file}")
if marker_end not in content:
    print(f"{marker_end!r} not in {target_file}")
before = content[:content.index(marker_start) + len(marker_start)]
after = content[content.index(marker_end):]
target_file.write_text(
    before + textwrap.indent(block.read_text().strip(), indentation, lambda _: True) + after[1:]
)
