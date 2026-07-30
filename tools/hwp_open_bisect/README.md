# hwp_open_bisect — 저장했는데 한컴이 못 여는 결함 추적

`rhwp` 가 저장한 HWP 를 한컴이 **열지 못하는** 결함(#3565 계열)은 `convert --verify` 로
잡히지 않는다. rhwp 자기 파서가 자기가 쓴 파일을 그대로 되읽기 때문이다. 판정자는
한컴뿐이고, 문제는 "387쪽 문서가 안 열린다"에서 원인까지 어떻게 좁히느냐다.

이 도구는 **한컴이 같은 원본을 저장한 파일을 정답지로 삼아**, rhwp 산출물의 일부만
정답지에 이식해 개방 여부로 이분한다. 코드를 고치지 않고 결함 위치를 짚는다.

## 절차

```bash
SRC="samples/문제문서.hwpx"
O=oracle.hwp; C=candidate.hwp

# 0) 후보와 정답지를 만든다
rhwp convert "$SRC" $C
python tools/hwp_open_bisect/hangul_com.py save-as "$SRC" $O

# 1) 재현 확인 — 후보는 못 열리고 정답지는 열려야 한다
python tools/hwp_open_bisect/hangul_com.py check-open $C   # rc=1
python tools/hwp_open_bisect/hangul_com.py check-open $O   # rc=0

# 2) 구조를 본다 — 레코드 수·태그·레벨·크기가 어디서 갈리는가
python tools/hwp_open_bisect/record_bisect.py diff $O $C

# 3) 구역 단위로 좁힌다
python tools/hwp_open_bisect/record_bisect.py hybrid $O $C h.hwp --stream BodyText/Section7
python tools/hwp_open_bisect/hangul_com.py check-open h.hwp

# 4) 레코드 종류로 좁힌다 (--invert 로 여집합도 함께 본다)
python tools/hwp_open_bisect/record_bisect.py hybrid $O $C h.hwp --section 7 --tag 76

# 5) 바이트 구간까지 좁힌다
python tools/hwp_open_bisect/record_bisect.py hybrid $O $C h.hwp \
    --section 7 --tag 76 --restore-bytes 190-220
```

`--tag` 와 `--invert` 를 **한 쌍으로** 돌려 상보 결과(하나는 실패, 하나는 개방)를 얻으면
그 종류가 단독 원인이라는 확증이 된다.

## 반드시 지킬 것

**대조군을 매 라운드에 넣는다.** 이식 실험은 원래 결함이 아닌 **새 결함**을 쉽게 만든다.
#3565 추적에서 실제로 걸린 것들:

| 자해 실험 | 왜 깨졌나 |
|---|---|
| 라이브러리 저장 직접 호출 | `doc_properties.section_count` 가 stale(1) 이라 구역 14개 문서에 1 을 쓴다 |
| 컨트롤만 제거 | 본문의 컨트롤 문자와 `char_count` 를 그대로 둬 개수가 어긋난다 |
| 본문만 이식 | 후보 본문이 정답지 DocInfo 의 없는 ID 를 가리킨다 (`--with-docinfo` 기본값이 막는다) |
| `PARA_HEADER` 만 이식 | "영역 태그 0개" 선언과 정답지 `PARA_RANGE_TAG` 레코드가 공존한다 |

무이식 판(`hybrid` 를 `--records 0-0` 으로)이 **열려야** 도구·파이프라인이 건전하다는
뜻이다. 이 대조 없이 얻은 판정은 믿지 않는다.

**COM 은 직렬로.** 판정을 동시에 돌리면 서로의 `Hwp.exe` 를 죽여 "무응답" 오판이 난다.
한 프로세스에서 `Hwp()` 를 두 번 만들지도 말 것(두 번째가 `com_error` 로 죽는다).

## #3565 에서 나온 결과

387쪽 13MB 문서 → 구역 → 문단 → 레코드 종류 → 바이트 순으로 좁혀,
그룹 컨테이너가 자식 목록에 쓰는 **종류 선언이 실제 레코드와 어긋난다**는 것을 찾았다
(중첩 그룹 `gso ` vs `$con`, 연결선 `$lin` vs `$col`). 레코드 **구조 자체는 정답지와
완전히 일치**했고 차이는 그 목록 내용뿐이었다.

같은 대조에서 개방과 무관한 정합 손실도 함께 드러났다 — #3567(영역 태그 전량 소실),
#3568(탭 매개변수 소실), #3569(BorderFill 초과), #3570(표·선 레코드 크기 불일치).

## 의존

`olefile`, `pyhwpx`(COM 판정용, Windows + 한컴 설치 필요). `diff`/`hybrid` 는 COM 없이
동작하므로 정답지만 확보하면 어느 환경에서나 쓸 수 있다.
