import * as assert from 'node:assert/strict';
import { test } from 'node:test';
import * as cdk from 'aws-cdk-lib';
import { Template } from 'aws-cdk-lib/assertions';
import { Ua571WebStack } from '../lib/web-stack';

const EXPECTED_CSP =
  "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; " +
  "img-src 'self'; connect-src 'self'; manifest-src 'self'; object-src 'none'; " +
  "base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

function synth(): Template {
  // Placeholder values only; never real account or zone IDs.
  const env = { account: '111111111111', region: 'us-east-1' };
  const key = cdk.ContextProvider.getKey(new cdk.Stack(new cdk.App(), 'Probe', { env }), {
    provider: 'hostedZone' as never,
    props: { domainName: 'danielendara.com', privateZone: false },
  }).key;
  const app = new cdk.App();
  app.node.setContext(key, { Id: '/hostedzone/ZONEID', Name: 'danielendara.com.' });
  return Template.fromStack(new Ua571WebStack(app, 'TestStack', { env }));
}

test('security headers policy sets the strict CSP and no X-XSS-Protection', () => {
  const template = synth();
  const policies = template.findResources('AWS::CloudFront::ResponseHeadersPolicy');
  const configs = Object.values(policies).map(
    (r) => (r.Properties as any).ResponseHeadersPolicyConfig,
  );
  assert.equal(configs.length, 1);
  const sec = configs[0].SecurityHeadersConfig;
  assert.deepEqual(sec.ContentSecurityPolicy, {
    ContentSecurityPolicy: EXPECTED_CSP,
    Override: true,
  });
  assert.equal(sec.XSSProtection, undefined);
});

test('deploy role trusts only the production environment of this repository', () => {
  const template = synth();
  const roles = Object.values(template.findResources('AWS::IAM::Role')).filter(
    (r) => (r.Properties as any).RoleName === 'ua571-github-deploy',
  );
  assert.equal(roles.length, 1);
  const statements = (roles[0].Properties as any).AssumeRolePolicyDocument.Statement;
  assert.equal(statements.length, 1);
  const condition = statements[0].Condition;

  // No wildcard matching anywhere in the trust policy.
  assert.equal(condition.StringLike, undefined);
  assert.equal(condition['ForAnyValue:StringLike'], undefined);

  assert.deepEqual(condition.StringEquals, {
    'token.actions.githubusercontent.com:aud': 'sts.amazonaws.com',
    'token.actions.githubusercontent.com:sub': [
      'repo:danielendara/ua571:environment:production',
      'repo:danielendara@293949/ua571@1315671999:environment:production',
    ],
  });
});
