---
name: commit
description: Analyze git changes and generate a conventional commit message
user_invocable: true
---

Analyze the current git changes and generate a commit message for the user to review.

Steps:

1. Run `git status` to see all changed and untracked files. If there are no changes, inform the user and stop.

2. Run `git diff --staged` to see staged changes. If nothing is staged, run `git diff` to see unstaged changes and also list untracked files.

3. Analyze all changes and draft a commit message following conventional commit style:
   - Format: `type(scope): description`
   - Types: feat, fix, chore, docs, refactor, test, style, perf
   - Scope: the module or area affected
   - Description: concise, in English, focus on "why" not "what"
   - Add a blank line and bullet-point body if there are multiple logical changes

4. Present the commit message to the user using AskUserQuestion with two options:
   - "Looks good, commit it" — proceed to commit with the generated message
   - "Let me adjust" — let the user provide their own message

5. Based on the user's choice:
   - If approved: stage the relevant files (prefer specific files over `git add -A`) and create the commit
   - If the user provides an adjusted message: use their message instead

6. Show the final commit hash and summary after committing.

Important:
- Do NOT stage `.DS_Store`, `.env`, or other files that should be ignored.
- Do NOT push to remote. Only commit locally.
