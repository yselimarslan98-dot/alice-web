// Checks web/s3.js against the AWS SigV4 examples from the Amazon S3 API reference
// ("Signature Calculations for the Authorization Header", examples bucket "examplebucket").
import assert from 'node:assert/strict';
import { signRequest, EMPTY_SHA256 } from '../web/s3.js';

const creds = { accessKeyId: 'AKIAIOSFODNN7EXAMPLE', secretAccessKey: 'wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',
  region: 'us-east-1', date: new Date('2013-05-24T00:00:00Z'), payloadHash: EMPTY_SHA256 };
const sig = async (url, headers) => /Signature=(\w+)/.exec((await signRequest({ method: 'GET', url, headers, ...creds })).authorization)[1];

assert.equal(await sig('https://examplebucket.s3.amazonaws.com/test.txt', { Range: 'bytes=0-9' }),
  'f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41');
assert.equal(await sig('https://examplebucket.s3.amazonaws.com/?lifecycle'),
  'fea454ca298b7da1c68078a5d1bdbfbbe0d65c699e0f91ac7a200a0136783543');
assert.equal(await sig('https://examplebucket.s3.amazonaws.com/?max-keys=2&prefix=J'),
  '34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7');
console.log('SigV4: 3/3 AWS examples match');
