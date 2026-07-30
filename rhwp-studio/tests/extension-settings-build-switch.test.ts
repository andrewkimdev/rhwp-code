import assert from 'node:assert/strict';
import test from 'node:test';
import { loadExtensionViewerSettings } from '../src/core/extension-settings.ts';

type MutableGlobal = typeof globalThis & {
  __RHWP_DISABLE_EXTERNAL_WEBFONTS__?: boolean;
  chrome?: unknown;
  browser?: unknown;
};

const globalRef = globalThis as MutableGlobal;

async function withGlobals(
  overrides: Partial<Record<'__RHWP_DISABLE_EXTERNAL_WEBFONTS__' | 'chrome' | 'browser', unknown>>,
  run: () => Promise<void>,
): Promise<void> {
  const keys = ['__RHWP_DISABLE_EXTERNAL_WEBFONTS__', 'chrome', 'browser'] as const;
  const previous = new Map<string, { existed: boolean; value: unknown }>();
  for (const key of keys) {
    previous.set(key, { existed: key in globalRef, value: globalRef[key] });
    if (key in overrides) {
      (globalRef as Record<string, unknown>)[key] = overrides[key];
    } else {
      delete (globalRef as Record<string, unknown>)[key];
    }
  }
  try {
    await run();
  } finally {
    for (const key of keys) {
      const saved = previous.get(key)!;
      if (saved.existed) {
        (globalRef as Record<string, unknown>)[key] = saved.value;
      } else {
        delete (globalRef as Record<string, unknown>)[key];
      }
    }
  }
}

test('빌드 스위치가 없으면 비확장 환경 기본값은 외부 웹폰트 허용(false)이다', async () => {
  await withGlobals({}, async () => {
    const settings = await loadExtensionViewerSettings();
    assert.equal(settings.disableExternalWebFonts, false);
  });
});

test('빌드 스위치가 켜지면 비확장 환경 기본값이 외부 웹폰트 사용 안 함(true)이 된다', async () => {
  await withGlobals({ __RHWP_DISABLE_EXTERNAL_WEBFONTS__: true }, async () => {
    const settings = await loadExtensionViewerSettings();
    assert.equal(settings.disableExternalWebFonts, true);
  });
});

test('확장 storage에 저장된 명시값은 빌드 스위치보다 우선한다', async () => {
  const fakeChrome = {
    runtime: {},
    storage: {
      sync: {
        get(
          _defaults: Record<string, unknown>,
          callback?: (items: Record<string, unknown>) => void,
        ) {
          callback?.({ disableExternalWebFonts: false });
        },
      },
    },
  };
  await withGlobals(
    { __RHWP_DISABLE_EXTERNAL_WEBFONTS__: true, chrome: fakeChrome },
    async () => {
      const settings = await loadExtensionViewerSettings();
      assert.equal(settings.disableExternalWebFonts, false);
    },
  );
});
