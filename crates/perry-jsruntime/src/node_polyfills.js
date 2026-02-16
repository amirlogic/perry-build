// Node.js global polyfills for V8 runtime
// These are injected before any modules are loaded

// Buffer polyfill using TextEncoder/TextDecoder
(function() {
    if (typeof globalThis.Buffer !== 'undefined') return;

    class Buffer extends Uint8Array {
        static alloc(size, fill, encoding) {
            const buf = new Buffer(size);
            if (fill !== undefined) {
                if (typeof fill === 'number') {
                    buf.fill(fill);
                } else if (typeof fill === 'string') {
                    const encoded = new TextEncoder().encode(fill);
                    for (let i = 0; i < size; i++) {
                        buf[i] = encoded[i % encoded.length];
                    }
                }
            }
            return buf;
        }

        static allocUnsafe(size) {
            return new Buffer(size);
        }

        static from(data, encodingOrOffset, length) {
            if (typeof data === 'string') {
                const encoding = encodingOrOffset || 'utf8';
                if (encoding === 'hex') {
                    const bytes = new Uint8Array(data.length / 2);
                    for (let i = 0; i < data.length; i += 2) {
                        bytes[i / 2] = parseInt(data.substr(i, 2), 16);
                    }
                    return new Buffer(bytes.buffer);
                }
                if (encoding === 'base64') {
                    const binary = atob(data);
                    const bytes = new Uint8Array(binary.length);
                    for (let i = 0; i < binary.length; i++) {
                        bytes[i] = binary.charCodeAt(i);
                    }
                    return new Buffer(bytes.buffer);
                }
                // utf8 / utf-8 / ascii / latin1
                const encoded = new TextEncoder().encode(data);
                return new Buffer(encoded.buffer, encoded.byteOffset, encoded.byteLength);
            }
            if (data instanceof ArrayBuffer) {
                return new Buffer(data, encodingOrOffset || 0, length !== undefined ? length : data.byteLength);
            }
            if (ArrayBuffer.isView(data)) {
                return new Buffer(data.buffer, data.byteOffset, data.byteLength);
            }
            if (Array.isArray(data)) {
                return new Buffer(new Uint8Array(data).buffer);
            }
            // Buffer.from(buffer) - copy
            if (data instanceof Buffer || data instanceof Uint8Array) {
                const copy = new Uint8Array(data);
                return new Buffer(copy.buffer, copy.byteOffset, copy.byteLength);
            }
            return new Buffer(0);
        }

        static isBuffer(obj) {
            return obj instanceof Buffer;
        }

        static isEncoding(encoding) {
            return ['utf8', 'utf-8', 'ascii', 'latin1', 'binary', 'hex', 'base64', 'ucs2', 'ucs-2', 'utf16le', 'utf-16le'].includes(encoding?.toLowerCase());
        }

        static concat(list, totalLength) {
            if (totalLength === undefined) {
                totalLength = list.reduce((acc, buf) => acc + buf.length, 0);
            }
            const result = Buffer.alloc(totalLength);
            let offset = 0;
            for (const buf of list) {
                result.set(buf, offset);
                offset += buf.length;
                if (offset >= totalLength) break;
            }
            return result;
        }

        static byteLength(string, encoding) {
            if (typeof string !== 'string') return string.length;
            return new TextEncoder().encode(string).length;
        }

        static compare(a, b) {
            const len = Math.min(a.length, b.length);
            for (let i = 0; i < len; i++) {
                if (a[i] < b[i]) return -1;
                if (a[i] > b[i]) return 1;
            }
            if (a.length < b.length) return -1;
            if (a.length > b.length) return 1;
            return 0;
        }

        toString(encoding, start, end) {
            const slice = this.subarray(start || 0, end || this.length);
            encoding = encoding || 'utf8';
            if (encoding === 'hex') {
                let hex = '';
                for (let i = 0; i < slice.length; i++) {
                    hex += slice[i].toString(16).padStart(2, '0');
                }
                return hex;
            }
            if (encoding === 'base64') {
                let binary = '';
                for (let i = 0; i < slice.length; i++) {
                    binary += String.fromCharCode(slice[i]);
                }
                return btoa(binary);
            }
            // utf8 / utf-8 / ascii / latin1
            return new TextDecoder().decode(slice);
        }

        write(string, offset, length, encoding) {
            offset = offset || 0;
            const encoded = new TextEncoder().encode(string);
            const len = Math.min(encoded.length, length !== undefined ? length : this.length - offset);
            this.set(encoded.subarray(0, len), offset);
            return len;
        }

        copy(target, targetStart, sourceStart, sourceEnd) {
            targetStart = targetStart || 0;
            sourceStart = sourceStart || 0;
            sourceEnd = sourceEnd || this.length;
            const slice = this.subarray(sourceStart, sourceEnd);
            target.set(slice, targetStart);
            return slice.length;
        }

        equals(other) {
            return Buffer.compare(this, other) === 0;
        }

        compare(other) {
            return Buffer.compare(this, other);
        }

        slice(start, end) {
            const sliced = super.subarray(start, end);
            return new Buffer(sliced.buffer, sliced.byteOffset, sliced.byteLength);
        }

        subarray(start, end) {
            const sliced = super.subarray(start, end);
            return new Buffer(sliced.buffer, sliced.byteOffset, sliced.byteLength);
        }

        readUInt8(offset) { return this[offset]; }
        readUInt16BE(offset) { return (this[offset] << 8) | this[offset + 1]; }
        readUInt16LE(offset) { return this[offset] | (this[offset + 1] << 8); }
        readUInt32BE(offset) { return ((this[offset] << 24) | (this[offset + 1] << 16) | (this[offset + 2] << 8) | this[offset + 3]) >>> 0; }
        readUInt32LE(offset) { return ((this[offset + 3] << 24) | (this[offset + 2] << 16) | (this[offset + 1] << 8) | this[offset]) >>> 0; }
        readInt8(offset) { const v = this[offset]; return v > 127 ? v - 256 : v; }
        readInt16BE(offset) { const v = this.readUInt16BE(offset); return v > 32767 ? v - 65536 : v; }
        readInt16LE(offset) { const v = this.readUInt16LE(offset); return v > 32767 ? v - 65536 : v; }
        readInt32BE(offset) { return (this[offset] << 24) | (this[offset + 1] << 16) | (this[offset + 2] << 8) | this[offset + 3]; }
        readInt32LE(offset) { return (this[offset + 3] << 24) | (this[offset + 2] << 16) | (this[offset + 1] << 8) | this[offset]; }

        readBigUInt64BE(offset) {
            const hi = BigInt(this.readUInt32BE(offset));
            const lo = BigInt(this.readUInt32BE(offset + 4));
            return (hi << 32n) | lo;
        }
        readBigUInt64LE(offset) {
            const lo = BigInt(this.readUInt32LE(offset));
            const hi = BigInt(this.readUInt32LE(offset + 4));
            return (hi << 32n) | lo;
        }

        writeUInt8(value, offset) { this[offset] = value & 0xff; return offset + 1; }
        writeUInt16BE(value, offset) { this[offset] = (value >> 8) & 0xff; this[offset + 1] = value & 0xff; return offset + 2; }
        writeUInt16LE(value, offset) { this[offset] = value & 0xff; this[offset + 1] = (value >> 8) & 0xff; return offset + 2; }
        writeUInt32BE(value, offset) { this[offset] = (value >> 24) & 0xff; this[offset + 1] = (value >> 16) & 0xff; this[offset + 2] = (value >> 8) & 0xff; this[offset + 3] = value & 0xff; return offset + 4; }
        writeUInt32LE(value, offset) { this[offset] = value & 0xff; this[offset + 1] = (value >> 8) & 0xff; this[offset + 2] = (value >> 16) & 0xff; this[offset + 3] = (value >> 24) & 0xff; return offset + 4; }

        toJSON() {
            return { type: 'Buffer', data: Array.from(this) };
        }

        get offset() { return this.byteOffset; }
    }

    globalThis.Buffer = Buffer;

    // TextEncoder/TextDecoder polyfill (needed by ethers.js fetch response handling)
    if (typeof globalThis.TextEncoder === 'undefined') {
        globalThis.TextEncoder = class TextEncoder {
            encode(str) {
                const buf = [];
                for (let i = 0; i < str.length; i++) {
                    let c = str.charCodeAt(i);
                    if (c < 0x80) {
                        buf.push(c);
                    } else if (c < 0x800) {
                        buf.push(0xC0 | (c >> 6), 0x80 | (c & 0x3F));
                    } else if (c < 0xD800 || c >= 0xE000) {
                        buf.push(0xE0 | (c >> 12), 0x80 | ((c >> 6) & 0x3F), 0x80 | (c & 0x3F));
                    } else {
                        i++;
                        c = 0x10000 + (((c & 0x3FF) << 10) | (str.charCodeAt(i) & 0x3FF));
                        buf.push(0xF0 | (c >> 18), 0x80 | ((c >> 12) & 0x3F), 0x80 | ((c >> 6) & 0x3F), 0x80 | (c & 0x3F));
                    }
                }
                return new Uint8Array(buf);
            }
        };
    }
    if (typeof globalThis.TextDecoder === 'undefined') {
        globalThis.TextDecoder = class TextDecoder {
            decode(buf) {
                if (!buf) return '';
                const bytes = new Uint8Array(buf.buffer || buf);
                let str = '';
                for (let i = 0; i < bytes.length; i++) {
                    str += String.fromCharCode(bytes[i]);
                }
                return str;
            }
        };
    }

    // Global object aliases (needed by ethers.js crypto-browser.js getGlobal())
    if (typeof globalThis.self === 'undefined') globalThis.self = globalThis;
    if (typeof globalThis.global === 'undefined') globalThis.global = globalThis;

    // Web Crypto API polyfill (needed by ethers.js crypto-browser.js)
    if (typeof globalThis.crypto === 'undefined') {
        globalThis.crypto = {
            getRandomValues(arr) {
                for (let i = 0; i < arr.length; i++) {
                    arr[i] = Math.floor(Math.random() * 256);
                }
                return arr;
            },
            subtle: {}
        };
    }

    // Timer globals using microtasks (no real event loop, but callbacks fire via Promise)
    let __timerId = 1;
    if (typeof globalThis.setTimeout === 'undefined') {
        globalThis.setTimeout = function(fn, delay, ...args) {
            const id = __timerId++;
            Promise.resolve().then(() => fn(...args));
            return id;
        };
        globalThis.clearTimeout = function(id) {};
        globalThis.setInterval = function(fn, delay) { return __timerId++; };
        globalThis.clearInterval = function(id) {};
    }

    // fetch() polyfill using op_perry_fetch Deno op
    if (typeof globalThis.fetch === 'undefined') {
        const core = Deno.core;
        globalThis.fetch = async function(input, init) {
            const url = typeof input === 'string' ? input : input.url;
            const method = (init && init.method) || 'GET';
            let body = (init && init.body) || '';
            // Convert Uint8Array/ArrayBuffer body to string (ethers.js sends Uint8Array)
            if (body && typeof body !== 'string') {
                if (body instanceof Uint8Array || body instanceof ArrayBuffer) {
                    const bytes = body instanceof ArrayBuffer ? new Uint8Array(body) : body;
                    body = new TextDecoder().decode(bytes);
                } else {
                    body = JSON.stringify(body);
                }
            }
            const headers = {};
            if (init && init.headers) {
                if (init.headers instanceof Headers) {
                    init.headers.forEach((v, k) => { headers[k] = v; });
                } else if (typeof init.headers === 'object') {
                    Object.assign(headers, init.headers);
                }
            }
            const result = core.ops.op_perry_fetch(url, method, body, headers);
            return {
                ok: result.status >= 200 && result.status < 300,
                status: result.status,
                statusText: result.statusText,
                headers: new Headers(result.headers),
                text: async () => result.body,
                json: async () => JSON.parse(result.body),
                arrayBuffer: async () => new TextEncoder().encode(result.body).buffer,
            };
        };
        // Headers polyfill if needed
        if (typeof globalThis.Headers === 'undefined') {
            globalThis.Headers = class Headers {
                constructor(init) {
                    this._map = {};
                    if (Array.isArray(init)) {
                        for (const [k, v] of init) {
                            this._map[k.toLowerCase()] = String(v);
                        }
                    } else if (init && typeof init === 'object') {
                        for (const [k, v] of Object.entries(init)) {
                            this._map[k.toLowerCase()] = String(v);
                        }
                    }
                }
                get(name) { return this._map[name.toLowerCase()] || null; }
                set(name, value) { this._map[name.toLowerCase()] = value; }
                has(name) { return name.toLowerCase() in this._map; }
                delete(name) { delete this._map[name.toLowerCase()]; }
                forEach(cb) { for (const [k, v] of Object.entries(this._map)) cb(v, k, this); }
                entries() { return Object.entries(this._map)[Symbol.iterator](); }
                keys() { return Object.keys(this._map)[Symbol.iterator](); }
                values() { return Object.values(this._map)[Symbol.iterator](); }
            };
        }
    }

    // AbortController polyfill
    if (typeof globalThis.AbortController === 'undefined') {
        globalThis.AbortController = class AbortController {
            constructor() {
                this.signal = { aborted: false, reason: undefined, addEventListener: () => {}, removeEventListener: () => {} };
            }
            abort(reason) {
                this.signal.aborted = true;
                this.signal.reason = reason || new Error('AbortError');
            }
        };
    }

    // Also provide process.env and process.version if not present
    if (typeof globalThis.process === 'undefined') {
        globalThis.process = {
            env: {},
            version: 'v20.0.0',
            versions: { node: '20.0.0' },
            platform: 'darwin',
            arch: 'arm64',
            pid: 0,
            cwd: () => '/',
            nextTick: (fn, ...args) => Promise.resolve().then(() => fn(...args)),
            stdout: { write: (s) => {} },
            stderr: { write: (s) => {} },
        };
    }
})();
