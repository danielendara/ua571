# Deploying ua571.danielendara.com

This project is designed so the **source can be public** while production hosting stays in the maintainer’s AWS account. Contributors never need AWS credentials.

## Architecture

```
GitHub Actions (main only, canonical repo)
        │  OIDC → IAM role ua571-github-deploy
        ▼
     S3 (private)  ←── OAC ──→  CloudFront
                                   │
                    ACM (us-east-1) + Route53
                                   │
                      https://ua571.danielendara.com
```

| Piece | Purpose |
|-------|---------|
| `infra/` | AWS CDK stack: bucket, cert, distribution, DNS, deploy role |
| `.github/workflows/deploy-web.yml` | Build WASM + sync + invalidate |
| `web/` | Static host + `pkg/` (build artifact, not committed) |

## Open-source safety model

| Actor | Can deploy to production? |
|-------|---------------------------|
| Maintainer push to `main` on `danielendara/ua571` | Yes (OIDC role) |
| Pull requests / forks | **No** — workflow `if:` blocks; OIDC `sub` won’t match |
| Random clone / self-host | Build locally; host anywhere static |

- **No long-lived AWS access keys** in the repository.
- Deploy uses **GitHub OIDC** + IAM role trust limited to:
  - `repo:danielendara/ua571:ref:refs/heads/main`
  - `repo:danielendara/ua571:environment:production`  
    (required when the workflow uses `environment: production`)
- GitHub **Environment** `production` holds variables (and optional required reviewers).

## One-time AWS setup (maintainer)

Prerequisites: AWS CLI + CDK bootstrap in **us-east-1**, Route53 zone for `danielendara.com`.

### 1. GitHub OIDC provider (once per account)

If you already use GitHub Actions → AWS OIDC (common), reuse it:

```bash
# Find existing provider ARN
aws iam list-open-id-connect-providers
```

If none exists, the CDK stack can create one. If create fails with “already exists”, redeploy with:

```bash
cd infra
npx cdk deploy -c githubOidcProviderArn=arn:aws:iam::ACCOUNT:oidc-provider/token.actions.githubusercontent.com
```

### 2. Deploy infrastructure

```bash
cd infra
npm install
npx cdk bootstrap aws://ACCOUNT/us-east-1   # if not already
npx cdk deploy
```

Stack outputs:

| Output | GitHub variable |
|--------|-----------------|
| `DeployRoleArn` | `AWS_ROLE_ARN` |
| `BucketName` | `UA571_S3_BUCKET` |
| `DistributionId` | `UA571_CLOUDFRONT_DISTRIBUTION_ID` |

Site URL: **https://ua571.danielendara.com**

### 3. GitHub configuration

1. Repo → **Settings → Environments → New environment: `production`**
2. Add **Environment variables** (not secrets — these are not confidential identifiers):

   | Name | Value |
   |------|--------|
   | `AWS_ROLE_ARN` | from stack output `DeployRoleArn` |
   | `UA571_S3_BUCKET` | from `BucketName` |
   | `UA571_CLOUDFRONT_DISTRIBUTION_ID` | from `DistributionId` |

3. Optional: enable **required reviewers** on `production` so deploys need approval even on `main`.

Repo/org variables also work if you prefer; the workflow reads `vars.*`.

### 4. First deploy

- **Actions → Deploy web → Run workflow**, or  
- Push a change under `web/` / `crates/ua571-web/` / etc. to `main`.

## Local deploy (emergency)

```bash
./scripts/build-web.sh
aws s3 sync web/ s3://$UA571_S3_BUCKET/ --delete
aws cloudfront create-invalidation \
  --distribution-id $UA571_CLOUDFRONT_DISTRIBUTION_ID \
  --paths "/*"
```

Requires IAM permissions equivalent to the deploy role (your personal admin user is fine).

## Self-hosting (anyone)

No AWS required:

```bash
./scripts/build-web.sh
# Serve the web/ directory over HTTPS (any static host)
python3 -m http.server 8080 --directory web   # local only
```

Suitable hosts: Cloudflare Pages, Netlify, GitHub Pages, nginx, S3+CloudFront, etc.  
Upload the **contents** of `web/` including `web/pkg/`.

## Stack context knobs (`infra/cdk.json`)

| Context key | Default |
|-------------|---------|
| `domainName` | `ua571.danielendara.com` |
| `hostedZoneName` | `danielendara.com` |
| `githubRepo` | `danielendara/ua571` |
| `githubBranch` | `main` |
| `githubOidcProviderArn` | (optional) existing OIDC provider |

## Cost notes

- S3 + CloudFront PriceClass 100, no access logs: typically **cents/month** at demo traffic.
- ACM cert and Route53 alias records: standard Route53 query fees only (zone already exists).

## Making the repo public

No infrastructure change required. When you flip GitHub visibility to public:

1. Confirm Environment `production` variables still set.
2. Confirm OIDC trust still lists `danielendara/ua571` (forks still cannot deploy).
3. Update README demo link if needed (already points at the custom domain).
