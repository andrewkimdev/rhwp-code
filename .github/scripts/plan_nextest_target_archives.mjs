#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const SLOW_LABEL = "slow";
const REGULAR_LABELS = ["1", "2", "3"];
const BUILDER_A_LABELS = ["1", "2"];
const BUILDER_B_LABELS = ["3"];

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

// worker 실행량은 archive별 target 수가 비슷해야 하므로 먼저 세 regular archive의 capacity를 정한다.
// A는 두 archive, B는 한 archive를 빌드한다. slow archive는 전용 builder가 만들므로 regular compile
// 경로와 분리되고, source 크기 큰 target부터 least-loaded builder에 배정한다.
regularTargets.sort((left, right) => (
  right.sourceBytes - left.sourceBytes || left.identity.localeCompare(right.identity)
));
const regularArchiveCapacities = new Map(
  REGULAR_LABELS.map((label, index) => [label, splitCapacity(regularTargets.length, REGULAR_LABELS.length)[index]]),
);
const regularBuilderACount = BUILDER_A_LABELS.reduce(
  (sum, label) => sum + regularArchiveCapacities.get(label),
  0,
);
const builderGroups = [
  { label: "a", capacity: regularBuilderACount, targets: [], sourceBytes: 0 },
  { label: "b", capacity: regularTargets.length - regularBuilderACount, targets: [], sourceBytes: 0 },
];
assignTargets(regularTargets, builderGroups);
if (builderGroups.some((builder) => builder.targets.length !== builder.capacity)) {
  fail("builder regular target assignment does not match archive capacities");
}

const archives = new Map();
archives.set(SLOW_LABEL, {
  label: SLOW_LABEL,
  builder: "slow",
  capacity: 1,
  targets: slowTargets,
  sourceBytes: slowTargets[0].sourceBytes,
});
for (const builder of builderGroups) {
  const labels = builder.label === "a" ? BUILDER_A_LABELS : BUILDER_B_LABELS;
  const groups = labels.map((label) => ({
    label,
    builder: builder.label,
    capacity: regularArchiveCapacities.get(label),
    targets: [],
    sourceBytes: 0,
  }));
  assignTargets(builder.targets, groups);
  for (const group of groups) {
    if (group.targets.length === 0) {
      fail(`archive ${group.label} has no Cargo test target`);
    }
    archives.set(group.label, group);
  }
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
  builders: Object.fromEntries(builderGroups.map((builder) => [builder.label, {
    regular_target_count: builder.targets.length,
    regular_source_bytes: builder.sourceBytes,
  }])),
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
