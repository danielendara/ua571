#!/usr/bin/env node
import * as cdk from 'aws-cdk-lib';
import { Ua571WebStack } from '../lib/web-stack';

const app = new cdk.App();

/**
 * CloudFront ACM certificates must live in us-east-1.
 * Deploy this stack to us-east-1 even if other apps use other regions.
 */
const env = {
  account: process.env.CDK_DEFAULT_ACCOUNT,
  region: 'us-east-1',
};

new Ua571WebStack(app, 'Ua571WebStack', {
  env,
  description: 'ua571.danielendara.com — static WASM console (S3 + CloudFront + Route53)',
  tags: {
    Project: 'ua571',
    ManagedBy: 'cdk',
  },
});

app.synth();
