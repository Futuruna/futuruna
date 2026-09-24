// Focused setup checks; no network, compiler build, install, or private data.
// Run with: node --test tests/tax_audit_setup.test.mjs
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const guide = readFileSync(join(root, 'website/public/ai-setup.md'), 'utf8');
const blocks = [...guide.matchAll(/```sh\n([\s\S]*?)\n```/g)];
const downloads = blocks.filter((match) => match[1].includes('BINARY=DOWNLOAD_NAME'));
assert.equal(downloads.length, 1, 'exactly one maintained download snippet');
const snippet = downloads[0][1]
  .replace('BINARY=DOWNLOAD_NAME', 'BINARY=runa-macos-arm64')
  .replace('RELEASE_TAG=RELEASE_TAG', 'RELEASE_TAG=v0.2.0');
const artifact = 'synthetic download fixture; never executed\n';
const digest = createHash('sha256').update(artifact).digest('hex');

test('download snippet fails closed and preserves an existing compiler', () => {
  const evidence = mkdtempSync(join(tmpdir(), 'futuruna-setup-test-'));
  console.log(`Synthetic setup evidence: ${evidence}`);
  const mockbin = join(evidence, 'mockbin');
  mkdirSync(mockbin);
  // Only curl is replaced. The guide runs the real system checksum checker.
  writeFileSync(join(mockbin, 'curl'), `#!${process.execPath}
const { writeFileSync } = require('node:fs');
const args = process.argv.slice(2);
const output = args[args.indexOf('--output') + 1];
const url = args.at(-1);
if (!url.startsWith('https://github.com/Futuruna/futuruna/releases/download/v0.2.0/')) process.exit(90);
const mode = process.env.FUTURUNA_SETUP_TEST_MODE;
if (mode === 'download-failure') process.exit(22);
let content = ${JSON.stringify(artifact)};
if (url.endsWith('/SHA256SUMS')) {
  const row = ${JSON.stringify(`${digest}  runa-macos-arm64\n`)};
  content = row;
  if (mode === 'wrong-checksum') content = '0'.repeat(64) + '  runa-macos-arm64\\n';
  if (mode === 'missing-entry') content = row.replace('runa-macos-arm64', 'runa-linux-arm64');
  if (mode === 'duplicate-entry') content = row + row;
  if (mode === 'manifest-failure') process.exit(22);
}
writeFileSync(output, content, { mode: 0o600, flag: 'wx' });
`, { mode: 0o700 });

  for (const mode of ['success', 'download-failure', 'manifest-failure', 'wrong-checksum', 'missing-entry', 'duplicate-entry']) {
    const cwd = join(evidence, mode);
    mkdirSync(join(cwd, 'target/release'), { recursive: true });
    const existing = join(cwd, 'target/release/runa');
    writeFileSync(existing, 'existing working compiler\n', { mode: 0o700 });
    const result = spawnSync('/bin/sh', ['-c', snippet], {
      cwd, encoding: 'utf8',
      env: { ...process.env, PATH: `${mockbin}:${process.env.PATH}`, FUTURUNA_SETUP_TEST_MODE: mode },
    });
    assert.ifError(result.error);
    assert.equal(readFileSync(existing, 'utf8'), 'existing working compiler\n', mode);
    const staging = readdirSync(join(cwd, 'target')).filter((name) => name.startsWith('runa-download.'));
    assert.equal(staging.length, 1, mode);
    const downloaded = join(cwd, 'target', staging[0], 'runa-macos-arm64');
    if (mode === 'success') {
      assert.equal(result.status, 0, result.stderr);
      const reported = result.stdout.match(/^Verified binary: (.+)$/m);
      assert.ok(reported, result.stdout);
      assert.equal(realpathSync(reported[1]), realpathSync(downloaded));
      assert.ok(statSync(downloaded).mode & 0o100, 'verified artifact executable');
    } else {
      assert.notEqual(result.status, 0, mode);
      assert.ok(!result.stdout.includes('Verified binary:'), mode);
      if (existsSync(downloaded)) assert.equal(statSync(downloaded).mode & 0o111, 0, mode);
    }
  }
});

test('runtime preflight never builds when the selected executable is missing', () => {
  const result = spawnSync('bash', [join(root, 'scripts/tax-audit-preflight.sh')], {
    cwd: root, encoding: 'utf8',
    env: { ...process.env, RUNA_BIN: '/nonexistent/futuruna-setup-test/runa' },
  });
  assert.ifError(result.error);
  assert.equal(result.status, 1);
  assert.ok(result.stderr.includes('No build was started.'));
});

test('release workflow checks its staged artifact, not a replacement build', () => {
  const workflow = readFileSync(join(root, '.github/workflows/release.yml'), 'utf8');
  assert.ok(workflow.includes('RUNA_BIN="$binary" bash scripts/tax-audit-preflight.sh'));
});

test('ordinary tax guides keep compiler commands on the checked binary', () => {
  for (const name of ['pension-og-fradrag.md', 'aarsopgoerelse-afstemning.md', 'boligjob.md']) {
    const text = readFileSync(join(root, 'examples/danish-income-tax', name), 'utf8');
    assert.ok(text.includes('tax-audit-runtime-check'), `${name}: link the runtime check`);
    const shell = [...text.matchAll(/```sh\n([\s\S]*?)\n```/g)].map(match => match[1]).join('\n');
    assert.ok(shell.includes('"$RUNA_BIN"'), `${name}: use the checked compiler`);
    assert.doesNotMatch(shell, /^\s*runa\s+(?:template|call|check|run)\s/m,
      `${name}: do not silently switch to a PATH compiler`);
  }
});
