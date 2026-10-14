---
name: explore-app
description: Use the real taskpaper-terminal app in a terminal the way a user would, to find bugs the tests miss, and turn what you find into scenarios. Use when asked to explore, try out or test the app by hand, or to grow the scenario library.
---

# Explore the app

The rules are in [the testing strategy](../../../docs/quality/testing-strategy.md#scenarios); this is the procedure.

1. Build the binary: from the repository root, `cargo build --release --manifest-path app/Cargo.toml` gives `app/target/release/tpt`.
2. Read the specs for the area you explore (`docs/specs/`, and the Decisions table of `docs/functional-requirements.md`, whose later decisions win).
3. Start the app in tmux on a file of your own, with every folder it writes to in a temporary directory, and keep the pane open after it exits so a panic stays readable:

   ```sh
   T=$(mktemp -d); printf 'Errands:\n\t- Post office\n' > "$T/o.taskpaper"
   tmux new-session -d -s explore-1 -x 100 -y 30 "env HOME=$T XDG_CONFIG_HOME=$T/config XDG_DATA_HOME=$T/data XDG_STATE_HOME=$T/state LANG=C.UTF-8 TERM=xterm-256color $PWD/app/target/release/tpt $T/o.taskpaper; sleep 30"
   ```

4. Drive it: `tmux send-keys -t explore-1 j` for a key (`C-q`, `Escape`, `Enter`, `Tab`, `BTab`, `Up`, `M-x` for `Alt+x`, `Space`), `tmux send-keys -t explore-1 -l 'Buy milk'` for text, `tmux set-buffer 'text'` then `tmux paste-buffer -p -t explore-1` for a paste, `tmux resize-window -t explore-1 -x 40 -y 10` to resize. Read the screen with `tmux capture-pane -p -t explore-1` after a short pause, and the file with `cat`. Pause about 0.1 s after `Escape`, or the next key arrives as an `Alt` chord. tmux has no kitty keyboard protocol, so only the classic key variants are reachable.
5. Use the app for real tasks, combining features, with plausible edge cases: an empty file, deep nesting, wide and combining characters, tiny terminals, fast typing, quitting and reopening. Judge each screen and each saved file against the spec.
6. For each bug, confirm it against the spec, reduce it to the shortest sequence of keys, and write it as a scenario under `app/tests/scenarios/library/<area>/` with a `# BUG:` line. For each behavior that works and no scenario covers, write a passing scenario. Run `bash app/scripts/validate.sh scenarios <area>` and make every new scenario pass or fail as intended.
7. Fix bugs within your task as the agent workflow requires, removing the `# BUG:` line in the same commit; report the others to the owner with their scenarios. As a Bug Finder, skip steps 6 and 7: file each bug in Todoist instead, as [the bug hunt](../../../docs/quality/bug-hunt.md#bug-finder) describes.
