export class RequestMap {
    pending = new Map();
    get size() {
        return this.pending.size;
    }
    has(id) {
        return this.pending.has(id);
    }
    set(id, request) {
        this.pending.set(id, request);
    }
    delete(id) {
        return this.pending.delete(id);
    }
    resolve(id, value) {
        const request = this.pending.get(id);
        if (!request) {
            return false;
        }
        this.pending.delete(id);
        request.resolve(value);
        return true;
    }
    reject(id, error) {
        const request = this.pending.get(id);
        if (!request) {
            return false;
        }
        this.pending.delete(id);
        request.reject(error);
        return true;
    }
    rejectAll(error) {
        for (const request of this.pending.values()) {
            request.reject(error);
        }
        this.pending.clear();
    }
    clear() {
        this.pending.clear();
    }
}
//# sourceMappingURL=request-map.js.map