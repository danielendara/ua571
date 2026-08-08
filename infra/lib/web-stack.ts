import * as cdk from 'aws-cdk-lib';
import * as acm from 'aws-cdk-lib/aws-certificatemanager';
import * as cloudfront from 'aws-cdk-lib/aws-cloudfront';
import * as origins from 'aws-cdk-lib/aws-cloudfront-origins';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as route53 from 'aws-cdk-lib/aws-route53';
import * as targets from 'aws-cdk-lib/aws-route53-targets';
import * as s3 from 'aws-cdk-lib/aws-s3';
import { Construct } from 'constructs';

export class Ua571WebStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    const domainName =
      (this.node.tryGetContext('domainName') as string | undefined) ??
      'ua571.danielendara.com';
    const hostedZoneName =
      (this.node.tryGetContext('hostedZoneName') as string | undefined) ??
      'danielendara.com';
    const githubRepo =
      (this.node.tryGetContext('githubRepo') as string | undefined) ??
      'danielendara/ua571';
    const githubBranch =
      (this.node.tryGetContext('githubBranch') as string | undefined) ?? 'main';

    // ── Hosting bucket (private; CloudFront OAC only) ──
    const siteBucket = new s3.Bucket(this, 'SiteBucket', {
      // Non-guessable name; domain is on CloudFront, not the website endpoint.
      bucketName: undefined,
      blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
      encryption: s3.BucketEncryption.S3_MANAGED,
      enforceSSL: true,
      versioned: true,
      // Keep history; empty on destroy only if you change this deliberately.
      removalPolicy: cdk.RemovalPolicy.RETAIN,
      autoDeleteObjects: false,
    });

    // ── DNS + TLS (us-east-1 required for CloudFront certs) ──
    const zone = route53.HostedZone.fromLookup(this, 'Zone', {
      domainName: hostedZoneName,
    });

    const certificate = new acm.Certificate(this, 'SiteCertificate', {
      domainName,
      validation: acm.CertificateValidation.fromDns(zone),
    });

    // ── Cache policies ──
    // HTML/JS shell: short TTL so deploys show up after invalidation + natural expiry.
    const shellCachePolicy = new cloudfront.CachePolicy(this, 'ShellCachePolicy', {
      cachePolicyName: `ua571-shell-${this.account}`,
      comment: 'HTML/JS entrypoints for ua571 web console',
      defaultTtl: cdk.Duration.minutes(5),
      maxTtl: cdk.Duration.hours(1),
      minTtl: cdk.Duration.seconds(0),
      enableAcceptEncodingBrotli: true,
      enableAcceptEncodingGzip: true,
      headerBehavior: cloudfront.CacheHeaderBehavior.none(),
      queryStringBehavior: cloudfront.CacheQueryStringBehavior.none(),
      cookieBehavior: cloudfront.CacheCookieBehavior.none(),
    });

    // WASM/pkg assets: long TTL (filenames change when you re-run wasm-bindgen with new hashes
    // only if you introduce hashed names later; still invalidated on deploy).
    const assetCachePolicy = new cloudfront.CachePolicy(this, 'AssetCachePolicy', {
      cachePolicyName: `ua571-assets-${this.account}`,
      comment: 'pkg/ WASM and glue for ua571',
      defaultTtl: cdk.Duration.days(7),
      maxTtl: cdk.Duration.days(30),
      minTtl: cdk.Duration.seconds(0),
      enableAcceptEncodingBrotli: true,
      enableAcceptEncodingGzip: true,
      headerBehavior: cloudfront.CacheHeaderBehavior.none(),
      queryStringBehavior: cloudfront.CacheQueryStringBehavior.none(),
      cookieBehavior: cloudfront.CacheCookieBehavior.none(),
    });

    const responseHeaders = new cloudfront.ResponseHeadersPolicy(
      this,
      'SecurityHeaders',
      {
        responseHeadersPolicyName: `ua571-security-${this.account}`,
        securityHeadersBehavior: {
          contentTypeOptions: { override: true },
          frameOptions: {
            frameOption: cloudfront.HeadersFrameOption.DENY,
            override: true,
          },
          referrerPolicy: {
            referrerPolicy:
              cloudfront.HeadersReferrerPolicy.STRICT_ORIGIN_WHEN_CROSS_ORIGIN,
            override: true,
          },
          strictTransportSecurity: {
            accessControlMaxAge: cdk.Duration.days(365),
            includeSubdomains: true,
            preload: false,
            override: true,
          },
          xssProtection: {
            protection: true,
            modeBlock: true,
            override: true,
          },
        },
        // Allow wasm + modules; tighten further if you add a full CSP later.
        customHeadersBehavior: {
          customHeaders: [
            {
              header: 'Cross-Origin-Opener-Policy',
              value: 'same-origin',
              override: true,
            },
          ],
        },
      },
    );

    const s3Origin = origins.S3BucketOrigin.withOriginAccessControl(siteBucket);

    const distribution = new cloudfront.Distribution(this, 'Distribution', {
      comment: `ua571 web console (${domainName})`,
      domainNames: [domainName],
      certificate,
      defaultRootObject: 'index.html',
      minimumProtocolVersion: cloudfront.SecurityPolicyProtocol.TLS_V1_2_2021,
      httpVersion: cloudfront.HttpVersion.HTTP2_AND_3,
      priceClass: cloudfront.PriceClass.PRICE_CLASS_100,
      enableLogging: false,
      defaultBehavior: {
        origin: s3Origin,
        viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
        allowedMethods: cloudfront.AllowedMethods.ALLOW_GET_HEAD_OPTIONS,
        cachedMethods: cloudfront.CachedMethods.CACHE_GET_HEAD_OPTIONS,
        compress: true,
        cachePolicy: shellCachePolicy,
        responseHeadersPolicy: responseHeaders,
      },
      additionalBehaviors: {
        'pkg/*': {
          origin: s3Origin,
          viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
          allowedMethods: cloudfront.AllowedMethods.ALLOW_GET_HEAD_OPTIONS,
          cachedMethods: cloudfront.CachedMethods.CACHE_GET_HEAD_OPTIONS,
          compress: true,
          cachePolicy: assetCachePolicy,
          responseHeadersPolicy: responseHeaders,
        },
      },
      // Single-page app: missing keys (deep links) → index.html for future routes.
      errorResponses: [
        {
          httpStatus: 403,
          responseHttpStatus: 200,
          responsePagePath: '/index.html',
          ttl: cdk.Duration.minutes(1),
        },
        {
          httpStatus: 404,
          responseHttpStatus: 200,
          responsePagePath: '/index.html',
          ttl: cdk.Duration.minutes(1),
        },
      ],
    });

    // ── Route53 alias ──
    const recordName = domainName.replace(`.${hostedZoneName}`, '');
    new route53.ARecord(this, 'AliasA', {
      zone,
      recordName,
      target: route53.RecordTarget.fromAlias(
        new targets.CloudFrontTarget(distribution),
      ),
    });
    new route53.AaaaRecord(this, 'AliasAAAA', {
      zone,
      recordName,
      target: route53.RecordTarget.fromAlias(
        new targets.CloudFrontTarget(distribution),
      ),
    });

    // ── GitHub Actions OIDC deploy role (open-source safe: no long-lived keys) ──
    // Reuse account OIDC provider if present:
    //   -c githubOidcProviderArn=arn:aws:iam::ACCOUNT:oidc-provider/token.actions.githubusercontent.com
    // Otherwise create one (only one per account is allowed).
    const existingProviderArn = this.node.tryGetContext(
      'githubOidcProviderArn',
    ) as string | undefined;

    const oidcProvider = existingProviderArn
      ? iam.OpenIdConnectProvider.fromOpenIdConnectProviderArn(
          this,
          'GitHubOidc',
          existingProviderArn,
        )
      : new iam.OpenIdConnectProvider(this, 'GitHubOidc', {
          url: 'https://token.actions.githubusercontent.com',
          clientIds: ['sts.amazonaws.com'],
        });

    // Trust both branch ref and GitHub Environment subjects.
    // Jobs that use `environment: production` present sub as
    //   repo:ORG/REPO:environment:production
    // not only ref:refs/heads/main.
    const oidcAudience = {
      StringEquals: {
        'token.actions.githubusercontent.com:aud': 'sts.amazonaws.com',
      },
    };
    const deployRole = new iam.Role(this, 'GitHubDeployRole', {
      roleName: 'ua571-github-deploy',
      description:
        'GitHub Actions deploy role for ua571 web (OIDC; main / production env only)',
      assumedBy: new iam.CompositePrincipal(
        new iam.FederatedPrincipal(
          oidcProvider.openIdConnectProviderArn,
          {
            ...oidcAudience,
            StringLike: {
              'token.actions.githubusercontent.com:sub': `repo:${githubRepo}:ref:refs/heads/${githubBranch}`,
            },
          },
          'sts:AssumeRoleWithWebIdentity',
        ),
        new iam.FederatedPrincipal(
          oidcProvider.openIdConnectProviderArn,
          {
            ...oidcAudience,
            StringLike: {
              'token.actions.githubusercontent.com:sub': `repo:${githubRepo}:environment:production`,
            },
          },
          'sts:AssumeRoleWithWebIdentity',
        ),
      ),
      maxSessionDuration: cdk.Duration.hours(1),
    });

    siteBucket.grantReadWrite(deployRole);
    siteBucket.grantDelete(deployRole);

    deployRole.addToPolicy(
      new iam.PolicyStatement({
        sid: 'CloudFrontInvalidate',
        actions: ['cloudfront:CreateInvalidation', 'cloudfront:GetInvalidation'],
        resources: [
          `arn:aws:cloudfront::${this.account}:distribution/${distribution.distributionId}`,
        ],
      }),
    );

    // ── Outputs for GitHub Actions variables ──
    new cdk.CfnOutput(this, 'SiteUrl', {
      value: `https://${domainName}`,
      description: 'Public console URL',
    });
    new cdk.CfnOutput(this, 'BucketName', {
      value: siteBucket.bucketName,
      description: 'Set GitHub variable UA571_S3_BUCKET',
    });
    new cdk.CfnOutput(this, 'DistributionId', {
      value: distribution.distributionId,
      description: 'Set GitHub variable UA571_CLOUDFRONT_DISTRIBUTION_ID',
    });
    new cdk.CfnOutput(this, 'DeployRoleArn', {
      value: deployRole.roleArn,
      description: 'Set GitHub variable AWS_ROLE_ARN (OIDC)',
    });
    new cdk.CfnOutput(this, 'DistributionDomainName', {
      value: distribution.distributionDomainName,
    });
  }
}
