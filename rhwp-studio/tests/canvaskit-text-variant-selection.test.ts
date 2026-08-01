import assert from 'node:assert/strict';
import test from 'node:test';

import type { LayerGlyphOutlineOp, LayerGlyphRunOp, LayerPaintOp, LayerTextRunOp } from '../src/core/types.ts';
import {
  selectLayerTextVariantsForLeaf,
  staticSvgPathLayersAreReplayable,
} from '../src/view/canvaskit/text-variant-selection.ts';

function textFallback(group: string): LayerTextRunOp {
  return {
    type: 'textRun',
    bbox: { x: 0, y: 0, width: 16, height: 16 },
    text: 'A',
    variant: {
      equivalenceGroup: group,
      variantId: 'textRun',
      variantKind: 'textRun',
      isDefaultFallback: true,
    },
  };
}

function glyphSidecar(group: string, payloadKind: 'bitmapGlyph' | 'svgGlyph'): LayerGlyphOutlineOp {
  return {
    type: 'glyphOutline',
    bbox: { x: 0, y: 0, width: 16, height: 16 },
    payloadKind,
    variant: {
      equivalenceGroup: group,
      variantId: payloadKind,
      variantKind: 'glyphOutline',
      isDefaultFallback: false,
    },
  };
}

function glyphRun(group: string, partIndex = 0, partCount = 1): LayerGlyphRunOp {
  return {
    type: 'glyphRun',
    bbox: { x: 0, y: 0, width: 16, height: 16 },
    source: { id: 0, utf8Range: { start: 0, end: 1 }, utf16Range: { start: 0, end: 1 } },
    variant: {
      equivalenceGroup: group,
      variantId: 'glyphRun',
      variantKind: 'glyphRun',
      partIndex,
      partCount,
    },
    paintStyle: { fontSize: 16, color: '#000000' },
    shapeKey: {
      fontInstance: { faceKey: 'face-0', sizePx: 16 },
      direction: 'ltr',
      writingMode: 'horizontal-tb',
      shapingEngine: 'test',
      fallbackPolicy: 'none',
    },
    placement: { runToPage: { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 }, baselineY: 0 },
    glyphIds: [1],
    positions: [{ x: 0, y: 0 }],
    clusters: [],
    direction: 'ltr',
    writingMode: 'horizontal-tb',
    orientation: 'horizontal',
    diagnostics: {
      quality: 'exact',
      replayEligibility: 'portable',
      strictVisualEligible: true,
      maxOriginDeltaPx: 0,
      maxAdvanceDeltaPx: 0,
      maxResidualAfterAdjustmentPx: 0,
      clusterMismatchCount: 0,
      missingGlyphCount: 0,
      usedFallbackFontCount: 0,
    },
  };
}

test('selects a decodable glyph sidecar exclusively', () => {
  const fallback = textFallback('text-0');
  const sidecar = glyphSidecar('text-0', 'bitmapGlyph');
  const selected = selectLayerTextVariantsForLeaf([fallback, sidecar], () => true);

  assert.deepEqual([...selected], [sidecar]);
});

test('keeps text fallback for corrupt, missing, oversized, or unparseable resources', () => {
  const failures = ['invalidBase64', 'decodeThrows', 'decodeNull', 'oversized', 'missingResource', 'unparseableSvg'];
  for (const failure of failures) {
    const fallback = textFallback(failure);
    const sidecar = glyphSidecar(failure, failure === 'unparseableSvg' ? 'svgGlyph' : 'bitmapGlyph');
    const ops: LayerPaintOp[] = [fallback, sidecar];
    const selected = selectLayerTextVariantsForLeaf(ops, () => false);
    assert.deepEqual([...selected], [fallback], failure);
  }
});

test('selects each equivalence group independently', () => {
  const firstFallback = textFallback('text-0');
  const firstSidecar = glyphSidecar('text-0', 'bitmapGlyph');
  const secondFallback = textFallback('text-1');
  const secondSidecar = glyphSidecar('text-1', 'svgGlyph');
  const selected = selectLayerTextVariantsForLeaf(
    [firstFallback, firstSidecar, secondFallback, secondSidecar],
    op => op.payloadKind === 'svgGlyph',
  );

  assert.deepEqual([...selected], [firstFallback, secondSidecar]);
});

test('selects a complete replayable GlyphRun when no outline is available', () => {
  const fallback = textFallback('text-glyph');
  const strict = glyphRun('text-glyph');
  const selected = selectLayerTextVariantsForLeaf(
    [fallback, strict],
    () => false,
    () => true,
  );

  assert.deepEqual([...selected], [strict]);
});

test('keeps fallback for incomplete or duplicate multipart GlyphRun variants', () => {
  for (const strictParts of [
    [glyphRun('incomplete', 0, 2)],
    [glyphRun('duplicate', 0, 2), glyphRun('duplicate', 0, 2)],
  ]) {
    const fallback = textFallback(strictParts[0].variant.equivalenceGroup);
    const selected = selectLayerTextVariantsForLeaf(
      [fallback, ...strictParts],
      () => false,
      () => true,
    );
    assert.deepEqual([...selected], [fallback]);
  }
});

test('keeps fallback for invalid or mixed multipart variant metadata', () => {
  const outOfRange = glyphRun('out-of-range', 2, 2);
  const oversized = glyphRun('oversized', 0, 4097);
  const mixedRun = glyphRun('mixed');
  const mixedOutline = glyphSidecar('mixed', 'svgGlyph');
  mixedOutline.variant!.variantId = 'glyphRun';

  for (const [group, strictParts] of [
    ['out-of-range', [outOfRange]],
    ['oversized', [oversized]],
    ['mixed', [mixedRun, mixedOutline]],
  ] as const) {
    const fallback = textFallback(group);
    const selected = selectLayerTextVariantsForLeaf(
      [fallback, ...strictParts],
      () => true,
      () => true,
    );
    assert.deepEqual([...selected], [fallback], group);
  }
});

test('prefers a replayable outline over GlyphRun and fallback', () => {
  const fallback = textFallback('outline-first');
  const strictRun = glyphRun('outline-first');
  const outline = glyphSidecar('outline-first', 'svgGlyph');
  const selected = selectLayerTextVariantsForLeaf(
    [fallback, strictRun, outline],
    () => true,
    () => true,
  );

  assert.deepEqual([...selected], [outline]);
});

test('does not evaluate lower-priority GlyphRun when an outline is selected', () => {
  const fallback = textFallback('outline-lazy');
  const strictRun = glyphRun('outline-lazy');
  const outline = glyphSidecar('outline-lazy', 'svgGlyph');
  let glyphRunChecks = 0;
  const selected = selectLayerTextVariantsForLeaf(
    [fallback, strictRun, outline],
    () => true,
    () => {
      glyphRunChecks += 1;
      return false;
    },
  );

  assert.deepEqual([...selected], [outline]);
  assert.equal(glyphRunChecks, 0);
});

test('rejects a variant that spoofs another paint op kind', () => {
  const fallback = textFallback('kind-spoof');
  const strictRun = glyphRun('kind-spoof');
  const spoofedText = textFallback('kind-spoof');
  spoofedText.variant!.variantId = 'glyphRun';
  spoofedText.variant!.variantKind = 'glyphRun';
  spoofedText.variant!.isDefaultFallback = false;
  const selected = selectLayerTextVariantsForLeaf(
    [fallback, strictRun, spoofedText],
    () => true,
    () => true,
  );

  assert.deepEqual([...selected], [fallback]);
});

test('rejects SVG layers when CanvasKit path construction throws or returns null', () => {
  const layers = [{ pathData: 'not-a-path', fill: '#000000', opacity: 1 }];
  assert.equal(staticSvgPathLayersAreReplayable(layers, () => null), false);
  assert.equal(staticSvgPathLayersAreReplayable(layers, () => { throw new Error('parse'); }), false);
  let deleted = false;
  assert.equal(staticSvgPathLayersAreReplayable(layers, () => ({ delete: () => { deleted = true; } })), true);
  assert.equal(deleted, true);
});
