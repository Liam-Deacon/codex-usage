import assert from 'node:assert/strict'
import { mkdtemp, mkdir, readFile, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import test from 'node:test'

import {
  collectGeneratedPackages,
  validateLocalOptionalDeps,
} from '../scripts/verify-npm-optional-deps.mjs'

test('package.json exposes codex-usage-cli bin alias', async () => {
  const packageJsonPath = new URL('../package.json', import.meta.url)
  const pkg = JSON.parse(await readFile(packageJsonPath, 'utf8'))
  assert.equal(pkg.bin['codex-usage-cli'], 'cli.mjs')
})

test('validateLocalOptionalDeps passes when generated packages match optionalDependencies', () => {
  const generated = new Map([
    ['codex-usage-cli-darwin-arm64', { version: '1.2.3', hasBinary: true }],
    ['codex-usage-cli-darwin-x64', { version: '1.2.3', hasBinary: true }],
  ])

  const result = validateLocalOptionalDeps(
    {
      'codex-usage-cli-darwin-arm64': '1.2.3',
      'codex-usage-cli-darwin-x64': '1.2.3',
    },
    generated,
  )

  assert.equal(result.ok, true)
  assert.deepEqual(result.missing, [])
  assert.deepEqual(result.extra, [])
  assert.deepEqual(result.versionMismatches, [])
  assert.deepEqual(result.missingBinaries, [])
})

test('validateLocalOptionalDeps reports missing package, version mismatch, and missing binary', () => {
  const generated = new Map([
    ['codex-usage-cli-darwin-arm64', { version: '1.2.2', hasBinary: false }],
    ['codex-usage-cli-linux-x64-gnu', { version: '1.2.3', hasBinary: true }],
  ])

  const result = validateLocalOptionalDeps(
    {
      'codex-usage-cli-darwin-arm64': '1.2.3',
      'codex-usage-cli-win32-x64-msvc': '1.2.3',
    },
    generated,
  )

  assert.equal(result.ok, false)
  assert.deepEqual(result.missing, ['codex-usage-cli-win32-x64-msvc'])
  assert.deepEqual(result.extra, ['codex-usage-cli-linux-x64-gnu'])
  assert.deepEqual(result.versionMismatches, [
    {
      name: 'codex-usage-cli-darwin-arm64',
      expected: '1.2.3',
      actual: '1.2.2',
    },
  ])
  assert.deepEqual(result.missingBinaries, ['codex-usage-cli-darwin-arm64'])
})

test('collectGeneratedPackages reads npm sub-packages and detects binary presence', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'codex-usage-'))
  const arm64Dir = path.join(root, 'darwin-arm64')
  const x64Dir = path.join(root, 'darwin-x64')

  await mkdir(arm64Dir, { recursive: true })
  await mkdir(x64Dir, { recursive: true })

  await writeFile(
    path.join(arm64Dir, 'package.json'),
    JSON.stringify({ name: 'codex-usage-cli-darwin-arm64', version: '9.9.9' }),
  )
  await writeFile(path.join(arm64Dir, 'codex-usage-cli.darwin-arm64.node'), '')

  await writeFile(
    path.join(x64Dir, 'package.json'),
    JSON.stringify({ name: 'codex-usage-cli-darwin-x64', version: '9.9.9' }),
  )

  const packages = await collectGeneratedPackages(root)

  assert.equal(packages.size, 2)
  assert.equal(packages.get('codex-usage-cli-darwin-arm64').hasBinary, true)
  assert.equal(packages.get('codex-usage-cli-darwin-x64').hasBinary, false)
})
