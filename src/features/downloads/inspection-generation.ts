export class InspectionGeneration {
  private current = 0;

  start() {
    this.current += 1;
    return this.current;
  }

  invalidate() {
    this.current += 1;
  }

  isCurrent(generation: number) {
    return this.current === generation;
  }
}
