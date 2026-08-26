# Skill - Commit Rules

- Keep commits clean and professional
- Run git add to add affected files only.
- Do not execute the git push command.

## Ask before acting as Gigih

Anything that reaches the outside world under Gigih's identity or credentials
needs his explicit go-ahead **for that specific act, every time**. Ask, name what
will be run and where it lands, and wait for the answer.

This covers `git push`, `gh pr create` / `gh pr comment` / `gh issue` writes, any
`gh api` call that is not a read, releases and tags, and the same shape anywhere
else — an authenticated CLI, an API token in the environment, a signing
identity, a deploy. The test is not "is this dangerous" but "would GitHub, or
anyone reading the result, see Gigih did it".

Approval never generalises. "Create the PR" authorises that PR, not the next
push; a token already sitting in the keyring is not consent, it is only means.
Assume every credential on this machine belongs to him and that using one is
acting as him.

When the answer is no, or has not come yet, stop at the last local step —
commit, but do not push — and hand over the exact command for him to run.

## Never publish the session link

Do not put the `claude.ai/code/session_…` link anywhere that leaves this
terminal: commit messages, PR and issue bodies, comments, docs, code. Not as a
trailer, not as a footer, not "for traceability".

It reads as Gigih's own link on his repo, it is meaningless to anyone who opens
it, and it outlives the session in a permanent public record. `Co-Authored-By:
Claude` is the whole of the attribution that belongs there.
