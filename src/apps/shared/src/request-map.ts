export interface PendingRequest {
  resolve(value: unknown): void;
  reject(error: unknown): void;
}

export class RequestMap {
  private readonly pending = new Map<string, PendingRequest>();

  get size(): number {
    return this.pending.size;
  }

  has(id: string): boolean {
    return this.pending.has(id);
  }

  set(id: string, request: PendingRequest): void {
    this.pending.set(id, request);
  }

  delete(id: string): boolean {
    return this.pending.delete(id);
  }

  resolve(id: string, value: unknown): boolean {
    const request = this.pending.get(id);
    if (!request) {
      return false;
    }

    this.pending.delete(id);
    request.resolve(value);
    return true;
  }

  reject(id: string, error: unknown): boolean {
    const request = this.pending.get(id);
    if (!request) {
      return false;
    }

    this.pending.delete(id);
    request.reject(error);
    return true;
  }

  rejectAll(error: unknown): void {
    for (const request of this.pending.values()) {
      request.reject(error);
    }
    this.pending.clear();
  }

  clear(): void {
    this.pending.clear();
  }
}
