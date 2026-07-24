import type { EventBus } from '@/core/event-bus';

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 4.0;
const ZOOM_SETTLE_EPSILON = 0.001;
const ZOOM_SMOOTHING_TIME_MS = 22;
const WHEEL_ZOOM_SENSITIVITY = 0.0042;
const MAX_WHEEL_DELTA_PX = 120;

export class ViewportManager {
  private scrollY = 0;
  private scrollX = 0;
  private viewportWidth = 0;
  private viewportHeight = 0;
  private zoom = 1.0;
  private container: HTMLElement | null = null;
  private resizeObserver: ResizeObserver | null = null;
  private scrollAnimationFrame: number | null = null;
  private zoomAnimationFrame: number | null = null;
  private zoomAnimationTimestamp: number | null = null;
  private zoomAnimating = false;
  private zoomTarget = 1.0;
  private onScrollBound: () => void;
  private onWheelBound: (e: WheelEvent) => void;
  private onZoomAnimationFrameBound: (timestamp: number) => void;
  private eventBus: EventBus;

  constructor(eventBus: EventBus) {
    this.eventBus = eventBus;
    this.onScrollBound = this.onScroll.bind(this);
    this.onWheelBound = this.onWheel.bind(this);
    this.onZoomAnimationFrameBound = this.onZoomAnimationFrame.bind(this);
  }

  /** 스크롤 컨테이너에 연결한다 */
  attachTo(container: HTMLElement): void {
    this.detach();
    this.container = container;
    container.addEventListener('scroll', this.onScrollBound, { passive: true });
    container.addEventListener('wheel', this.onWheelBound, { passive: false });

    this.resizeObserver = new ResizeObserver(() => {
      this.updateViewportSize();
      this.eventBus.emit('viewport-resize', this.viewportWidth, this.viewportHeight);
    });
    this.resizeObserver.observe(container);
    this.updateViewportSize();
  }

  /** 연결을 해제한다 */
  detach(): void {
    if (this.container) {
      this.container.removeEventListener('scroll', this.onScrollBound);
      this.container.removeEventListener('wheel', this.onWheelBound);
    }
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
    if (this.scrollAnimationFrame !== null) {
      cancelAnimationFrame(this.scrollAnimationFrame);
      this.scrollAnimationFrame = null;
    }
    this.cancelZoomAnimation();
    this.container = null;
  }

  private onScroll(): void {
    if (!this.container) return;
    this.scrollY = this.container.scrollTop;
    this.scrollX = this.container.scrollLeft;
    if (this.scrollAnimationFrame !== null) return;

    // 스크롤 중에는 최신 좌표만 다음 프레임에 반영해 렌더가 입력 이벤트를 막지 않게 한다.
    this.scrollAnimationFrame = requestAnimationFrame(() => {
      this.scrollAnimationFrame = null;
      this.eventBus.emit('viewport-scroll', this.scrollY, this.scrollX);
    });
  }

  /** Ctrl+휠: 브라우저 줌 대신 문서 줌 */
  private onWheel(e: WheelEvent): void {
    if (!e.ctrlKey && !e.metaKey) return;
    e.preventDefault();

    const deltaPixels = e.deltaMode === 1
      ? e.deltaY * 16
      : e.deltaMode === 2
        ? e.deltaY * Math.max(this.viewportHeight, 1)
        : e.deltaY;
    const boundedDelta = Math.max(
      -MAX_WHEEL_DELTA_PX,
      Math.min(MAX_WHEEL_DELTA_PX, deltaPixels),
    );
    if (boundedDelta === 0) return;

    this.smoothZoomTo(
      this.zoomTarget * Math.exp(-boundedDelta * WHEEL_ZOOM_SENSITIVITY),
    );
  }

  private updateViewportSize(): void {
    if (!this.container) return;
    this.viewportWidth = this.container.clientWidth;
    this.viewportHeight = this.container.clientHeight;
  }

  getScrollY(): number {
    return this.scrollY;
  }

  getScrollX(): number {
    return this.scrollX;
  }

  getViewportSize(): { width: number; height: number } {
    return { width: this.viewportWidth, height: this.viewportHeight };
  }

  getZoom(): number {
    return this.zoom;
  }

  setZoom(zoom: number): void {
    this.cancelZoomAnimation();
    this.zoom = this.clampZoom(zoom);
    this.zoomTarget = this.zoom;
    this.eventBus.emit('zoom-changed', this.zoom);
  }

  smoothZoomBy(delta: number): void {
    this.smoothZoomTo(this.zoomTarget + delta);
  }

  smoothZoomTo(zoom: number): void {
    this.zoomTarget = this.clampZoom(zoom);
    if (Math.abs(this.zoomTarget - this.zoom) <= ZOOM_SETTLE_EPSILON) {
      this.setZoom(this.zoomTarget);
      return;
    }
    this.zoomAnimating = true;
    if (this.zoomAnimationFrame === null) {
      this.zoomAnimationFrame = requestAnimationFrame(this.onZoomAnimationFrameBound);
    }
  }

  isZoomAnimating(): boolean {
    return this.zoomAnimating;
  }

  private onZoomAnimationFrame(timestamp: number): void {
    this.zoomAnimationFrame = null;
    const elapsed = this.zoomAnimationTimestamp === null
      ? 16
      : Math.max(1, Math.min(timestamp - this.zoomAnimationTimestamp, 50));
    this.zoomAnimationTimestamp = timestamp;

    const progress = 1 - Math.exp(-elapsed / ZOOM_SMOOTHING_TIME_MS);
    const nextZoom = this.zoom + (this.zoomTarget - this.zoom) * progress;
    const settled = Math.abs(this.zoomTarget - nextZoom) <= ZOOM_SETTLE_EPSILON;
    this.zoom = settled ? this.zoomTarget : nextZoom;
    if (settled) {
      this.zoomAnimating = false;
      this.zoomAnimationTimestamp = null;
    }
    this.eventBus.emit('zoom-changed', this.zoom);

    if (!settled) {
      this.zoomAnimationFrame = requestAnimationFrame(this.onZoomAnimationFrameBound);
    }
  }

  private cancelZoomAnimation(): void {
    if (this.zoomAnimationFrame !== null) {
      cancelAnimationFrame(this.zoomAnimationFrame);
      this.zoomAnimationFrame = null;
    }
    this.zoomAnimationTimestamp = null;
    this.zoomAnimating = false;
    this.zoomTarget = this.zoom;
  }

  private clampZoom(zoom: number): number {
    return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom));
  }

  setScrollTop(y: number): void {
    if (this.container) {
      this.container.scrollTop = y;
      this.scrollY = this.container.scrollTop;
    }
  }
}
