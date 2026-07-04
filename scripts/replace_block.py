import textwrap
import sys
import pathlib


target_file = pathlib.Path(sys.argv[1])
marker_start = sys.argv[2]
marker_end = sys.argv[3]
indentation = sys.argv[4]
block = pathlib.Path(sys.argv[5])

content = target_file.read_text()
before = content[:content.index(marker_start) + len(marker_start)]
after = content[content.index(marker_end):]
target_file.write(
    before + "\n" + textwrap.indent(block.read_text(), indentation) + "\n" + after
)
