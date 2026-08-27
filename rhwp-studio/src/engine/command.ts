/**
 * 편집 명령(EditCommand) 모듈군의 인덱스 — 구현은 ./command/ 관심사별 모듈에 있다.
 * 기존 임포트 경로('./command', '@/engine/command')의 공개 표면은 그대로 유지된다.
 *
 * - types — EditCommand/EditContext 계약, text mutation effect, OperationDescriptor
 * - cell-path — 본문/셀 분기·좌표축 헬퍼(isCell/cellPathJson/cellParaIndexOf/charCount)
 * - text-mutation — 텍스트 뮤테이션 헬퍼(insert/delete/replace + ...Immediate)
 * - text-commands — 본문 텍스트·문단 커맨드(삽입/삭제/줄바꿈/탭/분할/병합/선택 삭제)
 * - format-commands — 글자·문단 서식 커맨드
 * - submode-commands — 머리말/꼬리말·각주(HF/FN) 편집 커맨드
 * - cell-commands — 셀 내부 문단 구조 커맨드
 * - object-commands — 표/그림/도형 이동·리사이즈·양식 값 커맨드
 * - snapshot-command — 스냅샷 기반 커맨드(SnapshotCommand/SubmodeSnapshotCommand)
 */
export * from './command/types';
export * from './command/cell-path';
export * from './command/text-mutation';
export * from './command/text-commands';
export * from './command/format-commands';
export * from './command/submode-commands';
export * from './command/cell-commands';
export * from './command/object-commands';
export * from './command/snapshot-command';
