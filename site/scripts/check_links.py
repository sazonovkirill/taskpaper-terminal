#!/usr/bin/env python3
"""Fail when a built page links to a local file or anchor that does not exist.

Usage: python3 check_links.py site/public

Checks every href and src in every HTML file under the folder. Links that start with the
site's base URL or with / are resolved against the folder; external links are not fetched.
"""

import re
import sys
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class Page(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links, self.ids = [], set()

    def handle_starttag(self, tag, attrs):
        for name, value in attrs:
            if name in ("id", "name") and value:
                self.ids.add(value)
            elif name in ("href", "src") and value:
                self.links.append(value)
            elif name == "srcset" and value:
                self.links += [part.split()[0] for part in value.split(",") if part.strip()]


def target(public, base, page, link):
    """The file a link points to and its fragment, or None for an external link."""
    if link.startswith(base):
        link = "/" + link[len(base):].lstrip("/")
    parts = urlsplit(link)
    if parts.scheme or parts.netloc:
        return None
    path = unquote(parts.path)
    if not path:
        file = page
    elif path.startswith("/"):
        file = public / path.lstrip("/")
    else:
        file = page.parent / path
    if file.is_dir():
        file = file / "index.html"
    return file.resolve(), parts.fragment


def main():
    public = Path(sys.argv[1]).resolve()
    config = (public.parent / "config.toml").read_text()
    base = re.search(r'^base_url\s*=\s*"([^"]*)"', config, re.M).group(1).rstrip("/") + "/"
    pages = {}
    for file in public.rglob("*.html"):
        page = Page()
        page.feed(file.read_text())
        pages[file.resolve()] = page
    problems = []
    for file, page in pages.items():
        for link in page.links:
            if link.startswith(("mailto:", "data:", "javascript:")):
                continue
            found = target(public, base, file, link)
            if found is None:
                continue
            dest, fragment = found
            where = file.relative_to(public)
            if not dest.exists():
                problems.append(f"{where}: {link} does not exist")
            elif fragment and dest in pages and fragment not in pages[dest].ids:
                problems.append(f"{where}: {link} has no anchor #{fragment}")
    for problem in sorted(set(problems)):
        print(problem, file=sys.stderr)
    if problems:
        sys.exit(1)
    print(f"checked {sum(len(p.links) for p in pages.values())} links in {len(pages)} pages")


if __name__ == "__main__":
    main()
