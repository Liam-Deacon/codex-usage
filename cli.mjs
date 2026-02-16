#!/usr/bin/env node
import { runCliNode } from './index.js'
try {
  runCliNode(process.argv.slice(1))
} catch (e) {
  console.error(e.message)
  process.exit(1)
}
