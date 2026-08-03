#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const SLOW_LABEL = "slow";
const REGULAR_LABELS = ["1", "2", "3"];
const ARCHIVE_BUILDERS = new Map([
  [SLOW_LABEL, "slow"],
  ["1", "a"],
  ["2", "slow"],
  ["3", "b"],
]);

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      fail("usage: plan_nextest_target_archives.mjs --input FILE --output-dir DIR --package NAME --slow-test-target NAME");
    }
    values.set(key, value);
  }
  return values;
}

function sourceBytes(sourcePath) {
  try {
    return fs.statSync(sourcePath).size;
  } catch (error) {
    fail(`cannot stat Cargo test target source ${sourcePath}: ${error.message}`);
  }
}

function targetSelector(target) {
  if (target.kind.includes("test")) {
    return { identity: `test:${target.name}`, args: ["--test", target.name] };
  }
  if (target.kind.includes("rlib")) {
    return { identity: `lib:${target.name}`, args: ["--lib"] };
  }
  if (target.kind.includes("bin")) {
    return { identity: `bin:${target.name}`, args: ["--bin", target.name] };
  }
  fail(`unsupported test-enabled Cargo target ${target.name} (${target.kind.join(",")})`);
}

function splitCapacity(total, groupCount) {
  const base = Math.floor(total / groupCount);
  const remainder = total % groupCount;
  return Array.from({ length: groupCount }, (_, index) => base + (index < remainder ? 1 : 0));
}

function selectLeastLoaded(groups) {
  return groups.reduce((best, group) => (
    group.sourceBytes < best.sourceBytes
      || (group.sourceBytes === best.sourceBytes && group.label < best.label)
      ? group
      : best
  ));
}

function assignTargets(targets, groups) {
  for (const target of targets) {
    const available = groups.filter((group) => group.targets.length < group.capacity);
    if (available.length === 0) {
      fail("target assignment exhausted every group capacity");
    }
    const group = selectLeastLoaded(available);
    group.targets.push(target);
    group.sourceBytes += target.sourceBytes;
  }
}

const args = parseArgs(process.argv.slice(2));
const inputPath = args.get("--input");
const outputDir = args.get("--output-dir");
const packageName = args.get("--package");
const slowTestTarget = args.get("--slow-test-target");

if (!inputPath || !outputDir || !packageName || !slowTestTarget) {
  fail("all arguments are required");
}
if (path.resolve(outputDir) === path.parse(path.resolve(outputDir)).root) {
  fail("refusing to use the filesystem root as --output-dir");
}

const metadata = JSON.parse(fs.readFileSync(inputPath, "utf8"));
const packages = (metadata.packages ?? []).filter((candidate) => candidate.name === packageName);
if (packages.length !== 1) {
  fail(`expected exactly one Cargo package named ${packageName}, found ${packages.length}`);
}

const targetIdentities = new Set();
const candidates = packages[0].targets
  .filter((target) => target.test === true)
  .map((target) => {
    const selector = targetSelector(target);
    if (targetIdentities.has(selector.identity)) {
      fail(`duplicate Cargo test target selector ${selector.identity}`);
    }
    targetIdentities.add(selector.identity);
    return {
      ...selector,
      name: target.name,
      kind: target.kind,
      sourceBytes: sourceBytes(target.src_path),
    };
  });

const slowTargets = candidates.filter((target) => target.identity === `test:${slowTestTarget}`);
if (slowTargets.length !== 1) {
  fail(`expected exactly one integration test target ${slowTestTarget}, found ${slowTargets.length}`);
}
const regularTargets = candidates.filter((target) => target !== slowTargets[0]);
if (regularTargets.length < REGULAR_LABELS.length) {
  fail(`need at least ${REGULAR_LABELS.length} regular targets, found ${regularTargets.length}`);
}

// regular target을 세 archive에 직접 배정한다. builder group을 먼저 나누면 A가 두 archive를 만들어
// compile 임계 경로가 길어진다. source 크기 큰 target부터 archive별 least-loaded group에 배정해 세
// builder의 regular compile load를 함께 균형화한다.
regularTargets.sort((left, right) => (
  right.sourceBytes - left.sourceBytes || left.identity.localeCompare(right.identity)
));
const regularCapacities = splitCapacity(regularTargets.length, REGULAR_LABELS.length);
const regularGroups = REGULAR_LABELS.map((label, index) => ({
  label,
  builder: ARCHIVE_BUILDERS.get(label),
  capacity: regularCapacities[index],
  targets: [],
  sourceBytes: 0,
}));
assignTargets(regularTargets, regularGroups);

const archives = new Map();
archives.set(SLOW_LABEL, {
  label: SLOW_LABEL,
  builder: ARCHIVE_BUILDERS.get(SLOW_LABEL),
  capacity: 1,
  targets: slowTargets,
  sourceBytes: slowTargets[0].sourceBytes,
});
for (const group of regularGroups) {
  if (group.targets.length === 0) {
    fail(`archive ${group.label} has no Cargo test target`);
  }
  archives.set(group.label, group);
}

const orderedLabels = [SLOW_LABEL, ...REGULAR_LABELS];
if (archives.size !== orderedLabels.length || orderedLabels.some((label) => !archives.has(label))) {
  fail("archive labels are incomplete");
}
const assignedIdentities = orderedLabels.flatMap((label) => archives.get(label).targets.map((target) => target.identity));
if (new Set(assignedIdentities).size !== candidates.length || assignedIdentities.length !== candidates.length) {
  fail("Cargo test target assignment contains a duplicate or omission");
}

fs.rmSync(outputDir, { recursive: true, force: true });
fs.mkdirSync(outputDir, { recursive: true });
const plan = {
  package: packageName,
  total_test_targets: candidates.length,
  builders: Object.fromEntries(["slow", "a", "b"].map((builder) => {
    const ownedArchives = orderedLabels
      .map((label) => archives.get(label))
      .filter((archive) => archive.builder === builder);
    return [builder, {
      archive_labels: ownedArchives.map((archive) => archive.label),
      total_target_count: ownedArchives.reduce((sum, archive) => sum + archive.targets.length, 0),
      total_source_bytes: ownedArchives.reduce((sum, archive) => sum + archive.sourceBytes, 0),
    }];
  })),
  archives: Object.fromEntries(orderedLabels.map((label) => {
    const archive = archives.get(label);
    fs.writeFileSync(path.join(outputDir, `${label}.args`), `${archive.targets.flatMap((target) => target.args).join("\n")}\n`);
    return [label, {
      builder: archive.builder,
      target_count: archive.targets.length,
      source_bytes: archive.sourceBytes,
      targets: archive.targets.map(({ identity, name, kind, sourceBytes }) => ({
        identity,
        name,
        kind,
        source_bytes: sourceBytes,
      })),
    }];
  })),
};
fs.writeFileSync(path.join(outputDir, "assignment.json"), `${JSON.stringify(plan, null, 2)}\n`);
console.log(JSON.stringify(plan, null, 2));
