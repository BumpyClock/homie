export const PROTOCOL_VERSION = 1;
function isObjectRecord(value) {
    return typeof value === "object" && value !== null;
}
export function isHandshakeResponse(value) {
    if (!isObjectRecord(value)) {
        return false;
    }
    if (value.type === "hello") {
        return typeof value.protocol_version === "number" && typeof value.server_id === "string";
    }
    if (value.type === "reject") {
        return typeof value.code === "string" && typeof value.reason === "string";
    }
    return false;
}
export function isRpcResponse(value) {
    if (!isObjectRecord(value) || value.type !== "response") {
        return false;
    }
    return typeof value.id === "string";
}
export function isRpcEvent(value) {
    if (!isObjectRecord(value) || value.type !== "event") {
        return false;
    }
    return typeof value.topic === "string";
}
//# sourceMappingURL=protocol.js.map