import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CANVASKIT_MAX_ENCODED_IMAGE_BASE64_LENGTH,
  encodedImageDimensions,
  encodedImageHeader,
  encodedImageIsReplayable,
} from '../src/view/canvaskit/image-header.ts';

function png(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(33);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const view = new DataView(bytes.buffer);
  view.setUint32(8, 13);
  bytes.set([0x49, 0x48, 0x44, 0x52], 12);
  view.setUint32(16, width);
  view.setUint32(20, height);
  bytes.set([8, 6, 0, 0, 0], 24);
  return bytes;
}

function jpeg(width: number, height: number): Uint8Array {
  return new Uint8Array([
    0xff, 0xd8,
    0xff, 0xe0, 0x00, 0x04, 0x00, 0x00,
    0xff, 0xc0, 0x00, 0x0b, 0x08,
    height >> 8, height & 0xff,
    width >> 8, width & 0xff,
    0x01, 0x01, 0x11, 0x00,
  ]);
}

function gif(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(13);
  bytes.set([0x47, 0x49, 0x46, 0x38, 0x39, 0x61]);
  const view = new DataView(bytes.buffer);
  view.setUint16(6, width, true);
  view.setUint16(8, height, true);
  return bytes;
}

function webp(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(30);
  bytes.set([0x52, 0x49, 0x46, 0x46]);
  const view = new DataView(bytes.buffer);
  view.setUint32(4, 22, true);
  bytes.set([0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58], 8);
  view.setUint32(16, 10, true);
  const encodedWidth = width - 1;
  const encodedHeight = height - 1;
  bytes.set([
    encodedWidth & 0xff,
    (encodedWidth >> 8) & 0xff,
    (encodedWidth >> 16) & 0xff,
  ], 24);
  bytes.set([
    encodedHeight & 0xff,
    (encodedHeight >> 8) & 0xff,
    (encodedHeight >> 16) & 0xff,
  ], 27);
  return bytes;
}

function bmp(width: number, height: number): Uint8Array {
  const bytes = new Uint8Array(54);
  bytes.set([0x42, 0x4d]);
  const view = new DataView(bytes.buffer);
  view.setUint32(2, 54, true);
  view.setUint32(10, 54, true);
  view.setUint32(14, 40, true);
  view.setInt32(18, width, true);
  view.setInt32(22, height, true);
  view.setUint16(26, 1, true);
  view.setUint16(28, 24, true);
  return bytes;
}

test('encoded image admission accepts bounded browser formats', () => {
  const fixtures = [
    [png(320, 240), { format: 'png', width: 320, height: 240 }],
    [jpeg(300, 200), { format: 'jpeg', width: 300, height: 200 }],
    [gif(160, 120), { format: 'gif', width: 160, height: 120 }],
    [webp(640, 480), { format: 'webp', width: 640, height: 480 }],
    [bmp(800, -600), { format: 'bmp', width: 800, height: 600 }],
  ] as const;

  for (const [bytes, expected] of fixtures) {
    assert.deepEqual(encodedImageHeader(bytes), expected);
    assert.deepEqual(encodedImageDimensions(bytes), {
      width: expected.width,
      height: expected.height,
    });
    assert.equal(encodedImageIsReplayable(bytes), true);
  }
  assert.equal(encodedImageIsReplayable(png(8192, 4096)), true);
});

test('encoded image admission rejects malformed and truncated structures', () => {
  const fixtures = [png(1, 1), jpeg(1, 1), gif(1, 1), webp(1, 1), bmp(1, 1)];
  for (const bytes of fixtures) {
    assert.notEqual(encodedImageHeader(bytes), null);
    assert.equal(encodedImageHeader(bytes.subarray(0, bytes.byteLength - 1)), null);
  }

  const malformedPng = png(1, 1);
  new DataView(malformedPng.buffer).setUint32(8, 12);
  assert.equal(encodedImageHeader(malformedPng), null);

  const malformedGif = gif(1, 1);
  malformedGif[10] = 0x80;
  assert.equal(encodedImageHeader(malformedGif), null);

  const malformedWebp = webp(1, 1);
  new DataView(malformedWebp.buffer).setUint32(4, 100, true);
  assert.equal(encodedImageHeader(malformedWebp), null);

  const malformedBmp = bmp(1, 1);
  new DataView(malformedBmp.buffer).setUint32(10, 10, true);
  assert.equal(encodedImageHeader(malformedBmp), null);
  new DataView(malformedBmp.buffer).setUint32(10, 55, true);
  assert.equal(encodedImageHeader(malformedBmp), null);

  const malformedJpeg = jpeg(1, 1);
  malformedJpeg[17] = 2;
  assert.equal(encodedImageHeader(malformedJpeg), null);
});

test('encoded image admission rejects oversized payloads before decode', () => {
  assert.equal(encodedImageIsReplayable(new Uint8Array()), false);
  assert.equal(encodedImageIsReplayable(png(8193, 1)), false);
  assert.equal(encodedImageIsReplayable(jpeg(8192, 8192)), false);

  const maxRawBytes = Math.floor(CANVASKIT_MAX_ENCODED_IMAGE_BASE64_LENGTH / 4) * 3;
  const overEncodedLimit = new Uint8Array(maxRawBytes + 1);
  overEncodedLimit.set(png(1, 1));
  assert.equal(encodedImageIsReplayable(overEncodedLimit), false);
});
