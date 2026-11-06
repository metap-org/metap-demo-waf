"""Render docs/architecture/**/*.md into one self-contained HTML page for viewing.

Usage (Python >= 3.9, `pip install -r docs/overview/tools/requirements.txt`):

    python3 docs/overview/tools/render_arch.py            # defaults below
    python3 render_arch.py <docs/architecture dir> <out.html>

Defaults: source = docs/architecture, output = docs/overview/v1-waap-architecture.html,
both resolved relative to this script, so it works from any working directory.
"""
import html
import os
import re
import sys
import urllib.parse

from markdown_it import MarkdownIt

_HERE = os.path.dirname(os.path.abspath(__file__))
if len(sys.argv) == 3:
    SRC, OUT = sys.argv[1], sys.argv[2]
elif len(sys.argv) == 1:
    SRC = os.path.join(_HERE, "..", "..", "architecture")
    OUT = os.path.join(_HERE, "..", "v1-waap-architecture.html")
else:
    sys.exit(__doc__)


def collect():
    files = []
    for root, _, names in os.walk(SRC):
        for n in names:
            if n.endswith(".md") and n != "README.md":
                files.append(os.path.relpath(os.path.join(root, n), SRC))

    def key(p):
        parts = p.split(os.sep)
        if p == "index.md":
            return (0,)
        # 00-index first inside each folder, level-3 after its parent's files
        return (1,) + tuple((0 if x == "00-index.md" else 1, x) for x in parts)

    return sorted(files, key=key)


def file_id(path):
    return "f-" + re.sub(r"[^a-z0-9]+", "-", path.lower().removesuffix(".md")).strip("-")


def gh_slug(text):
    s = text.strip().lower()
    s = "".join(ch for ch in s if ch.isalnum() or ch in " -_")
    return s.replace(" ", "-")


md = MarkdownIt("commonmark", {"html": False}).enable("table").enable("strikethrough")


def render_fence(self, tokens, idx, options, env):
    t = tokens[idx]
    lang = (t.info or "").strip().split(" ")[0]
    body = html.escape(t.content)
    if lang == "mermaid":
        return f'<div class="diagram"><pre class="mermaid">{body}</pre></div>\n'
    cls = f' class="lang-{html.escape(lang)}"' if lang else ""
    return f"<pre class=\"code\"><code{cls}>{body}</code></pre>\n"


md.add_render_rule("fence", render_fence)


def resolve_link(href, cur):
    if re.match(r"^[a-z]+:", href):
        return href, True
    path, _, frag = href.partition("#")
    path, frag = urllib.parse.unquote(path), urllib.parse.unquote(frag)
    if not path:
        target = cur
    else:
        target = os.path.normpath(os.path.join(os.path.dirname(cur), path))
        if target.endswith(os.sep) or os.path.isdir(os.path.join(SRC, target)):
            target = os.path.join(target, "00-index.md")
    if not target.endswith(".md"):
        return href, True
    fid = file_id(target)
    return (f"#{fid}--{frag}" if frag else f"#{fid}"), False


def render_file(path):
    text = open(os.path.join(SRC, path), encoding="utf-8").read()
    tokens = md.parse(text)
    fid = file_id(path)
    title = path
    for i, t in enumerate(tokens):
        if t.type == "heading_open":
            inline = tokens[i + 1]
            label = inline.content
            if t.tag == "h1" and title == path:
                title = label.replace("`", "")
            t.attrSet("id", f"{fid}--{gh_slug(label)}")
        if t.type == "inline" and t.children:
            for c in t.children:
                if c.type == "link_open":
                    new, external = resolve_link(c.attrGet("href"), path)
                    c.attrSet("href", new)
                    if external:
                        c.attrSet("target", "_blank")
                        c.attrSet("rel", "noopener")
    body = md.renderer.render(tokens, md.options, {})
    return fid, title, body


files = collect()
sections, nav = [], []
current_group = None
for p in files:
    fid, title, body = render_file(p)
    group = p.split(os.sep)[0] if os.sep in p else ""
    depth = p.count(os.sep)
    sections.append(
        f'<section class="doc" id="{fid}"><div class="path">{html.escape(p)}</div>{body}</section>'
    )
    is_folder_index = p.endswith("00-index.md")
    cls = "top" if (depth <= 1 and is_folder_index) or p == "index.md" else "sub"
    if "08-components" in p and not p.endswith("08-components/00-index.md"):
        cls = "sub2"
    nav.append(f'<a class="{cls}" href="#{fid}">{html.escape(title)}</a>')

template = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "arch_template.html"), encoding="utf-8").read()
out = template.replace("{{NAV}}", "\n".join(nav)).replace("{{CONTENT}}", "\n".join(sections))
open(OUT, "w", encoding="utf-8").write(out)
print(f"{len(files)} files -> {OUT} ({len(out)//1024} KB)")
