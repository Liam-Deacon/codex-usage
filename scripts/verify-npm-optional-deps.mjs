#!/usr/bin/env node
import { execFile } from 'node:child_process'
import { access, readdir, readFile } from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { promisify } from 'node:util'
import { pathToFileURL } from 'node:url'

const execFileAsync = promisify(execFile)

function toInt(value, fallback) {
  if (value === undefined) {
    return fallback
  }
  const parsed = Number.parseInt(String(value), 10)
  if (Number.isNaN(parsed) || parsed < 0) {
    throw new Error(`Invalid integer value: ${value}`)
  }
  return parsed
}

function parseArgs(argv) {
  if (!argv[0] || (argv[0] !== 'local' && argv[0] !== 'registry')) {
    throw new Error('Usage: verify-npm-optional-deps.mjs <local|registry> [--package-json <path>] [--npm-dir <path>] [--version <x.y.z>] [--retries <n>] [--delay-seconds <n>]')
  }

  const options = {
    mode: argv[0],
    packageJsonPath: 'package.json',
    npmDir: 'npm',
    version: undefined,
    retries: 18,
    delaySeconds: 10,
  }

  for (let i = 1; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--package-json') {
      options.packageJsonPath = argv[++i]
    } else if (arg === '--npm-dir') {
      options.npmDir = argv[++i]
    } else if (arg === '--version') {
      options.version = argv[++i]
    } else if (arg === '--retries') {
      options.retries = toInt(argv[++i], options.retries)
    } else if (arg === '--delay-seconds') {
      options.delaySeconds = toInt(argv[++i], options.delaySeconds)
    } else {
      throw new Error(`Unknown argument: ${arg}`)
    }
  }

  return options
}

async function readJson(filePath) {
  const content = await readFile(filePath, 'utf8')
  return JSON.parse(content)
}

function normalizeVersionOutput(raw) {
  const trimmed = raw.trim()
  if (!trimmed) {
    return ''
  }

  try {
    const parsed = JSON.parse(trimmed)
    if (Array.isArray(parsed)) {
      return String(parsed[0] ?? '')
    }
    return String(parsed ?? '')
  } catch {
    return trimmed.replace(/^"|"$/g, '')
  }
}

async function hasNodeBinary(dir) {
  const files = await readdir(dir)
  return files.some((file) => file.endsWith('.node'))
}

export async function collectGeneratedPackages(npmDir) {
  const packages = new Map()
  let entries = []

  try {
    entries = await readdir(npmDir, { withFileTypes: true })
  } catch (error) {
    if (error.code === 'ENOENT') {
      return packages
    }
    throw error
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue
    }

    const packageDir = path.join(npmDir, entry.name)
    const packageJsonPath = path.join(packageDir, 'package.json')
    try {
      await access(packageJsonPath)
    } catch {
      continue
    }

    const pkg = await readJson(packageJsonPath)
    packages.set(String(pkg.name), {
      name: String(pkg.name),
      version: String(pkg.version),
      dir: packageDir,
      hasBinary: await hasNodeBinary(packageDir),
    })
  }

  return packages
}

export function validateLocalOptionalDeps(optionalDependencies, generatedPackages) {
  const declared = Object.entries(optionalDependencies ?? {}).map(([name, version]) => ({
    name,
    version: String(version),
  }))
  const declaredByName = new Map(declared.map((pkg) => [pkg.name, pkg]))
  const generatedByName = generatedPackages

  const missing = declared
    .filter((pkg) => !generatedByName.has(pkg.name))
    .map((pkg) => pkg.name)
    .sort()

  const extra = [...generatedByName.keys()]
    .filter((name) => !declaredByName.has(name))
    .sort()

  const versionMismatches = []
  const missingBinaries = []

  for (const pkg of declared) {
    const generated = generatedByName.get(pkg.name)
    if (!generated) {
      continue
    }
    if (generated.version !== pkg.version) {
      versionMismatches.push({
        name: pkg.name,
        expected: pkg.version,
        actual: generated.version,
      })
    }
    if (!generated.hasBinary) {
      missingBinaries.push(pkg.name)
    }
  }

  return {
    missing,
    extra,
    versionMismatches,
    missingBinaries,
    ok:
      missing.length === 0 &&
      extra.length === 0 &&
      versionMismatches.length === 0 &&
      missingBinaries.length === 0,
  }
}

function formatLocalValidation(result) {
  const lines = []
  if (result.missing.length > 0) {
    lines.push(`Missing generated packages: ${result.missing.join(', ')}`)
  }
  if (result.extra.length > 0) {
    lines.push(`Unexpected generated packages: ${result.extra.join(', ')}`)
  }
  if (result.versionMismatches.length > 0) {
    const details = result.versionMismatches
      .map(({ name, expected, actual }) => `${name} (expected ${expected}, got ${actual})`)
      .join(', ')
    lines.push(`Version mismatches: ${details}`)
  }
  if (result.missingBinaries.length > 0) {
    lines.push(`Generated package missing native binary: ${result.missingBinaries.join(', ')}`)
  }
  return lines
}

export async function verifyLocalOptionalDeps(packageJsonPath, npmDir) {
  const rootPackage = await readJson(packageJsonPath)
  const generated = await collectGeneratedPackages(npmDir)
  const result = validateLocalOptionalDeps(rootPackage.optionalDependencies, generated)

  if (!result.ok) {
    const details = formatLocalValidation(result)
    throw new Error(`Local npm package validation failed.\n${details.join('\n')}`)
  }
}

async function sleep(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms))
}

export async function verifyRegistryPackageVersion(pkgName, version, retries, delaySeconds) {
  for (let attempt = 1; attempt <= retries; attempt += 1) {
    try {
      const { stdout } = await execFileAsync('npm', ['view', `${pkgName}@${version}`, 'version', '--json'], {
        encoding: 'utf8',
      })
      const resolved = normalizeVersionOutput(stdout)
      if (resolved === version) {
        return
      }
      if (resolved.length > 0) {
        throw new Error(`Resolved unexpected version '${resolved}'`)
      }
      throw new Error('No version returned by npm view')
    } catch (error) {
      if (attempt === retries) {
        const detail =
          error.stderr?.trim() ||
          error.stdout?.trim() ||
          error.message ||
          String(error)
        throw new Error(`Package ${pkgName}@${version} not available after ${retries} attempts: ${detail}`)
      }
      await sleep(delaySeconds * 1000)
    }
  }
}

export async function verifyRegistryOptionalDeps(packageJsonPath, version, retries, delaySeconds) {
  const rootPackage = await readJson(packageJsonPath)
  const packages = [
    rootPackage.name,
    ...Object.keys(rootPackage.optionalDependencies ?? {}),
  ]

  for (const pkgName of packages) {
    process.stdout.write(`Checking npm registry for ${pkgName}@${version}... `)
    await verifyRegistryPackageVersion(pkgName, version, retries, delaySeconds)
    process.stdout.write('ok\n')
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  const versionOverride = options.version

  if (options.mode === 'local') {
    await verifyLocalOptionalDeps(options.packageJsonPath, options.npmDir)
    console.log('Local npm optional dependency validation passed')
    return
  }

  const rootPackage = await readJson(options.packageJsonPath)
  const version = versionOverride ?? String(rootPackage.version)
  await verifyRegistryOptionalDeps(options.packageJsonPath, version, options.retries, options.delaySeconds)
  console.log('Registry npm package validation passed')
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(error.message)
    process.exit(1)
  })
}
