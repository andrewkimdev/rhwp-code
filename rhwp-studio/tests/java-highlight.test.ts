import test from 'node:test';
import assert from 'node:assert/strict';

import { highlightJava } from '../src/ui/java-highlight.ts';

test('keywords get the keyword span', () => {
  const out = highlightJava('public record FixFlatData(');
  assert.match(out, /<span class="entity-tok-keyword">public<\/span>/);
  assert.match(out, /<span class="entity-tok-keyword">record<\/span>/);
});

test('capitalized ASCII identifiers get the type span, Korean field names pass through plain', () => {
  const out = highlightJava('        String 신청인_성명,');
  assert.match(out, /<span class="entity-tok-type">String<\/span>/);
  assert.ok(out.includes('신청인_성명'));
  assert.ok(!out.includes('<span class="entity-tok-type">신청인_성명'));
});

test('string literals get the string span', () => {
  const out = highlightJava('RESOURCE_PATH = "/hwpx/fix-flat.hwpx";');
  assert.match(out, /<span class="entity-tok-string">"\/hwpx\/fix-flat\.hwpx"<\/span>/);
});

test('annotations get the annotation span', () => {
  const out = highlightJava('@JsonProperty("신청인_성명(한글)") String 신청인_성명_한글_,');
  assert.match(out, /<span class="entity-tok-annotation">@JsonProperty<\/span>/);
  assert.match(out, /<span class="entity-tok-string">"신청인_성명\(한글\)"<\/span>/);
});

test('block and line comments get the comment span, in full', () => {
  const block = highlightJava('/**\n * hello\n */\npublic class X {}');
  assert.match(block, /<span class="entity-tok-comment">\/\*\*\n \* hello\n \*\/<\/span>/);

  const line = highlightJava('// TODO: fill sampleData()');
  assert.match(line, /<span class="entity-tok-comment">\/\/ TODO: fill sampleData\(\)<\/span>/);
});

test('HTML-special characters are escaped everywhere, including inside spans', () => {
  const out = highlightJava('List<수입물품내역> a && b < c;');
  assert.ok(!out.includes('List<수입물품내역>'));
  assert.match(out, /<span class="entity-tok-type">List<\/span>/);
  assert.ok(out.includes('&amp;&amp;'));
  assert.ok(out.includes('&lt;'));
});

test('is idempotent-safe on empty input', () => {
  assert.equal(highlightJava(''), '');
});
