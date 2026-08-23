package com.example.fix;

import java.io.IOException;

import com.ktnet.aspline.hwpx.tooling.template.HwpxTemplateModule;

/**
 * 'fix-flat' 템플릿 모듈 초안 — TemplateEntityGenerator가 생성함. sampleData()를 채우는 걸 권장합니다.
 * 등록하려면 HwpxTemplateEngineApplication의 모듈 목록에 인스턴스를 추가하세요.
 */
public class FixFlatTemplateModule implements HwpxTemplateModule<FixFlatData> {

    private static final String RESOURCE_PATH = "/hwpx/fix-flat.hwpx";

    @Override
    public String code() {
        return "fix-flat";
    }

    @Override
    public Class<FixFlatData> dataType() {
        return FixFlatData.class;
    }

    @Override
    public byte[] hwpxBytes() throws IOException {
        return HwpxTemplateModule.readClasspathResource(FixFlatTemplateModule.class, RESOURCE_PATH);
    }

    // TODO: 데모/스키마 미리보기용 예시 데이터가 필요하면 sampleData()를 override하세요.
}
