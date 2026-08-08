# ua571 infrastructure

AWS CDK (TypeScript) stack for **https://ua571.danielendara.com**.

Creates:

- Private S3 bucket + CloudFront (OAC)
- ACM certificate + Route53 alias in `danielendara.com`
- IAM role for GitHub Actions OIDC deploys

See [docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md) for full setup.

```bash
npm install
npx cdk synth
npx cdk deploy   # region forced to us-east-1 in bin/app.ts
```
