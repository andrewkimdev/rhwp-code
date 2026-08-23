#!/usr/bin/env python3
"""template-entity 패리티 픽스처 빌더 (rhwp-code tests/fixtures/template-entity/).

hwpx-template-engine의 TemplateEntityGenerator 와 바이트 단위로 같은 출력을 내는지
검증하는 합성 .hwpx 픽스처를 만든다. 실물 내부 서식(고객명 포함 위험)은 쓰지 않고
더미 이름만 쓴다. 패키지 껍데기(mimetype/META-INF/header.xml/...)는 작은 실물
템플릿 licinf_911.hwpx 에서 가져오고 Contents/section0.xml 만 교체한다.

재생성: python3 scripts/template_entity_fixtures.py <licinf_911.hwpx 경로> <출력 디렉터리>
"""
import io
import sys
import zipfile

NS = (
    'xmlns:ha="http://www.hancom.co.kr/hwpml/2011/app" '
    'xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" '
    'xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" '
    'xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core" '
    'xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head" '
    'xmlns:hhs="http://www.hancom.co.kr/hwpml/2011/history" '
    'xmlns:hm="http://www.hancom.co.kr/hwpml/2011/master-page" '
    'xmlns:hpf="http://www.hancom.co.kr/schema/2011/hpf" '
    'xmlns:dc="http://purl.org/dc/elements/1.1/" '
    'xmlns:opf="http://www.idpf.org/2007/opf"'
)

_next_id = [100]


def _id() -> int:
    _next_id[0] += 7
    return _next_id[0]


def esc(t: str) -> str:
    return t.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')


def run_text(t: str) -> str:
    return f'<hp:run charPrIDRef="0"><hp:t>{esc(t)}</hp:t></hp:run>'


def para(children: str) -> str:
    return (
        f'<hp:p id="{_id()}" paraPrIDRef="0" styleIDRef="0" pageBreak="0" columnBreak="0">'
        f'{children}</hp:p>'
    )


def para_text(t: str) -> str:
    return para(run_text(t))


def field_begin(name: str, editable: bool = True) -> tuple[str, int]:
    begin_id = _id()
    xml = (
        f'<hp:fieldBegin id="{begin_id}" type="CLICK_HERE" name="{esc(name)}" '
        f'editable="{1 if editable else 0}" dirty="0" zorder="-1" fieldid="{_id()}">'
        '<hp:parameters cnt="2">'
        '<hp:integerParam name="Prop">9</hp:integerParam>'
        f'<hp:stringParam name="Command" xml:space="preserve">Clickhere:set:10:Direction:wstring:2:{esc(name)} HelpState:wstring:0:  </hp:stringParam>'
        '</hp:parameters></hp:fieldBegin>'
    )
    return xml, begin_id


def para_field(name: str, value: str = '') -> str:
    """누름틀 1개를 담은 문단 — fieldBegin/값 run/fieldEnd 실물 구조를 따른다."""
    fb, begin_id = field_begin(name)
    inner = (
        f'<hp:run charPrIDRef="0"><hp:ctrl>{fb}</hp:ctrl></hp:run>'
        f'{run_text(value)}'
        f'<hp:run charPrIDRef="0"><hp:ctrl><hp:fieldEnd id="{_id()}" beginIDRef="{begin_id}"/></hp:ctrl></hp:run>'
    )
    return para(inner)


def cell(content: str, row: int, col: int, row_span: int = 1, col_span: int = 1) -> str:
    return (
        '<hp:tc header="0" hasMargin="0" protect="0" editable="0" dirty="0" borderFillIDRef="0">'
        f'<hp:subList textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="CENTER">'
        f'{content}</hp:subList>'
        f'<hp:cellAddr colAddr="{col}" rowAddr="{row}"/>'
        f'<hp:cellSpan colSpan="{col_span}" rowSpan="{row_span}"/>'
        '<hp:cellSz width="3000" height="1000"/>'
        '</hp:tc>'
    )


def tbl(rows: str, row_cnt: int, col_cnt: int) -> str:
    return (
        f'<hp:tbl id="{_id()}" zOrder="0" numberingType="TABLE" textWrap="SQUARE" '
        f'textFlow="BOTH_SIDES" lock="0" rowCnt="{row_cnt}" colCnt="{col_cnt}" cellSpacing="0" '
        f'borderFillIDRef="0" noAdjust="0">'
        '<hp:sz width="40000" widthRelTo="ABSOLUTE" height="10000" heightRelTo="ABSOLUTE"/>'
        '<hp:pos treatAsChar="0" affectLSpacing="0" flowWithText="0" allowOverlap="0" '
        'vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" horzAlign="LEFT"/>'
        f'{rows}</hp:tbl>'
    )


def tbl_ctrl(rows: str, row_cnt: int, col_cnt: int) -> str:
    """표 1개 — 실물 구조상 <hp:tbl> 은 문단(<hp:p>)의 직접 자식이다 (ctrl 래퍼 아님,
    섹션 루트 직접 자식도 아님 — parse_hwpx_section 은 최상위에서 p 만 본다)."""
    return para(tbl(rows, row_cnt, col_cnt))


def cell_tbl(content: str, row: int, col: int, rows: str, row_cnt: int, col_cnt: int) -> str:
    """셀 안에 중첩 표를 넣는 형태 (필요 시). 마커 셀에는 쓰지 않는다."""
    nested = para(f'<hp:run charPrIDRef="0"><hp:ctrl>{tbl(rows, row_cnt, col_cnt)}</hp:ctrl></hp:run>')
    return cell(content + nested, row, col)


def marker_row(marker: str, col_cnt: int) -> str:
    """마커 전용 첫 행 — 첫 셀에만 마커 텍스트, 나머지 셀은 빈 문단."""
    cells = cell(para_text(marker), 0, 0)
    for c in range(1, col_cnt):
        cells += cell(para_text(''), 0, c)
    return f'<hp:tr>{cells}</hp:tr>'


def field_row(fields: list[str], col: int = 0) -> str:
    """필드들을 한 행의 셀들에 하나씩 담는다."""
    cells = ''
    for i, name in enumerate(fields):
        cells += cell(para_field(name, ''), 1, i)
    return f'<hp:tr>{cells}</hp:tr>'


def section(paras: str) -> bytes:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>'
        f'<hs:sec {NS}>{paras}</hs:sec>'
    ).encode('utf-8')


def build(shell: str, out_dir: str) -> None:
    import os

    fixtures: dict[str, bytes] = {}

    # ── flat: 상위 필드(mangling/keyword) + TITLE/BODY(seq)/FOOTER(sum) + PAGENO ──
    _next_id[0] = 100
    flat = ''
    flat += tbl_ctrl(
        marker_row('#REPEAT-TITLE:품목내역', 2) + field_row(['품목내역_단위']),
        2, 2)
    flat += tbl_ctrl(
        marker_row('#REPEAT-BODY:품목내역', 2)
        + field_row(['품목내역_명칭', '품목내역_수량', '#seq:품목내역_번호']),
        2, 3)
    flat += tbl_ctrl(
        marker_row('#REPEAT-FOOTER:품목내역', 2)
        + field_row(['#sum:품목내역_수량', '총합계_수량단위']),
        2, 2)
    flat += tbl_ctrl(
        marker_row('#PAGENO', 2) + field_row(['현재_페이지', '전체_페이지']),
        2, 2)
    # 상위 필드: 깨끗한 이름 / 괄호 / 숫자 시작 / 키워드 / 식별자 유지($)
    for name in ['신청인_성명', '신청인_성명(한글)', '1차_선택', 'record', '$보증금']:
        flat += para_field(name, '')
    fixtures['fix-flat.hwpx'] = section(flat)

    # ── nested: 2단계 -NESTED 그룹 (자식 TITLE 필드는 부모 블록 필드로 분류) ──
    _next_id[0] = 200
    nested = para_field('신청인_상호', '')
    nested += tbl_ctrl(
        marker_row('#REPEAT-BODY:수입물품내역', 2) + field_row(['수입물품내역_NO', '수입물품내역_원산지']),
        2, 2)
    nested += tbl_ctrl(
        marker_row('#REPEAT-TITLE-NESTED:수입물품내역/물품상세내역', 2) + field_row(['물품그룹_명칭']),
        2, 2)
    nested += tbl_ctrl(
        marker_row('#REPEAT-BODY-NESTED:수입물품내역/물품상세내역', 2)
        + field_row(['물품상세내역_상세명', '물품상세내역_상세수량']),
        2, 2)
    fixtures['fix-nested.hwpx'] = section(nested)

    # ── lenient: 마커 없음 — #seq:/#sum: 포함 전부 최상위 폴백 ──
    _next_id[0] = 300
    lenient = para_field('신청인_주소', '')
    lenient += tbl_ctrl(
        '<hp:tr>' + cell(para_text('품목'), 0, 0) + cell(para_text('수량'), 0, 1) + '</hp:tr>'
        + field_row(['품목내역_명칭', '품목내역_수량', '#seq:품목내역_번호', '#sum:품목내역_수량']),
        2, 2)
    fixtures['fix-lenient.hwpx'] = section(lenient)

    # ── error: BODY 마커 2개 → 검증 에러 ──
    _next_id[0] = 400
    err = tbl_ctrl(marker_row('#REPEAT-BODY:품목내역', 2) + field_row(['품목내역_명칭']), 2, 2)
    err += tbl_ctrl(marker_row('#REPEAT-BODY:품목내역', 2) + field_row(['품목내역_수량']), 2, 2)
    fixtures['fix-error-two-bodies.hwpx'] = section(err)

    os.makedirs(out_dir, exist_ok=True)
    shell_zf = zipfile.ZipFile(shell)
    for name, sec_xml in fixtures.items():
        out_path = os.path.join(out_dir, name)
        with zipfile.ZipFile(out_path, 'w', zipfile.ZIP_DEFLATED) as zf:
            for entry in shell_zf.namelist():
                if entry == 'Contents/section0.xml':
                    zf.writestr(entry, sec_xml)
                elif entry.startswith('BinData/') or entry.startswith('Preview/'):
                    continue  # 섹션이 참조하지 않는 이미지/미리보기는 떼어 작게 만든다
                else:
                    zf.writestr(entry, shell_zf.read(entry))
        print(f'wrote {out_path}')


if __name__ == '__main__':
    build(sys.argv[1], sys.argv[2])
