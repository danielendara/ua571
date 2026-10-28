# ua571 infrastructure

AWS CDK (TypeScript) stack for **https://ua571.danielendara.com**.

Creates:

- Private S3 bucket + CloudFront (OAC)
- CloudFront response headers policy: Content-Security-Policy, HSTS, `X-Frame-Options`, Referrer-Policy (see `CONTENT_SECURITY_POLICY` in `lib/web-stack.ts`)
- ACM certificate + Route53 alias in `danielendara.com`
- IAM role for GitHub Actions OIDC deploys

See [docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md) for full setup.

```bash
npm install
npx cdk synth
npx cdk deploy   # region forced to us-east-1 in bin/app.ts
```
