# Agent / AI coding rules (ua571)

You are working in a repository that may be **public open source** while production hosting stays in the maintainer’s **private AWS account**.

## Hard prohibitions

1. **Never commit or `git add -f`:**
   - `infra/cdk.context.json` (contains AWS account ID + hosted zone ID)
   - `infra/cdk.out/**`
   - `.env`, `.env.*` (except `*.example`)
   - AWS credentials files, `*.pem`, private keys
   - `web/pkg/**`, `target/**` (build outputs)

2. **Never paste into source or docs:**
   - Real AWS account IDs, access keys, secret keys, session tokens
   - Live role ARNs / bucket names / distribution IDs as “defaults” in code  
     (those belong in GitHub Environment variables only)

3. **Never disable security guards** without an explicit human request:
   - Do not delete `scripts/check-secrets.sh`, denylists, or the CI `secret-guard` job
   - Do not set `core.hooksPath` away from `.githooks` without asking

4. **Before any commit**, run:

   ```bash
   ./scripts/check-secrets.sh staged
   ```

   If it fails, fix the staging set — do not force-add denied files.

## Preferred deploy / infra edits

- Edit `infra/lib/web-stack.ts`, `infra/cdk.json` (public templates only)
- Use placeholders in docs: `ACCOUNT`, `BUCKET`, `DISTRIBUTION_ID`
- Leave `cdk deploy` outputs in the human’s AWS/GitHub settings, not in git

## If you need AWS context for local synth

`cdk.context.json` may exist **locally** after deploy. It is **gitignored**.  
Do not stage it. Do not “fix” ignore rules to track it.

## Quick verification

```bash
./scripts/install-git-hooks.sh   # once per clone
./scripts/check-secrets.sh all
git ls-files infra/cdk.context.json   # must print nothing
```
