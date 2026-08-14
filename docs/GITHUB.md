# GitHub maintainer setup (public repo)

The source is meant to be **public**. Production AWS stays in the maintainer account.  
This page is the control-plane checklist — not end-user docs.

## Who can do what

| Actor | Push `main` | Merge PRs | Change settings | Deploy site |
|-------|-------------|-----------|-----------------|-------------|
| Maintainer (`danielendara`) | Via PR + CI (admin bypass only for emergencies) | Yes | Yes | Yes (OIDC, `main` only) |
| Outside PR / fork | No | No | No | **No** |
| Dependabot | Opens PRs only | No | No | No |

## Applied on the GitHub side

- **Default branch:** `main`
- **Merges:** squash (and rebase); merge commits off; delete head branch on merge
- **Homepage:** https://ua571.danielendara.com
- **Wiki / Projects:** off
- **Actions:** workflows get `contents: read` by default; they cannot approve PRs
- **Deploy:** `.github/workflows/deploy-web.yml` only runs for `danielendara/ua571` + `refs/heads/main`
- **Environment `production`:** holds `AWS_ROLE_ARN`, `UA571_S3_BUCKET`, `UA571_CLOUDFRONT_DISTRIBUTION_ID` (not in git)
- **CODEOWNERS:** `*` → `@danielendara`

## After the repo is public (rulesets unlock)

Free personal **private** repos cannot use branch protection / rulesets. Once public:

1. Ruleset **protect-main**
   - Target: `main`
   - Require a pull request (0 extra approvals — solo maintainer)
   - Require conversation resolution
   - Require status checks (strict): `secret-guard`, `ubuntu-latest`, `macos-latest`, `windows-latest`, `wasm32`
   - Block force-push and branch deletion
   - Bypass actor: maintainer (emergency only)
2. Environment `production`: deploy from `main` only
3. Enable **secret scanning** + **push protection**
4. Enable **private vulnerability reporting**
5. Re-run failed CI on open PRs (private-repo Actions minutes / billing)

## Going public (order)

1. Confirm `git ls-files | grep -E 'cdk.context|cdk.out|\.env$|credentials'` is empty
2. Confirm no account IDs / keys in tracked files (`./scripts/check-secrets.sh tracked`)
3. Settings → **Change repository visibility** → Public
4. Apply the ruleset (script or GitHub UI)
5. Re-run CI on open PRs

Do **not** put `AWS_ROLE_ARN` or bucket/distribution IDs into source. They stay in the Environment.

## Local clone

```bash
./scripts/install-git-hooks.sh
./scripts/check-secrets.sh all
```
