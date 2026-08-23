package com.example.fix;

import com.fasterxml.jackson.annotation.JsonProperty;

/** 'fix-lenient' 템플릿의 타입 데이터 클래스 초안 — TemplateEntityGenerator가 생성함. 리뷰 후 사용하세요. */
public record FixLenientData(
        String 신청인_주소,
        String 품목내역_명칭,
        String 품목내역_수량,
        @JsonProperty("#seq:품목내역_번호") String _seq_품목내역_번호,
        @JsonProperty("#sum:품목내역_수량") String _sum_품목내역_수량) {
}
