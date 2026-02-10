export interface PendingRequest {
    resolve(value: unknown): void;
    reject(error: unknown): void;
}
export declare class RequestMap {
    private readonly pending;
    get size(): number;
    has(id: string): boolean;
    set(id: string, request: PendingRequest): void;
    delete(id: string): boolean;
    resolve(id: string, value: unknown): boolean;
    reject(id: string, error: unknown): boolean;
    rejectAll(error: unknown): void;
    clear(): void;
}
//# sourceMappingURL=request-map.d.ts.map