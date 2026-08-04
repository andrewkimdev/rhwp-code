---
kind: analysis
status: active
canonical: mydocs/working/task_m100_3820_stage1.md
last_verified: 2026-08-04
---

# Task #3820·#3821 Stage 5 — p108 TIFF 그림 52 미출력 분석

## 재현

human p108의 render-tree에는 그림 52가 누락된 것이 아니라 Body `Image(pi=1147, ci=0)`로
존재한다. bbox는 `x=105.1, y=227.3, w=482.4, h=100.8`, wrap은 `TopAndBottom`이다.
그러나 rhwp SVG는 해당 node를 다음처럼 raw `data:image/tiff`로 방출하며, rsvg/브라우저는
이를 안정적으로 decode하지 않아 raster review에는 caption만 남는다. 한컴 PDF에는 같은 위치에
청색 flow diagram과 caption이 모두 있다.

```text
<image x="105.066..." y="227.264..." width="482.4" height="100.8"
       href="data:image/tiff;base64,..."/>
```

`stage3-review-pairs/p108_rhwp_pdf.png`가 직접 증적이다. 따라서 이는 페이지 owner나
TopAndBottom 흐름 결함이 아니라 browser-compatible image emission 결함이다.

## 기존 기능과 누락 경로

`image_resolver::tiff_bytes_to_png_bytes()`와 `emitted_image_bytes()`는 이미 TIFF→PNG
변환을 구현하며, `html.rs`도 `image/tiff`를 PNG로 내보낸다. 하지만 SVG의
`render_image_node`, `render_page_background_image`, generic `draw_image`는 BMP/PCX만
변환하고 TIFF branch가 없다. Wasm `web_canvas::draw_image`도 자체 MIME 판별기에 TIFF가
없고 raw data URI를 `HtmlImageElement`로 넘긴다.

즉 paint/HTML의 정상 경로가 SVG export와 browser canvas의 fallback에 전파되지 않은 상태다.

## 예정 변경과 수용 기준

1. SVG의 세 image-emission path와 Wasm WebCanvas를 HTML과 같은 TIFF→PNG converter로
   연결한다. 변환 실패 시 기존 raw payload fallback은 유지한다.
2. SVG renderer unit은 TIFF `ImageNode`가 `data:image/png;base64,`로 방출됨을 고정한다.
3. p108 SVG의 `image/tiff`가 사라지고 PNG가 나오며, raster 쌍에서 그림 52가 PDF와 같은
   페이지에 보인다.
4. p109 및 p156과 #3820 focused regressions를 다시 확인한다. 이 문서가 commit된 뒤에만
   code/test를 수정한다.
