package com.example.fix;

import java.util.List;

import com.fasterxml.jackson.annotation.JsonProperty;

/** 'fix-flat' 템플릿의 타입 데이터 클래스 초안 — TemplateEntityGenerator가 생성함. 리뷰 후 사용하세요. */
public record FixFlatData(
        String 품목내역_단위,
        String 총합계_수량단위,
        String 신청인_성명,
        @JsonProperty("신청인_성명(한글)") String 신청인_성명_한글_,
        @JsonProperty("1차_선택") String _차_선택,
        @JsonProperty("record") String record_,
        String $보증금,
        List<품목내역> 품목내역) {

    public record 품목내역(
            String 품목내역_명칭,
            String 품목내역_수량) {
    }
}
