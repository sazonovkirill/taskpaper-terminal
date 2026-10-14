---
name: bug-hunt
description: Take a role in a taskpaper-terminal bug hunt round, as Bug Finder, Bug Reviewer or Bug Fixer, or run the round as the coordinator. Use when a brief names one of these roles or asks for a bug hunt.
---

# Bug hunt

The rules are in [the bug hunt](../../../docs/quality/bug-hunt.md); this is the procedure. Do your own role's steps and nothing else.

## Coordinator

1. Start Bug Finder A and Bug Finder B in parallel, each with its [set of areas](../../../docs/quality/bug-hunt.md#areas), swapped from the last round, plus the features added and the bugs fixed since then.
2. When both have reported, start the Bug Reviewer with their lists.
3. Ask the owner the Reviewer's NEEDS_CLARIFICATION questions. Split the WILL_FIX bugs by area and start Bug Fixer A and Bug Fixer B, naming each one's bugs and the latest `D-n` on `main`.
4. Report to the owner: questions, fixes and the severity trend. In the nightly run that is the morning report; re-arm both timers.

## Bug Finder

1. Clone or update `main` and build: `cargo build --release --manifest-path app/Cargo.toml`.
2. List the open Todoist tasks labelled `bug` in the project `taskpaper-terminal`.
3. Explore your areas with the [`explore-app` skill](../explore-app/SKILL.md), steps 2 to 5.
4. For each new bug, add a task in the Inbox section with the label `bug`, an honest priority, and a description that opens with `Found by Bug Finder <A|B>, <run> <date>, tested on main <commit> in tmux.` followed by **Repro**, **Expected** and **Actual**.
5. Reply in your thread with each bug's title, priority and link, or that you found nothing worth filing.

## Bug Reviewer

1. List the open `bug` tasks that carry none of WILL_FIX, NEEDS_CLARIFICATION and WILL_NOT_FIX.
2. For each, check the specs and Decisions table, and reproduce it when in doubt.
3. Set the task's labels to its existing labels plus exactly one verdict, set its final priority, and comment `Bug Reviewer: <LABEL>. <reason>` in one line. Move a WILL_NOT_FIX task to Ready.
4. Reply in your thread with each bug's verdict and priority, and the questions for the owner.

## Bug Fixer

1. For each bug in your brief, run the [`claim-task` skill](../claim-task/SKILL.md) on it.
2. Write a test or scenario that fails because of the bug, then fix it, and run the [`review-commit` skill](../review-commit/SKILL.md).
3. Record any new decision as the next `D-n`; renumber it if the other Fixer reached `main` with that number first.
4. Run the [`deliver-task` skill](../deliver-task/SKILL.md).
5. Reply in your thread with each bug's commit and any new decision.
