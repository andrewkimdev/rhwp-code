#!/usr/bin/env node

import fs from "node:fs";

function fail(message) {
  throw new Error(message);
}

const [flag, inputPath] = process.argv.slice(2);
if (flag !== "--input" || !inputPath || process.argv.length !== 4) {
  fail("usage: count_nextest_runnable.mjs --input FILE");
}

const testList = JSON.parse(fs.readFileSync(inputPath, "utf8"));
const suites = Object.values(testList["rust-suites"] ?? {});
if (suites.length === 0) {
  fail("nextest test list does not contain rust-suites");
}

const runnable = suites.reduce((sum, suite) => (
  sum + Object.values(suite.testcases ?? {}).filter((testcase) => testcase?.ignored !== true).length
), 0);
console.log(runnable);
