#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      fail("usage: allocate_nextest_archive_shards.mjs --input FILE --output-dir DIR --regular-shards N --slow-binary-id ID --slow-test NAME");
    }
    values.set(key, value);
  }
  return values;
}

function escapeMatcher(value) {
  return value
    .replaceAll("\\", "\\\\")
    .replaceAll("\n", "\\n")
    .replaceAll("\r", "\\r")
    .replaceAll("\t", "\\t")
    .replaceAll(")", "\\)")
    .replaceAll(",", "\\,");
}

function binaryFilter(binaryIds) {
  if (binaryIds.length === 0) {
    fail("cannot create an archive filter without test binaries");
  }
  return binaryIds
    .map((binaryId) => `binary_id(=${escapeMatcher(binaryId)})`)
    .join(" | ");
}

function chooseLeastLoadedShard(shards) {
  return shards.reduce((best, shard) => (
    shard.runnableTests < best.runnableTests
      || (shard.runnableTests === best.runnableTests && shard.index < best.index)
      ? shard
      : best
  ));
}

const args = parseArgs(process.argv.slice(2));
const inputPath = args.get("--input");
const outputDir = args.get("--output-dir");
const regularShardCount = Number(args.get("--regular-shards"));
const slowBinaryId = args.get("--slow-binary-id");
const slowTest = args.get("--slow-test");

if (!inputPath || !outputDir || !slowBinaryId || !slowTest || !Number.isInteger(regularShardCount) || regularShardCount < 1) {
  fail("all arguments are required and --regular-shards must be a positive integer");
}
if (path.resolve(outputDir) === path.parse(path.resolve(outputDir)).root) {
  fail("refusing to use the filesystem root as --output-dir");
}

const testList = JSON.parse(fs.readFileSync(inputPath, "utf8"));
const suites = Object.values(testList["rust-suites"] ?? {});
if (suites.length === 0) {
  fail("nextest test list does not contain rust-suites");
}

const regularBinaries = [];
let runnableTests = 0;
let slowMatches = 0;

for (const suite of suites) {
  const binaryId = suite["binary-id"];
  if (typeof binaryId !== "string" || binaryId.length === 0) {
    fail("nextest test list contains a suite without binary-id");
  }

  const testcases = Object.entries(suite.testcases ?? {});
  const runnable = testcases.filter(([, testcase]) => testcase?.ignored !== true);
  runnableTests += runnable.length;

  let regularRunnable = runnable.length;
  if (binaryId === slowBinaryId) {
    slowMatches = runnable.filter(([testName]) => testName === slowTest).length;
    regularRunnable -= slowMatches;
  }
  if (regularRunnable > 0) {
    regularBinaries.push({ binaryId, runnableTests: regularRunnable });
  }
}

if (slowMatches !== 1) {
  fail(`expected exactly one runnable slow test ${slowBinaryId}::${slowTest}, found ${slowMatches}`);
}

regularBinaries.sort((left, right) => (
  right.runnableTests - left.runnableTests || left.binaryId.localeCompare(right.binaryId)
));
const regularShards = Array.from({ length: regularShardCount }, (_, offset) => ({
  index: offset + 1,
  runnableTests: 0,
  binaryIds: [],
}));
for (const binary of regularBinaries) {
  const shard = chooseLeastLoadedShard(regularShards);
  shard.binaryIds.push(binary.binaryId);
  shard.runnableTests += binary.runnableTests;
}

const regularRunnableTests = regularShards.reduce((sum, shard) => sum + shard.runnableTests, 0);
if (regularRunnableTests + 1 !== runnableTests) {
  fail(`shard allocation mismatch: regular=${regularRunnableTests}, slow=1, total=${runnableTests}`);
}
if (regularShards.some((shard) => shard.binaryIds.length === 0)) {
  fail("at least one regular archive shard has no test binaries");
}

fs.rmSync(outputDir, { recursive: true, force: true });
fs.mkdirSync(outputDir, { recursive: true });
fs.writeFileSync(path.join(outputDir, "slow.filter"), `${binaryFilter([slowBinaryId])}\n`);
for (const shard of regularShards) {
  fs.writeFileSync(path.join(outputDir, `${shard.index}.filter`), `${binaryFilter(shard.binaryIds)}\n`);
}

const assignment = {
  runnable_tests: runnableTests,
  slow: {
    binary_id: slowBinaryId,
    test: slowTest,
    runnable_tests: 1,
  },
  regular: regularShards.map((shard) => ({
    index: shard.index,
    runnable_tests: shard.runnableTests,
    binary_ids: shard.binaryIds,
  })),
};
fs.writeFileSync(path.join(outputDir, "assignment.json"), `${JSON.stringify(assignment, null, 2)}\n`);
console.log(JSON.stringify(assignment, null, 2));
