package com.example.fix;

import java.util.List;

/** 'fix-nested' 템플릿의 타입 데이터 클래스 초안 — TemplateEntityGenerator가 생성함. 리뷰 후 사용하세요. */
public record FixNestedData(
        String 신청인_상호,
        List<수입물품내역> 수입물품내역) {

    public record 수입물품내역(
            String 수입물품내역_NO,
            String 수입물품내역_원산지,
            String 물품그룹_명칭,
            List<물품상세내역> 물품상세내역) {

        public record 물품상세내역(
                String 물품상세내역_상세명,
                String 물품상세내역_상세수량) {
        }
    }
}
