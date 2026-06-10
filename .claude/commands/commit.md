Create a git commit using Conventional Commits format.

1. Run `git status` and `git diff` (staged and unstaged) in parallel to review all changes.
2. Stage relevant files by name (avoid `git add -A` or `git add .`).
3. Run the e2e tests from `client/python/`: `make test-e2e-null`
   - If they pass: use `type(scope): message`
   - If they fail: prefix the message with `WIP: ` → `WIP: type(scope): message`
4. Infer the commit type and scope from the changed files:
   - Use compound scopes when changes span two sub-areas: `server/render`, `server/scene`, `server/ipc`
   - Common scopes: `server`, `server/render`, `server/scene`, `server/ipc`, `proto`, `python-client`, `python-client/psychopy`, `docs`
5. Commit using a HEREDOC:

```bash
git commit -m "$(cat <<'EOF'
type(scope): short description

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
```

6. Run `git status` to confirm success.

Do not skip hooks (`--no-verify`). Do not amend unless explicitly asked.
