# ua571 recovery runbook

This is the recovery runbook for ua571: how to roll back a bad web deploy and confirm the site is healthy. Last verified 2026-09-27 (read-only). Anything not seen directly is marked **UNVERIFIED**. All RTOs are **estimates**.

---

## ua571: Rust TUI + WASM web console (reference template)

- **Hosting:** S3 (**versioned: true**, RETAIN, non-guessable name) + CloudFront + Route 53 at `ua571.danielendara.com`. Verified in `infra/lib/web-stack.ts`.
- **Deploy path:** GitHub Actions "Deploy web" on push to `main` (path-filtered) or `workflow_dispatch`. It uses OIDC role `ua571-github-deploy` (no long-lived keys) and GitHub Environment `production` (no required reviewers at launch). It runs `aws s3 sync` (`pkg/` with `--delete`) and invalidates `/*`, with Cache-Control `max-age=60`. Recent runs take **about 30-40 s**.
- **Rollback, option 1 (RTO est. 1-3 min):** `git revert <BAD_SHA>` on `main` through a PR, and the push triggers the deploy. `workflow_dispatch` **can't deploy an old SHA**: the job only runs when `github.ref == refs/heads/main`, so dispatch always ships current `main`.
```bash
gh workflow run deploy-web.yml -R danielendara/ua571 --ref main   # re-deploy main after the revert lands
gh run watch -R danielendara/ua571
```
Docs: https://cli.github.com/manual/gh_workflow_run
- **Rollback, option 2 (RTO est. 3-5 min, AWS creds):** restore the previous object versions of `index.html` and `pkg/*` (the `--delete` left delete markers, so remove those), then invalidate `/*`. Bucket name is in `gh variable list -R danielendara/ua571 --env production`. Docs: [S3 restore](https://docs.aws.amazon.com/AmazonS3/latest/userguide/RestoringPreviousVersions.html).
- **DB:** none. **Backups:** S3 versions plus git.
- **Health check:** `curl -sI https://ua571.danielendara.com/ | head -1` → `200`. `curl -s https://ua571.danielendara.com/build-id.js` shows the build ID. `curl -sI https://ua571.danielendara.com/pkg/ua571_web_bg.wasm | grep -i content-type` → `application/wasm`.

### Known gaps (est.)

| Current RTO | Gap | Fix |
|---|---|---|
| 1-3 min (revert → Actions) | Can't dispatch an old SHA. | Add a `ref` input to `workflow_dispatch` (restricted to ancestors of main), or document the S3-version rollback |
