import type { DeferredPaginationResult } from '@/core/wasm-bridge';

export interface DeferredPaginationClient {
  beginDeferredPagination(fragmentBudget: number): DeferredPaginationResult;
  stepDeferredPagination(fragmentBudget: number): DeferredPaginationResult;
  cancelDeferredPagination(): boolean;
}

type ScheduledTask = ReturnType<typeof setTimeout>;
type ScheduleTask = (callback: () => void, delayMs?: number) => ScheduledTask;
type RunnerState = 'idle' | 'begin-scheduled' | 'stepping';

/**
 * #2424 continuation을 한 macrotask당 한 fragment budget씩 전진시킨다.
 * 입력은 `requestStart()`로 최신 revision 하나만 남긴다. 최초 admission은 다음 task에서 바로
 * 확인하고, 이미 전진 중이거나 restart debounce 중인 job만 지정한 window까지 합친다.
 */
export class DeferredPaginationRunner {
  private scheduled: ScheduledTask | null = null;
  private state: RunnerState = 'idle';
  private scheduledStartDelayMs: number | null = null;
  private generation = 0;

  constructor(
    private readonly client: DeferredPaginationClient,
    private readonly onComplete: (result: DeferredPaginationResult) => void,
    private readonly onFallback: (result: DeferredPaginationResult) => void,
    private readonly fragmentBudget = 1,
    private readonly scheduleTask: ScheduleTask = (callback, delayMs = 0) =>
      setTimeout(callback, delayMs),
    private readonly cancelTask: (task: ScheduledTask) => void = (task) => clearTimeout(task),
  ) {}

  isActive(): boolean {
    return this.state === 'stepping';
  }

  hasPendingWork(): boolean {
    return this.state !== 'idle';
  }

  /**
   * 기존 continuation을 폐기하고 최신 descriptor의 begin을 input stack 밖에 예약한다.
   *
   * 아직 admission을 확인하지 않은 최초 begin은 0ms를 유지한다. 성공해 전진 중인 job 또는
   * 이미 debounce 중인 restart만 `restartDelayMs`를 다시 적용하므로 unsupported fallback을
   * 고정 window만큼 늦추지 않는다.
   */
  requestStart(restartDelayMs: number): void {
    const normalizedRestartDelayMs = Math.max(0, restartDelayMs);
    const delayMs = this.state === 'stepping'
      || (this.state === 'begin-scheduled' && (this.scheduledStartDelayMs ?? 0) > 0)
      ? normalizedRestartDelayMs
      : 0;

    const generation = ++this.generation;
    this.cancelScheduledTask();
    this.client.cancelDeferredPagination();
    this.state = 'begin-scheduled';
    this.scheduledStartDelayMs = delayMs;
    this.scheduled = this.scheduleTask(() => {
      if (!this.isCurrent(generation, 'begin-scheduled')) return;
      this.scheduled = null;
      this.scheduledStartDelayMs = null;
      try {
        this.accept(this.client.beginDeferredPagination(this.fragmentBudget), generation);
      } catch {
        this.fail(generation);
      }
    }, delayMs);
  }

  cancel(): void {
    ++this.generation;
    this.cancelScheduledTask();
    if (this.state === 'stepping') {
      this.client.cancelDeferredPagination();
    }
    this.state = 'idle';
    this.scheduledStartDelayMs = null;
  }

  private accept(result: DeferredPaginationResult, generation: number): void {
    if (generation !== this.generation) return;
    if (result.status === 'pending') {
      this.state = 'stepping';
      this.scheduleNextStep(generation);
      return;
    }
    this.state = 'idle';
    if (result.status === 'complete') {
      this.onComplete(result);
      return;
    }
    this.onFallback(result);
  }

  private scheduleNextStep(generation: number): void {
    if (!this.isCurrent(generation, 'stepping') || this.scheduled !== null) return;
    this.scheduled = this.scheduleTask(() => {
      if (!this.isCurrent(generation, 'stepping')) return;
      this.scheduled = null;
      try {
        this.accept(this.client.stepDeferredPagination(this.fragmentBudget), generation);
      } catch {
        this.fail(generation);
      }
    }, 0);
  }

  private fail(generation: number): void {
    if (generation !== this.generation) return;
    this.state = 'idle';
    this.scheduledStartDelayMs = null;
    this.onFallback({
      ok: false,
      status: 'fallback',
      revision: 0,
      fragmentsProcessed: 0,
      pageCount: 0,
    });
  }

  private isCurrent(generation: number, state: RunnerState): boolean {
    return generation === this.generation && this.state === state;
  }

  private cancelScheduledTask(): void {
    if (this.scheduled === null) return;
    this.cancelTask(this.scheduled);
    this.scheduled = null;
  }
}
