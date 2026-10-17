#!/usr/bin/env python3
"""Generate the Reference's key and settings tables from the app's source (docs/website.md W-REF-03).

Usage: python3 reference_tables.py              write site/data/keys.json and site/data/settings.json
       python3 reference_tables.py --check PUBLIC
                                                fail when those files are stale, or when the built
                                                site in PUBLIC does not render a command, a key
                                                table or a setting they list (W-REF-04)
(normally through `bash site/site.sh reference` and `bash site/site.sh check`)

Reads, from the Rust source under app/src:
  app/commands/registry.rs  REGISTRY (default bindings), NAVIGATOR (the Navigator's keys),
                            SHORTCUTS and Group::name (the `?` panel's groups)
  app/commands.rs           Command::name and the helpers it calls (the labels)
  ui/keymap.rs              the key tables of pickers, prompts and dialogs, and config names
  ui/settings.rs            LISTED and the settings window's group, name, description, kind and
                            default of each setting
  ui/config.rs              every key config.toml reads
  ui/theme/colors.rs        THEMES, the built-in themes' ids and names

The Rust readers and the key notation come from the printable cheat sheet's build script; the
cheat sheet itself is rendered from keys.json by templates/shortcodes/cheatsheet.html.

Hand-written text is limited to the topic each command group belongs to, the labels of the
dialog keys (whose inputs the app does not name) and one default the source leaves to run time.
A command, key table or setting the script cannot place or name stops it with an error.
"""

import json
import os
import re
import sys
import tempfile
from pathlib import Path

SITE = Path(__file__).resolve().parent.parent
REPO = SITE.parent
SRC = REPO / "app" / "src"
DATA = SITE / "data"

# --- Hand-written placement -----------------------------------------------------------------

# The Reference page (content/reference/<topic>.md) that lists each key group's commands.
GROUP_TOPICS = {
    "Moving": "navigation",
    "Folding": "navigation",
    "Zoom and History": "navigation",
    "Editing and Creating": "modes",
    "Selection": "structure",
    "Structure": "structure",
    "Block Metadata": "tags",
    "View and Panels": "navigator",
    "Editing": "modes",
    "Navigator": "navigator",
}
# Commands listed on another page than their group's, by config name.
COMMAND_TOPICS = {
    "go_to_today": "views", "go_to_upcoming": "views", "go_to_overdue": "views",
    "go_to_colors": "views", "go_to_logbook": "views", "go_to_starred": "views",
    "go_to_flagged": "views", "go_to_inbox": "navigator",
    "select_inbox_project": "navigator", "reset_inbox_project": "navigator",
    "quick_add": "quick-add",
    "open_attachments": "attachments",
    "open_link": "blocks", "convert_type": "blocks", "make_section": "blocks", "toggle_section": "blocks",
    "remove_section": "blocks", "count_blocks_and_words": "structure",
    "show_hide_completed": "tags", "toggle_word_wrap": "appearance", "choose_theme": "appearance",
    "open_settings": "settings", "animations_full": "animation", "animations_reduced": "animation",
    "animations_off": "animation", "start_weather_animation": "animation",
    "shortcuts": "modes", "scroll_shortcuts_down": "modes", "scroll_shortcuts_up": "modes",
    "keymap": "modes",
    "save_as": "files", "quit": "files",
    "narrow_pane": "panes", "widen_pane": "panes",
}
# Edit mode's commands follow their mode, not their group (Structure, View and Panels).
EDIT_TOPIC = "modes"
PANE_PREFIXES = ("open_in_new_pane", "open_location_in_new_pane", "focus_pane", "move_pane",
                 "close_pane")

# The key tables of ui/keymap.rs: their title, page and what each input does, keyed by the
# input as written in the table (whitespace removed).
CONTEXTS = {
    "TEXT": ("Text fields", "modes", {
        "go!(Left,false)": "Caret left", "go!(Right,false)": "Caret right",
        "go!(WordLeft,false)": "Word left", "go!(WordRight,false)": "Word right",
        "go!(Start,false)": "To the start", "go!(End,false)": "To the end",
        "go!(Left,true)": "Select left", "go!(Right,true)": "Select right",
        "go!(WordLeft,true)": "Select a word left", "go!(WordRight,true)": "Select a word right",
        "go!(Start,true)": "Select to the start", "go!(End,true)": "Select to the end",
        "TextEdit::DeleteBackward": "Delete before the caret",
        "TextEdit::DeleteForward": "Delete after the caret",
        "TextEdit::DeleteWordBackward": "Delete the word before the caret",
        "TextEdit::DeleteWordForward": "Delete the word after the caret",
        "TextEdit::DeleteToStart": "Delete to the start",
    }),
    "MENU": ("Pickers, palettes and menus", "navigator", {
        "MenuInput::Dismiss": "Close", "MenuInput::Choose": "Choose the highlighted entry",
        "MenuInput::Previous": "Previous entry", "MenuInput::Next": "Next entry",
        "MenuInput::Erase": "Erase a filter character",
    }),
    "OPEN_IN_NEW_PANE": ("Open in a new pane, from Go to Symbol and Go to Project", "navigator",
                         {"": "Open the highlighted block in a new pane"}),
    "SETTINGS": ("Settings window", "settings", {
        "SettingsInput::Next": "Next", "SettingsInput::Previous": "Previous",
        "SettingsInput::SwitchColumn": "Switch between groups and settings",
        "SettingsInput::Change": "Toggle, or open the value picker or prompt",
        "SettingsInput::Cycle(false)": "Previous choice, or toggle",
        "SettingsInput::Cycle(true)": "Next choice, or toggle",
        "SettingsInput::Reset": "Reset to the default", "SettingsInput::Filter": "Filter by name",
        "SettingsInput::Close": "Close",
    }),
    "SETTINGS_TEXT": ("Settings window: prompt and filter", "settings", {
        "SettingsInput::Accept": "Accept", "SettingsInput::Cancel": "Cancel",
        "SettingsInput::Next": "Next", "SettingsInput::Previous": "Previous",
    }),
    "DATE_PROMPT": ("Date prompt", "tags", {
        "DateInput::Apply": "Set the date", "DateInput::Cancel": "Cancel",
    }),
    "QUICK_ADD": ("Quick Add panel", "quick-add", {
        "QuickAddInput::Add": "Add the block", "QuickAddInput::Cancel": "Close, adding nothing",
        "QuickAddInput::ChooseProject": "Choose the project to add to",
    }),
    "NODE_EDITOR": ("Edit Node dialog", "tags", {
        "NodeInput::Accept": "Save every change and close", "NodeInput::Cancel": "Close, changing nothing",
        "NodeInput::Next": "Next row", "NodeInput::Previous": "Previous row",
        "NodeInput::CycleType": "Change the type",
    }),
    "ATTACHMENTS": ("Attachments panel", "attachments", {
        "AttachmentsInput::Close": "Close", "AttachmentsInput::Edit": "Edit the selected note or comment",
        "AttachmentsInput::AddComment": "Add a comment",
        "AttachmentsInput::DeleteComment": "Delete the selected comment",
    }),
    "SCROLL": ("Attachments panel: scrolling and selection", "attachments", {
        "ScrollBy::Down": "Down a line, or the next item", "ScrollBy::Up": "Up a line, or the previous item",
        "ScrollBy::PageDown": "Down a page, or the next item",
        "ScrollBy::PageUp": "Up a page, or the previous item",
        "ScrollBy::Top": "Top, with the first item", "ScrollBy::Bottom": "Bottom, with the last item",
    }),
    "NOTE_EDITOR": ("Note editor", "attachments", {
        "AttachmentsInput::Save": "Save the note and stop editing",
        "AttachmentsInput::Break": "Break the line",
        "AttachmentsInput::LineUp": "Line up", "AttachmentsInput::LineDown": "Line down",
    }),
    "SAVE_AS": ("Save As prompt", "files", {
        "SaveAs::Accept": "Save there", "SaveAs::Cancel": "Cancel",
        "SaveAs::Complete": "Complete the folder being typed",
    }),
    "CLOSE_KEYMAP": ("Keymap dialog", "modes", {"": "Close the Keymap dialog"}),
    "PROMPT": ("Conflict prompt", "files", {
        "|_|PromptKey::Send(Message::Quit)": "Quit",
        "|_|{PromptKey::Send(Message::Conflict(ConflictInput::KeepMine))}": "Keep mine",
        "|_|{PromptKey::Send(Message::Conflict(ConflictInput::TakeTheirs))}": "Take theirs",
        "|_|{PromptKey::Send(Message::Conflict(ConflictInput::ToggleDiff))}": "Show or hide the diff",
        "|_|PromptKey::Scroll(1)": "Scroll the diff down a line",
        "|_|PromptKey::Scroll(-1)": "Scroll the diff up a line",
        "PromptKey::Scroll": "Scroll the diff down a page",
        "|rows|{PromptKey::Scroll(rows.saturating_neg())}": "Scroll the diff up a page",
    }),
}
# Sequences a context's method handles before its table, by table and action.
CONTEXT_SEQUENCES = {"SCROLL": {"ScrollBy::Top": ["g g"]}}
# In the open Navigator the command that opens it closes it.
NAVIGATOR_LABELS = {"Command::ToggleNavigator": "Close Navigator"}
# Defaults the settings window takes at run time rather than from a constant.
RUNTIME_DEFAULTS = {"self.default_animations": ("full", "full, or reduced over SSH")}
# Settings listed on another page besides the Settings page, by key.
SETTING_TOPICS = {
    "theme": "appearance", "icons": "appearance", "word_wrap": "appearance",
    "today_starred": "views",
    "sidebar_upcoming": "views", "sidebar_overdue": "views", "sidebar_colors": "views",
    "sidebar_logbook": "views", "sidebar_starred": "views", "sidebar_flagged": "views",
    "attachments_notes": "attachments", "attachments_comments": "attachments",
    "animations": "animation", "weather": "animation", "weather_idle_minutes": "animation",
    "autosave_delay_ms": "files", "backups.enabled": "files", "backups.dir": "files",
    "backups.keep": "files",
}
# Keys config.toml reads that the settings window does not list, each documented in prose.
CONFIG_ONLY = {"keymap"}


class SourceError(Exception):
    pass


# --- Reading Rust source -------------------------------------------------------------------

def strip_comments(src):
    """The source without // comments, leaving string and char literals intact."""
    out, i, n = [], 0, len(src)
    while i < n:
        c = src[i]
        raw = re.match(r'r(#*)"', src[i:]) if c == "r" and not re.match(r"\w", src[i - 1 : i]) else None
        if raw:
            # No table the script reads holds a raw string, so it stands in as an empty one.
            i = src.index('"' + raw.group(1), i + len(raw.group(0))) + 1 + len(raw.group(1))
            out.append('""')
        elif c == '"':
            j = i + 1
            while src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            out.append(src[i : j + 1])
            i = j + 1
        elif c == "'" and re.match(r"'(\\.|[^\\'])'", src[i:]):
            m = re.match(r"'(\\.|[^\\'])'", src[i:])
            out.append(m.group(0))
            i += len(m.group(0))
        elif src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def scan(text):
    """Yield (index, char, depth) for characters outside string literals."""
    depth, i, n = 0, 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            j = i + 1
            while text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            i = j + 1
            continue
        if c == "'" and re.match(r"'(\\.|[^\\'])'", text[i:]):
            i += len(re.match(r"'(\\.|[^\\'])'", text[i:]).group(0))
            continue
        if c in "([{":
            yield i, c, depth
            depth += 1
        elif c in ")]}":
            depth -= 1
            yield i, c, depth
        else:
            yield i, c, depth
        i += 1


def matching(text, open_at):
    """Index of the bracket closing the one at open_at."""
    base = None
    for i, c, d in scan(text[open_at:]):
        if base is None:
            base = d
        elif c in ")]}" and d == base:
            return open_at + i
    raise SourceError("unbalanced brackets")


def split_top(text, sep):
    """Split on sep (a string) at bracket depth 0, outside strings; drop empty parts."""
    parts, start = [], 0
    marks = [i for i, c, d in scan(text) if d == 0 and text.startswith(sep, i)]
    for i in marks:
        if sep == "|" and (text[i - 1 : i] == "|" or text[i + 1 : i + 2] == "|"):
            continue
        parts.append(text[start:i])
        start = i + len(sep)
    parts.append(text[start:])
    return [p.strip() for p in parts if p.strip()]


def unescape(literal):
    body = literal[1:-1]
    return re.sub(r"\\(.)", lambda m: {"n": "\n", "t": "\t"}.get(m.group(1), m.group(1)), body)


STRING = r'"(\\.|[^"\\])*"'


def strings(text):
    return [unescape(m.group(0)) for m in re.finditer(STRING, text)]


def norm(expr):
    return re.sub(r"\s+", "", expr).rstrip(",")


def const_body(src, name):
    m = re.search(rf"const {name}\b[^=]*=\s*&?\[", src)
    if not m:
        raise SourceError(f"no `{name}` table found")
    start = m.end() - 1
    return src[start + 1 : matching(src, start)]


def tuples(body):
    """The top-level (...) tuples of a table body, each split into its fields."""
    return [split_top(item[1 : matching(item, 0)], ",")
            for item in split_top(body, ",") if item.startswith("(")]


def fn_body(src, name, after=0):
    m = re.compile(rf"fn {name}\s*(?:<[^>]*>)?\s*\(([^)]*)\)[^{{]*\{{").search(src, after)
    if not m:
        return None
    open_at = m.end() - 1
    params = [p.split(":")[0].strip() for p in split_top(m.group(1), ",")]
    return params, src[open_at + 1 : matching(src, open_at)]


class Sources:
    """The Rust files under app/src, comments stripped, read once."""

    def __init__(self, root):
        self.files = {str(p.relative_to(root)): strip_comments(p.read_text())
                      for p in sorted(root.rglob("*.rs"))}

    def __getitem__(self, path):
        return self.files[path]

    def method(self, method, variant):
        """The source whose `fn method(self)` has a `Self::variant` arm."""
        for src in self.files.values():
            found = fn_body(src, method)
            if found and found[0] == ["self"] and re.search(rf"Self::{variant}\b", found[1]):
                return src
        raise SourceError(f"no fn {method}(self) matching Self::{variant}")

    def function(self, name):
        """The source of a free `fn name`."""
        for src in self.files.values():
            if fn_body(src, name):
                return src
        raise SourceError(f"no fn {name}")

    def constant(self, name):
        """A constant's value: an integer, or a Duration in milliseconds."""
        aliases = [m.group(1) for src in self.files.values()
                   for m in [re.search(rf"(\w+) as {name}\b", src)] if m]
        target = aliases[0] if aliases else name
        for src in self.files.values():
            m = re.search(rf"const {target}\s*:[^=]*=\s*([^;]+);", src)
            if m:
                value = norm(m.group(1))
                for pattern, scale in ((r"([\d_]+)", 1), (r"Self\(([\d_]+)\)", 1),
                                       (r"Duration::from_millis\(([\d_]+)\)", 1),
                                       (r"Duration::from_secs\(([\d_]+)\)", 1000)):
                    found = re.fullmatch(pattern, value)
                    if found:
                        return int(found.group(1).replace("_", "")) * scale
                raise SourceError(f"cannot read the value of const {name}: {value}")
        raise SourceError(f"no const {name}")


# --- Evaluating names ----------------------------------------------------------------------

def parse_term(text):
    """A path with optional (args): returns (last path segment, [args])."""
    text = norm(text)
    m = re.fullmatch(r"([\w:]+)(?:\((.*)\))?", text)
    if not m:
        raise SourceError(f"cannot read `{text}`")
    head = m.group(1).split("::")[-1]
    args = [parse_term(a) for a in split_top(m.group(2), ",")] if m.group(2) else []
    return head, args


def is_binding(head):
    return re.fullmatch(r"[a-z_][a-z0-9_]*", head) and head not in ("true", "false")


def unify(pattern, term, binds):
    head, args = pattern
    if head == "_" and not args:
        return True
    if is_binding(head) and not args:
        binds[head] = term
        return True
    return head == term[0] and len(args) == len(term[1]) and all(
        unify(p, t, binds) for p, t in zip(args, term[1]))


def match_parts(text):
    """(scrutinee, arms text) of a `match x { ... }` expression."""
    m = re.match(r"\s*match\s+(\w+)\s*\{", text)
    if not m:
        raise SourceError("not a match")
    open_at = m.end() - 1
    return m.group(1), text[open_at + 1 : matching(text, open_at)]


def arms(text):
    """The (pattern, expression) arms of a match body; an arm whose expression is a block needs
    no comma after it."""
    out, i = [], 0
    while text[i:].strip():
        arrow = next(j for j, c, d in scan(text) if j >= i and d == 0 and text.startswith("=>", j))
        pattern, start = text[i:arrow].strip(), arrow + 2
        while text[start].isspace():
            start += 1
        if text[start] == "{":
            end = matching(text, start) + 1
        else:
            end = next((j for j, c, d in scan(text) if j >= start and d == 0 and c == ","), len(text))
        out.append((pattern, text[start:end].strip()))
        i = end
        while i < len(text) and (text[i].isspace() or text[i] == ","):
            i += 1
    return out


def render_term(term):
    head, args = term
    return head + (f"({', '.join(render_term(a) for a in args)})" if args else "")


class Raw(str):
    """A tuple part or argument kept as the Rust expression it is."""


class Evaluator:
    """Evaluates the small functions that name commands and settings: matches over enums, string
    literals, format!, tuples and calls of other such functions."""

    def __init__(self, sources, src):
        self.sources = sources
        self.src = src

    def call(self, name, values, src=None):
        src = self.src if src is None else src
        found = fn_body(src, name)
        if not found:
            src = self.sources.function(name)
            found = fn_body(src, name)
        params, body = found
        params = [p for p in params if p != "&self"]
        return self.block(body, dict(zip(params, values)), src)

    def block(self, body, env, src):
        statements = split_top(body, ";")
        for statement in statements[:-1]:
            m = re.match(r"let\s+(\(?[\w,\s]+\)?)\s*(?::[^=]+)?=\s*(.*)", statement, re.S)
            if not m:
                raise SourceError(f"cannot follow `{statement[:40]}`")
            value = self.eval(m.group(2), env, src)
            names = [n.strip() for n in m.group(1).strip("()").split(",") if n.strip()]
            if len(names) == 1 and not m.group(1).startswith("("):
                env[names[0]] = value
            else:
                env.update(zip(names, value))
        return self.eval(statements[-1], env, src)

    def eval(self, expr, env, src):
        expr = expr.strip()
        expr = re.sub(r"\.(into|to_owned|to_string)\(\)$", "", expr)
        if expr.startswith("{") and matching(expr, 0) == len(expr) - 1:
            return self.block(expr[1:-1], dict(env), src)
        if expr.startswith("match"):
            scrutinee, body = match_parts(expr)
            value = env[scrutinee]
            for pattern, result in arms(body):
                for alternative in split_top(pattern, "|"):
                    binds = {}
                    if unify(parse_term(alternative), value, binds):
                        return self.eval(result, {**env, **binds}, src)
            raise SourceError(f"no arm of `match {scrutinee}` for {render_term(value)}")
        if re.fullmatch(STRING, expr):
            return unescape(expr)
        if expr.startswith("(") and matching(expr, 0) == len(expr) - 1:
            return tuple(self.eval(part, env, src) if self.simple(part) else Raw(part)
                         for part in split_top(expr[1:-1], ","))
        m = re.fullmatch(r"format!\((.*)\)", expr, re.S)
        if m:
            fmt, *args = split_top(m.group(1), ",")
            values = iter(self.text(self.eval(a, env, src)) for a in args)
            return re.sub(r"\{(\w*)\}",
                          lambda g: self.text(env[g.group(1)]) if g.group(1) else next(values),
                          unescape(fmt))
        m = re.fullmatch(r"(\w+)\s*\+\s*(\d+)", expr)
        if m:
            return str(int(self.text(env[m.group(1)])) + int(m.group(2)))
        m = re.fullmatch(r"(\w+)\.(\w+)\(\)", expr)
        if m and m.group(1) in env:
            value = env[m.group(1)]
            return self.call(m.group(2), [value], self.sources.method(m.group(2), value[0]))
        m = re.fullmatch(r"self\.(\w+)\((.*)\)", expr, re.S)
        if m:
            return self.call(m.group(1), [self.eval(a, env, src) for a in split_top(m.group(2), ",")], src)
        m = re.fullmatch(r"(?:\w+::)*(\w+)\((.*)\)", expr, re.S)
        if m and m.group(1)[0].islower():
            args = [self.eval(a, env, src) if self.simple(a) else Raw(a) for a in split_top(m.group(2), ",")]
            if m.group(1) == "switch_on_by_default":
                group, name, description, _ = args
                return (group, name, description, "SettingKind::Switch", None,
                        "SettingValue::Switch(true)")
            return self.call(m.group(1), args, src)
        if expr in env:
            return env[expr]
        raise SourceError(f"cannot evaluate `{expr}`")

    @staticmethod
    def simple(part):
        """Whether a tuple part or argument is text to evaluate rather than an expression kept."""
        return bool(re.fullmatch(STRING, part.strip()) or re.fullmatch(r"\w+", part.strip())
                    or part.strip().startswith(("match", "format!")))

    @staticmethod
    def text(value):
        if isinstance(value, str):
            return value
        head, args = value
        if args or not head.isdigit():
            raise SourceError(f"{render_term(value)} is not text")
        return head


# --- Keys ----------------------------------------------------------------------------------
# Ports of parse_key, key_label, sequence_label and the kitty_only test helper in ui/keymap.rs.

NAMED = {
    "space": " ", "enter": "Enter", "esc": "Esc", "tab": "Tab", "backspace": "Backspace",
    "delete": "Delete", "up": "↑", "down": "↓", "left": "←", "right": "→", "home": "Home",
    "end": "End", "pageup": "PgUp", "pagedown": "PgDn", "menu": "Menu",
}


def parse_key(text):
    """(modifiers, key, is_char) as the keymap reads one key of the config notation."""
    if len(text) == 1 or "+" not in text:
        mods, name = [], text
    elif text.endswith("++"):
        mods, name = text[:-2].split("+"), "+"
    else:
        *mods, name = text.split("+")
    held = set()
    for modifier in mods:
        if modifier not in ("ctrl", "alt", "shift"):
            raise SourceError(f"not a key: {text}")
        held.add(modifier)
    if len(name) == 1 or re.fullmatch(r"\w–\w", name):
        c = name.upper() if "shift" in held else name
        return held - {"shift"}, c, True
    if name == "tab" and "shift" in held:
        return held, "BackTab", False
    if name in NAMED:
        return held, NAMED[name], False
    if re.fullmatch(r"f\d+", name):
        return held, name.upper(), False
    raise SourceError(f"not a key: {text}")


def key_caps(text):
    """The caps one key press reads as on screen, modifiers first."""
    held, key, is_char = parse_key(text)
    caps = [m.capitalize() for m in ("ctrl", "alt") if m in held]
    if (is_char and key.isupper() and held) or "shift" in held:
        caps.append("Shift")
    if key == " ":
        key = "Space"
    elif key == "BackTab":
        key = "Tab"
    elif is_char and "ctrl" in held:
        key = key.upper()
    return caps + [key]


def kitty_only(text):
    held, key, is_char = parse_key(text)
    if key == "Menu":
        return True
    if key in ("Enter", "Tab", "Backspace", "Esc") and not is_char:
        return bool(held & {"ctrl", "shift"})
    if is_char and "ctrl" in held:
        return not (key.isascii() and key.islower()) or key in "him"
    return False


def binding(keys):
    """A key sequence written as the config file writes it, as the site shows it: its label as
    the app's panels write it, each press's caps, and whether only the kitty protocol sends it."""
    presses = keys.split(" ")
    plain = all(len(p) == 1 and p != " " for p in presses)
    return {
        "label": ("" if plain else " ").join("+".join(key_caps(p)) for p in presses),
        "presses": [key_caps(p) for p in presses],
        "kitty": any(kitty_only(p) for p in presses),
        "alt": any("alt" in parse_key(p)[0] for p in presses),
    }


def config_name(name):
    """A command name as config.toml may write it: lower case, runs of other characters as `_`."""
    return "_".join(w for w in re.split(r"[^0-9a-z]+", name.lower()) if w)


KEY_NAMES = {"Esc": "esc", "Enter": "enter", "Tab": "tab", "BackTab": "tab", "Backspace": "backspace",
             "Delete": "delete", "Up": "up", "Down": "down", "Left": "left", "Right": "right",
             "Home": "home", "End": "end", "PageUp": "pageup", "PageDown": "pagedown"}


def key_macro(text):
    """A `key!(...)` of ui/keymap.rs in the config notation."""
    m = re.fullmatch(r"key!\(\s*((?:(?:ctrl|alt|shift)\s+)*)('(?:\\.|[^\\'])'|F\s+\d+|\w+)\s*\)", text.strip())
    if not m:
        raise SourceError(f"cannot read the key `{text}`")
    mods = m.group(1).split()
    name = m.group(2)
    if name.startswith("'"):
        name = unescape(name)
        name = "space" if name == " " else name
    elif re.fullmatch(r"F\s+\d+", name):
        name = "f" + name.split()[1]
    elif name in KEY_NAMES:
        name = KEY_NAMES[name]
        if m.group(2) == "BackTab" and "shift" not in mods:
            mods.append("shift")
    else:
        raise SourceError(f"no key name for `{name}`")
    mods = [x for x in ("ctrl", "alt", "shift") if x in mods]
    return "+".join([*mods, name])


# --- Building ------------------------------------------------------------------------------

def slug(text):
    return "-".join(w for w in re.split(r"[^0-9a-z]+", text.lower()) if w)


def build_keys(sources):
    registry_src = sources["app/commands/registry.rs"]
    names = Evaluator(sources, sources["app/commands.rs"])

    def name_of(command):
        return names.text(names.call("name", [parse_term(command)]))

    registry = []
    body = const_body(registry_src, "REGISTRY")
    for m in re.finditer(r"entry!\(", body):
        open_at = m.end() - 1
        command, mode, keys = split_top(body[open_at + 1 : matching(body, open_at)], ",")
        registry.append({"command": norm(command), "mode": norm(mode).split("::")[-1].lower(),
                         "keys": strings(keys)})
    shortcuts = []
    for mode, group, commands in tuples(const_body(registry_src, "SHORTCUTS")):
        listed = [norm(c) for c in split_top(commands.strip()[2:-1], ",")]
        shortcuts.append((norm(mode).split("::")[-1].lower(), norm(group).split("::")[-1], listed))
    _, group_body = fn_body(registry_src, "name", registry_src.index("impl Group"))
    group_names = {norm(p).split("::")[-1]: unescape(e.strip())
                   for p, e in arms(match_parts(group_body)[1])}

    commands = []
    for mode in ("navigate", "edit"):
        group_of, panel = {}, set()
        for listed_mode, group, listed in shortcuts:
            if listed_mode == mode:
                for command in listed:
                    group_of[command] = group_names[group]
                    panel.add(command)
        previous = None
        for entry in (e for e in registry if e["mode"] == mode):
            command = entry["command"]
            if command not in group_of:
                if previous is None:
                    raise SourceError(f"{command} in {mode} mode comes before any grouped command")
                group_of[command] = group_of[previous]
            previous = command
            name = name_of(command)
            # Keys cannot give a retype, which only the command menu offers (D-44).
            config = None if command.startswith("Command::Retype") else config_name(name)
            group = group_of[command]
            if mode == "edit":
                topic = EDIT_TOPIC
            elif config.startswith(PANE_PREFIXES):
                topic = "panes"
            elif config in COMMAND_TOPICS:
                topic = COMMAND_TOPICS[config]
            elif group in GROUP_TOPICS:
                topic = GROUP_TOPICS[group]
            else:
                raise SourceError(f"the key group {group!r} has no page; add it to GROUP_TOPICS")
            commands.append({
                "id": f"{mode}-{slug(config or name)}", "name": name, "config": config, "mode": mode,
                "group": group, "topic": topic, "panel": command in panel,
                "menu_only": command.startswith("Command::Retype"),
                "keys": [binding(k) for k in entry["keys"]], "raw": entry["keys"],
            })
    for command, keys in tuples(const_body(registry_src, "NAVIGATOR")):
        command = norm(command)
        name = NAVIGATOR_LABELS.get(command) or name_of(command)
        commands.append({
            "id": f"navigator-{slug(name)}", "name": name, "config": None, "mode": "navigator",
            "group": "Navigator", "topic": "navigator", "panel": False, "menu_only": False,
            "keys": [binding(k) for k in strings(keys)], "raw": strings(keys),
        })

    keymap_src = sources["ui/keymap.rs"]
    contexts = []
    for table, (title, topic, labels) in CONTEXTS.items():
        body = const_body(keymap_src, table)
        if table in ("OPEN_IN_NEW_PANE", "CLOSE_KEYMAP"):
            pairs = [(k, "") for k in split_top(body, ",")]
        else:
            pairs = [(key, norm(action)) for key, action in tuples(body)]
        rows = {}
        for key, action in pairs:
            if action not in labels:
                raise SourceError(f"{table} binds {key} to `{action}`, which CONTEXTS does not name")
            rows.setdefault(labels[action], []).append(binding(key_macro(key)))
        for action, sequences in CONTEXT_SEQUENCES.get(table, {}).items():
            rows[labels[action]] = [binding(keys) for keys in sequences] + rows[labels[action]]
        contexts.append({"id": f"keys-{slug(table)}", "table": table, "title": title,
                         "topic": topic, "rows": [{"label": label, "keys": keys}
                                                  for label, keys in rows.items()]})
    tables = set(re.findall(r"^const (\w+): &\[\(Key,", keymap_src, re.M))
    tables |= set(re.findall(r"^const (\w+): &\[PromptBinding\]", keymap_src, re.M))
    tables |= set(re.findall(r"^const (\w+): \[Key; \d+\]", keymap_src, re.M))
    missing = tables - set(CONTEXTS)
    if missing:
        raise SourceError(f"ui/keymap.rs has key tables CONTEXTS does not list: {sorted(missing)}")

    renamed = [{"old": unescape(o), "new": unescape(n)}
               for o, n in tuples(const_body(keymap_src, "RENAMED"))]
    timeout = sources.constant("SEQUENCE_TIMEOUT")
    order = {mode: list(dict.fromkeys(group_names[g] for m, g, _ in shortcuts if m == mode))
             for mode in ("navigate", "edit")}
    sheet = cheatsheet(commands, order)
    for c in commands:
        del c["raw"]
    return {"commands": commands, "contexts": contexts, "renamed": renamed,
            "sequence_timeout_ms": timeout, "cheatsheet": sheet}


def cheatsheet(commands, order):
    """The printable cheat sheet's cards (W-REF-07): navigate mode's groups, edit mode and the
    Navigator, with the merging of rows that build.py did."""
    def rows(selected):
        listed = [{"label": c["name"], "raw": c["raw"], "panel": c["panel"]}
                  for c in selected if c["raw"]]
        return [{"label": r["label"], "panel": r["panel"],
                 "keys": [binding(k) for k in r["raw"]],
                 "typed": [{"keys": binding(k), "label": w} for k, w in r.get("typed", [])]}
                for r in collapse_typed(collapse_ranges(listed))]

    def groups(mode):
        return order[mode]

    def of(mode, group=None):
        return [c for c in commands if c["mode"] == mode and group in (None, c["group"])]

    cards = [{"title": g, "sub": "navigate mode", "rows": rows(of("navigate", g)), "groups": []}
             for g in groups("navigate")]
    cards.append({"title": "Edit mode", "sub": "while typing in a block", "rows": [],
                  "groups": [{"title": g, "rows": rows(of("edit", g))} for g in groups("edit")]})
    cards.append({"title": "Navigator", "sub": "the window of places to go",
                  "rows": rows(of("navigator")), "groups": []})
    unbound = [c["name"] for c in of("navigate") if not c["raw"]]
    return {"cards": cards, "unbound": unbound}


def collapse_ranges(rows):
    """Merge runs of 3+ rows whose keys differ only in a last digit counting up by one."""
    def same_but_digit(a, b):
        if len(a["raw"]) != len(b["raw"]):
            return False
        return all(x[:-1] == y[:-1] and x[-1].isdigit() and y[-1].isdigit()
                   and int(y[-1]) == int(x[-1]) + 1 for x, y in zip(a["raw"], b["raw"]))

    out, i = [], 0
    while i < len(rows):
        j = i + 1
        while j < len(rows) and same_but_digit(rows[j - 1], rows[j]):
            j += 1
        if j - i >= 3:
            run = rows[i:j]
            span = f"{run[0]['raw'][0][-1]}–{run[-1]['raw'][0][-1]}"
            prefix = os.path.commonprefix([r["label"] for r in run])
            parts = [r["label"][len(prefix):] for r in run]
            digits = [r["raw"][0][-1] for r in run]
            label = (prefix + span if parts == digits
                     else f"{prefix.rstrip(': ')} {span}: " + ", ".join(parts))
            out.append({**run[0], "label": label, "raw": [k[:-1] + span for k in run[0]["raw"]],
                        "panel": any(r["panel"] for r in run)})
        else:
            out.extend(rows[i:j])
        i = j
    return out


def collapse_typed(rows):
    """Fold `k X` rows into the row bound to the prefix key `k`, as a shared "then" row."""
    heads = {r["raw"][0]: r for r in rows if len(r["raw"]) == 1 and " " not in r["raw"][0]}
    out, follow = [], {}
    for r in rows:
        if len(r["raw"]) == 1 and " " in r["raw"][0]:
            prefix, rest = r["raw"][0].split(" ", 1)
            head = heads.get(prefix)
            if head is not None and " " not in rest:
                word = " ".join(w for w in r["label"].split() if w not in head["label"].split())
                follow.setdefault(prefix, []).append((rest, (word or r["label"]).lower()))
                continue
        out.append(r)
    if not follow:
        return rows
    maps = list(follow.values())
    if all(m == maps[0] for m in maps):
        last = max(i for i, r in enumerate(out) if r["raw"] and r["raw"][0] in follow)
        out.insert(last + 1, {"label": "…then a type letter", "raw": [], "typed": maps[0],
                              "panel": False})
    else:
        for r in out:
            if r["raw"] and r["raw"][0] in follow:
                r["typed"] = follow[r["raw"][0]]
    return out


def build_settings(sources):
    src = sources["ui/settings.rs"]
    config_src = sources["ui/config.rs"]
    colors_src = sources["ui/theme/colors.rs"]
    themes = [{"id": unescape(t[0]), "name": unescape(t[1]), "truecolor": "Source::Terminal" not in t[2]}
              for t in tuples(const_body(colors_src, "THEMES"))]
    choice_lists = {
        "ICONS": [unescape(t[1]) for t in tuples(const_body(src, "ICONS"))],
        "animation::NAMES": [unescape(t[1]) for t in
                             tuples(const_body(sources["ui/animation.rs"], "NAMES"))],
    }
    listed = [parse_term(t) for t in split_top(const_body(src, "LISTED"), ",")]
    impl = src.index("impl Listed")
    evaluator = Evaluator(sources, src)
    key_params, key_body = fn_body(src, "key", impl)
    _, setting_body = fn_body(src, "setting", src.index("impl Tui"))
    setting_match = setting_body[setting_body.index("match listed"):]
    setting_match = setting_match[: matching(setting_match, setting_match.index("{")) + 1]

    settings = []
    for term in listed:
        key = evaluator.block(key_body, {"self": term}, src)
        group, name, description, kind_expr, _, default_expr = evaluator.eval(
            setting_match, {"listed": term}, src)
        if isinstance(description, Raw):
            description = strings(description)[-1]
        kind, choices, least, unit = setting_kind(norm(kind_expr), choice_lists, themes, sources)
        default, default_text = setting_default(norm(default_expr), kind, choices, sources)
        settings.append({
            "id": f"setting-{slug(key)}", "key": key, "group": group, "name": name,
            "description": description, "kind": kind, "choices": choices, "least": least,
            "unit": unit, "default": default, "default_text": default_text,
            "topic": SETTING_TOPICS.get(key),
        })

    read_body = fn_body(config_src, "read")[1]
    read_keys = set(re.findall(r'^\s*"(\w+)" =>', read_body, re.M))
    read_keys |= {f"backups.{k}" for k in
                  re.findall(r'^\s*"(\w+)" =>', fn_body(config_src, "read_backups")[1], re.M)}
    read_keys.discard("backups")
    read_keys |= {s["key"] for s in settings if s["key"].startswith(("sidebar_", "attachments_"))}
    unlisted = read_keys - {s["key"] for s in settings} - CONFIG_ONLY
    if unlisted:
        raise SourceError(f"config.toml reads {sorted(unlisted)}, which the settings window does "
                          "not list; document them and add them to CONFIG_ONLY")
    for view_key in re.findall(r'=> "(sidebar_\w+)"', config_src):
        if view_key not in {s["key"] for s in settings}:
            raise SourceError(f"config.toml reads {view_key}, which the settings window does not list")
    return {"settings": settings, "themes": themes, "config_only": sorted(CONFIG_ONLY),
            "groups": list(dict.fromkeys(s["group"] for s in settings))}


def setting_kind(expr, choice_lists, themes, sources):
    """(kind, choices, least, unit) of a SettingKind expression."""
    if expr == "SettingKind::Switch":
        return "switch", None, None, None
    m = re.fullmatch(r"choices\(&([\w:]+)\)", expr)
    if m:
        return "choice", choice_lists[m.group(1)], None, None
    if expr.startswith("SettingKind::Choice(BuiltIn::all()"):
        return "choice", [t["id"] for t in themes], None, None
    m = re.fullmatch(r"SettingKind::Number\{least:(.+),unit:(\".*\"),?\}", expr)
    if m:
        least = m.group(1)
        if not least.isdigit():
            const = re.search(r"(\w+)\.as_millis\(\)", least) or re.search(r"(\w+)\)*$", least)
            least = sources.constant(const.group(1))
        return "number", None, int(least), unescape(m.group(2))
    if expr.startswith("SettingKind::Text"):
        return "text", None, None, None
    raise SourceError(f"cannot read the setting kind `{expr}`")


def setting_default(expr, kind, choices, sources):
    """(value as config.toml writes it, text) of a setting's default."""
    for marker, (value, text) in RUNTIME_DEFAULTS.items():
        if marker in expr:
            return json.dumps(value), text
    m = re.fullmatch(r"SettingValue::Switch\((true|false)\)", expr)
    if m:
        return m.group(1), "on" if m.group(1) == "true" else "off"
    m = re.fullmatch(r"SettingValue::Choice\((.+)\)", expr)
    if m:
        index = m.group(1)
        if not index.isdigit():
            index = sources.constant(re.match(r"\w+::(\w+)", index).group(1))
        value = choices[int(index)]
        return json.dumps(value), value
    m = re.fullmatch(r"SettingValue::Number\((.+)\)", expr)
    if m:
        const = re.search(r"(\w+)(?:\.as_millis\(\))?\)*$", m.group(1)).group(1)
        value = sources.constant(const)
        return str(value), str(value)
    if expr == "SettingValue::Text(String::new())":
        return None, "the default folder"
    raise SourceError(f"cannot read the default `{expr}`")


# --- Writing and checking ------------------------------------------------------------------

def generate(out):
    sources = Sources(SRC)
    out.mkdir(parents=True, exist_ok=True)
    for name, data in (("keys.json", build_keys(sources)), ("settings.json", build_settings(sources))):
        (out / name).write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n")


def check(public):
    problems = []
    with tempfile.TemporaryDirectory() as scratch:
        generate(Path(scratch))
        for name in ("keys.json", "settings.json"):
            if (Path(scratch) / name).read_text() != (DATA / name).read_text():
                problems.append(f"site/data/{name} is stale: run `bash site/site.sh reference`")
    keys = json.loads((DATA / "keys.json").read_text())
    settings = json.loads((DATA / "settings.json").read_text())
    rendered = set()
    for page in (public / "reference").rglob("*.html"):
        rendered |= set(re.findall(r'\bid="([^"]+)"', page.read_text()))
    wanted = [c["id"] for c in keys["commands"]] + [c["id"] for c in keys["contexts"]]
    wanted += [s["id"] for s in settings["settings"]] + [f"setting-{k}" for k in settings["config_only"]]
    wanted += [f"theme-{t['id']}" for t in settings["themes"]]
    missing = [w for w in wanted if w not in rendered]
    if missing:
        problems.append("the Reference renders no row for: " + ", ".join(missing))
    for problem in problems:
        print(f"reference_tables.py: {problem}", file=sys.stderr)
    return not problems


def main(argv):
    try:
        if argv[:1] == ["--check"] and len(argv) == 2:
            sys.exit(0 if check(Path(argv[1])) else 1)
        if argv:
            sys.exit(__doc__)
        generate(DATA)
    except SourceError as error:
        sys.exit(f"reference_tables.py: {error}")


if __name__ == "__main__":
    main(sys.argv[1:])
