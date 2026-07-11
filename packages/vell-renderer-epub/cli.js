#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Samin Yeasar

/**
 * CLI wrapper for the Vell EPUB 3 renderer.
 *
 * Usage:
 *   node cli.js <input-ast.json> [output.epub]
 *   cat input-ast.json | node cli.js > output.epub
 *
 * The input JSON should be a serialized VellDocument AST matching
 * the VellDocument interface in index.ts.
 */

import { readFileSync } from "fs";
import { writeFileSync } from "fs";
import { renderEpub } from "./dist/index.js";

const args = process.argv.slice(2);

async function main() {
  let inputData;
  let outputPath;

  if (args.length >= 1 && args[0] !== "-") {
    // Read from file
    const inputPath = args[0];
    inputData = readFileSync(inputPath, "utf-8");
    outputPath = args[1];
  } else {
    // Read from stdin
    const chunks = [];
    for await (const chunk of process.stdin) {
      chunks.push(chunk);
    }
    inputData = Buffer.concat(chunks).toString("utf-8");
    outputPath = args[0] === "-" ? args[1] : args[0];
  }

  let doc;
  try {
    doc = JSON.parse(inputData);
  } catch {
    console.error("Error: Invalid JSON input — could not parse AST.");
    process.exit(1);
  }

  try {
    const epubBuffer = await renderEpub(doc);

    if (outputPath) {
      writeFileSync(outputPath, epubBuffer);
      console.error(`Wrote EPUB to ${outputPath} (${epubBuffer.length} bytes)`);
    } else {
      // Write to stdout as raw bytes
      process.stdout.write(Buffer.from(epubBuffer));
    }
  } catch (err) {
    console.error("Error rendering EPUB:", err.message);
    process.exit(1);
  }
}

main();
