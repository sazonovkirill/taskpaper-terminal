# taskpaper-terminal

A terminal editor for `.taskpaper` files: an infinite, Workflowy-like outliner built in Rust with [Ratatui](https://ratatui.rs).

## Layout

- `app/` — the application (Rust crate: source code, resources)
- `docs/` — requirements, specs and references ([index](docs/README.md))

Agents start with [AGENTS.md](AGENTS.md), which links the workflow and quality rules in [docs/quality/](docs/quality/README.md).

## Running

```sh
cd app
cargo run
```

Settings, such as key bindings, live in `~/.config/taskpaper-terminal/config.toml` (`$XDG_CONFIG_HOME` respected, also on macOS); see [keybindings.md §5](docs/specs/keybindings.md#5-configuration).
