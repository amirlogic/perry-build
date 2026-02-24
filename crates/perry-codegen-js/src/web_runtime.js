// Perry Web Runtime - maps perry/ui widgets to DOM elements
// This file is embedded via include_str! and injected into HTML output.

(function() {
"use strict";

// --- Handle System ---
// Widget handles are wrapper objects with methods that delegate to DOM elements.
// State handles are objects with .value getter/setter and methods.

const handles = new Map();   // handle int → DOM element
const states = new Map();    // handle int → { _value, subscribers[] }
let nextHandle = 1;

function allocHandle(el) {
    const h = nextHandle++;
    handles.set(h, el);
    return h;
}

function getHandle(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return handles.get(h._perryHandle);
    return handles.get(h);
}

function getHandleId(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return h._perryHandle;
    return h;
}

// Create a widget wrapper object with all perry/ui methods
function wrapWidget(h) {
    const w = {
        _perryHandle: h,
        addChild(child) { perry_ui_widget_add_child(h, getHandleId(child)); },
        removeAllChildren() { perry_ui_widget_remove_all_children(h); },
        setBackground(r, g, b, a) { perry_ui_set_background(h, r, g, b, a); },
        setForeground(r, g, b, a) { perry_ui_set_foreground(h, r, g, b, a); },
        setFontSize(size) { perry_ui_set_font_size(h, size); },
        setFontWeight(weight) { perry_ui_set_font_weight(h, weight); },
        setFontFamily(family) { perry_ui_set_font_family(h, family); },
        setPadding(val) { perry_ui_set_padding(h, val); },
        setFrame(w, ht) { perry_ui_set_frame(h, w, ht); },
        setCornerRadius(r) { perry_ui_set_corner_radius(h, r); },
        setBorder(w, r, g, b, a) { perry_ui_set_border(h, w, r, g, b, a); },
        setOpacity(o) { perry_ui_set_opacity(h, o); },
        setEnabled(e) { perry_ui_set_enabled(h, e); },
        setTooltip(t) { perry_ui_set_tooltip(h, t); },
        setControlSize(s) { perry_ui_set_control_size(h, s); },
        animateOpacity(from, to, dur) { perry_ui_animate_opacity(h, from, to, dur); },
        animatePosition(fx, fy, tx, ty, dur) { perry_ui_animate_position(h, fx, fy, tx, ty, dur); },
        setOnClick(cb) { perry_ui_set_on_click(h, cb); },
        setOnHover(cb) { perry_ui_set_on_hover(h, cb); },
        setOnDoubleClick(cb) { perry_ui_set_on_double_click(h, cb); },
        run() { perry_ui_app_run(); },
        // Canvas methods
        fillRect(x, y, w, ht) { perry_ui_canvas_fill_rect(h, x, y, w, ht); },
        strokeRect(x, y, w, ht) { perry_ui_canvas_stroke_rect(h, x, y, w, ht); },
        clearRect(x, y, w, ht) { perry_ui_canvas_clear_rect(h, x, y, w, ht); },
        setFillColor(r, g, b, a) { perry_ui_canvas_set_fill_color(h, r, g, b, a); },
        setStrokeColor(r, g, b, a) { perry_ui_canvas_set_stroke_color(h, r, g, b, a); },
        beginPath() { perry_ui_canvas_begin_path(h); },
        moveTo(x, y) { perry_ui_canvas_move_to(h, x, y); },
        lineTo(x, y) { perry_ui_canvas_line_to(h, x, y); },
        arc(x, y, r, sa, ea) { perry_ui_canvas_arc(h, x, y, r, sa, ea); },
        closePath() { perry_ui_canvas_close_path(h); },
        fill() { perry_ui_canvas_fill(h); },
        stroke() { perry_ui_canvas_stroke(h); },
        setLineWidth(w) { perry_ui_canvas_set_line_width(h, w); },
        fillText(t, x, y) { perry_ui_canvas_fill_text(h, t, x, y); },
        setFont(f) { perry_ui_canvas_set_font(h, f); },
    };
    return w;
}

// --- State Reactive System ---
function stateCreate(initialValue) {
    const h = nextHandle++;
    const sObj = { _value: initialValue, subscribers: [] };
    states.set(h, sObj);
    // Return a state wrapper with .value getter/setter and methods
    const wrapper = {
        _perryHandle: h,
        _perryState: true,
        get value() { return sObj._value; },
        set value(v) { stateSet(h, v); },
        get() { return sObj._value; },
        set(v) { stateSet(h, v); },
        bindText(widget) { perry_ui_state_bind_text(h, getHandleId(widget)); },
        bindTextNumeric(widget) { perry_ui_state_bind_text_numeric(h, getHandleId(widget)); },
        bindSlider(widget) { perry_ui_state_bind_slider(h, getHandleId(widget)); },
        bindToggle(widget) { perry_ui_state_bind_toggle(h, getHandleId(widget)); },
        bindVisibility(widget) { perry_ui_state_bind_visibility(h, getHandleId(widget)); },
        bindForEach(parent, fn) { perry_ui_state_bind_foreach(h, getHandleId(parent), fn); },
        onChange(cb) { perry_ui_state_on_change(h, cb); },
    };
    return wrapper;
}

function stateGet(h) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    return s ? s._value : undefined;
}

function stateSet(h, value) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    if (!s) return;
    s._value = value;
    for (const sub of s.subscribers) {
        try { sub(value); } catch(e) { console.error("State subscriber error:", e); }
    }
}

function stateSubscribe(h, fn) {
    const hId = getHandleId(h);
    const s = states.get(hId);
    if (s) s.subscribers.push(fn);
}

// --- CSS Reset ---
const style = document.createElement("style");
style.textContent = `
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif; }
#perry-root { display: flex; flex-direction: column; min-height: 100vh; }
button { cursor: pointer; padding: 6px 16px; border: 1px solid #ccc; border-radius: 6px; background: #fff; font: inherit; }
button:hover { background: #f0f0f0; }
button:active { background: #e0e0e0; }
input[type="text"], input[type="password"], select, textarea { padding: 6px 10px; border: 1px solid #ccc; border-radius: 6px; font: inherit; }
input[type="range"] { width: 100%; }
hr { border: none; border-top: 1px solid #ddd; margin: 4px 0; }
fieldset { border: 1px solid #ddd; border-radius: 8px; padding: 12px; }
legend { font-weight: 600; padding: 0 6px; }
progress { width: 100%; }
`;
document.head.appendChild(style);

// --- Root ---
let perryRoot = null;
function getRoot() {
    if (!perryRoot) {
        perryRoot = document.getElementById("perry-root");
        if (!perryRoot) {
            perryRoot = document.createElement("div");
            perryRoot.id = "perry-root";
            document.body.appendChild(perryRoot);
        }
    }
    return perryRoot;
}

// --- Widget Creation ---
function perry_ui_app_create(title, width, height) {
    document.title = title;
    const root = getRoot();
    root.style.maxWidth = width + "px";
    root.style.margin = "0 auto";
    root.style.padding = "16px";
    root.style.minHeight = height + "px";
    return wrapWidget(allocHandle(root));
}

function perry_ui_vstack_create(spacing) {
    const el = document.createElement("div");
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = spacing + "px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_hstack_create(spacing) {
    const el = document.createElement("div");
    el.style.display = "flex";
    el.style.flexDirection = "row";
    el.style.gap = spacing + "px";
    el.style.alignItems = "center";
    return wrapWidget(allocHandle(el));
}

function perry_ui_zstack_create() {
    const el = document.createElement("div");
    el.style.position = "relative";
    return wrapWidget(allocHandle(el));
}

function perry_ui_text_create(text) {
    const el = document.createElement("span");
    el.textContent = text;
    return wrapWidget(allocHandle(el));
}

function perry_ui_button_create(label, callback) {
    const el = document.createElement("button");
    el.textContent = label;
    if (typeof callback === "function") {
        el.addEventListener("click", callback);
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_textfield_create(placeholder, callback) {
    const el = document.createElement("input");
    el.type = "text";
    el.placeholder = placeholder || "";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(el.value));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_securefield_create(placeholder, callback) {
    const el = document.createElement("input");
    el.type = "password";
    el.placeholder = placeholder || "";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(el.value));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_toggle_create(label, callback) {
    const wrapper = document.createElement("label");
    wrapper.style.display = "flex";
    wrapper.style.alignItems = "center";
    wrapper.style.gap = "8px";
    wrapper.style.cursor = "pointer";
    const input = document.createElement("input");
    input.type = "checkbox";
    wrapper.appendChild(input);
    wrapper.appendChild(document.createTextNode(label || ""));
    if (typeof callback === "function") {
        input.addEventListener("change", () => callback(input.checked ? 1.0 : 0.0));
    }
    wrapper._input = input;
    return wrapWidget(allocHandle(wrapper));
}

function perry_ui_slider_create(min, max, initial, callback) {
    const el = document.createElement("input");
    el.type = "range";
    el.min = min;
    el.max = max;
    el.value = initial;
    el.step = "any";
    if (typeof callback === "function") {
        el.addEventListener("input", () => callback(parseFloat(el.value)));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_scrollview_create() {
    const el = document.createElement("div");
    el.style.overflow = "auto";
    el.style.flex = "1";
    return wrapWidget(allocHandle(el));
}

function perry_ui_spacer_create() {
    const el = document.createElement("div");
    el.style.flex = "1";
    return wrapWidget(allocHandle(el));
}

function perry_ui_divider_create() {
    const el = document.createElement("hr");
    return wrapWidget(allocHandle(el));
}

function perry_ui_progressview_create(value) {
    const el = document.createElement("progress");
    el.max = 1;
    el.value = (value != null) ? value : 0;
    return wrapWidget(allocHandle(el));
}

function perry_ui_image_create(src, width, height) {
    const el = document.createElement("img");
    el.src = src || "";
    if (width > 0) el.style.width = width + "px";
    if (height > 0) el.style.height = height + "px";
    el.style.objectFit = "contain";
    return wrapWidget(allocHandle(el));
}

function perry_ui_picker_create(items_json, selected, callback) {
    const el = document.createElement("select");
    let items = [];
    try { items = JSON.parse(items_json); } catch(e) {}
    for (let i = 0; i < items.length; i++) {
        const opt = document.createElement("option");
        opt.value = i;
        opt.textContent = items[i];
        if (i === selected) opt.selected = true;
        el.appendChild(opt);
    }
    if (typeof callback === "function") {
        el.addEventListener("change", () => callback(parseInt(el.value)));
    }
    return wrapWidget(allocHandle(el));
}

function perry_ui_form_create() {
    const el = document.createElement("form");
    el.addEventListener("submit", e => e.preventDefault());
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = "8px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_section_create(title) {
    const el = document.createElement("fieldset");
    if (title) {
        const legend = document.createElement("legend");
        legend.textContent = title;
        el.appendChild(legend);
    }
    el.style.display = "flex";
    el.style.flexDirection = "column";
    el.style.gap = "6px";
    return wrapWidget(allocHandle(el));
}

function perry_ui_navigationstack_create() {
    const el = document.createElement("div");
    el._navStack = [];
    return wrapWidget(allocHandle(el));
}

function perry_ui_canvas_create(width, height) {
    const el = document.createElement("canvas");
    el.width = width;
    el.height = height;
    el._ctx = el.getContext("2d");
    return wrapWidget(allocHandle(el));
}

// --- Child Management ---
function perry_ui_widget_add_child(parent_h, child_h) {
    const parent = getHandle(parent_h);
    const child = getHandle(child_h);
    if (parent && child) parent.appendChild(child);
}

function perry_ui_widget_remove_all_children(h) {
    const el = getHandle(h);
    if (el) {
        while (el.lastChild && el.lastChild.tagName !== "LEGEND") {
            el.removeChild(el.lastChild);
        }
    }
}

// Resolve handle-or-wrapper to int for internal use
function resolveHandle(h) {
    if (typeof h === "object" && h !== null && h._perryHandle) return h._perryHandle;
    return h;
}

// --- Styling ---
function perry_ui_set_background(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.backgroundColor = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_foreground(h, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.color = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_font_size(h, size) {
    const el = getHandle(h);
    if (el) el.style.fontSize = size + "px";
}

function perry_ui_set_font_weight(h, weight) {
    const el = getHandle(h);
    if (el) el.style.fontWeight = weight === 1 ? "bold" : "normal";
}

function perry_ui_set_font_family(h, family) {
    const el = getHandle(h);
    if (el) el.style.fontFamily = family;
}

function perry_ui_set_padding(h, value) {
    const el = getHandle(h);
    if (el) el.style.padding = value + "px";
}

function perry_ui_set_frame(h, width, height) {
    const el = getHandle(h);
    if (el) {
        if (width > 0) el.style.width = width + "px";
        if (height > 0) el.style.height = height + "px";
    }
}

function perry_ui_set_corner_radius(h, radius) {
    const el = getHandle(h);
    if (el) el.style.borderRadius = radius + "px";
}

function perry_ui_set_border(h, width, r, g, b, a) {
    const el = getHandle(h);
    if (el) el.style.border = `${width}px solid rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_set_opacity(h, opacity) {
    const el = getHandle(h);
    if (el) el.style.opacity = opacity;
}

function perry_ui_set_enabled(h, enabled) {
    const el = getHandle(h);
    if (el) {
        el.disabled = !enabled;
        el.style.opacity = enabled ? "1" : "0.5";
        el.style.pointerEvents = enabled ? "auto" : "none";
    }
}

function perry_ui_set_tooltip(h, text) {
    const el = getHandle(h);
    if (el) el.title = text;
}

function perry_ui_set_control_size(h, size) {
    const el = getHandle(h);
    if (!el) return;
    const scale = size === 0 ? 0.85 : size === 2 ? 1.2 : 1.0;
    el.style.fontSize = (scale * 100) + "%";
}

// --- Animations ---
function perry_ui_animate_opacity(h, from, to, duration) {
    const el = getHandle(h);
    if (!el) return;
    el.style.opacity = from;
    el.style.transition = `opacity ${duration}s ease`;
    requestAnimationFrame(() => { el.style.opacity = to; });
}

function perry_ui_animate_position(h, fromX, fromY, toX, toY, duration) {
    const el = getHandle(h);
    if (!el) return;
    el.style.position = "relative";
    el.style.left = fromX + "px";
    el.style.top = fromY + "px";
    el.style.transition = `left ${duration}s ease, top ${duration}s ease`;
    requestAnimationFrame(() => { el.style.left = toX + "px"; el.style.top = toY + "px"; });
}

// --- Event Handlers ---
function perry_ui_set_on_click(h, callback) {
    const el = getHandle(h);
    if (el && typeof callback === "function") el.addEventListener("click", callback);
}

function perry_ui_set_on_hover(h, callback) {
    const el = getHandle(h);
    if (!el || typeof callback !== "function") return;
    el.addEventListener("mouseenter", () => callback(1));
    el.addEventListener("mouseleave", () => callback(0));
}

function perry_ui_set_on_double_click(h, callback) {
    const el = getHandle(h);
    if (el && typeof callback === "function") el.addEventListener("dblclick", callback);
}

// --- State Bindings ---
function perry_ui_state_bind_text(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    stateSubscribe(stateH, (v) => { el.textContent = String(v); });
    el.textContent = String(stateGet(stateH));
}

function perry_ui_state_bind_text_numeric(stateH, widgetH) {
    perry_ui_state_bind_text(stateH, widgetH);
}

function perry_ui_state_bind_slider(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    stateSubscribe(stateH, (v) => { el.value = v; });
    el.value = stateGet(stateH);
}

function perry_ui_state_bind_toggle(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    const input = el._input || el.querySelector("input[type=checkbox]");
    if (!input) return;
    stateSubscribe(stateH, (v) => { input.checked = !!v; });
    input.checked = !!stateGet(stateH);
}

function perry_ui_state_bind_visibility(stateH, widgetH) {
    const el = getHandle(widgetH);
    if (!el) return;
    function update(v) { el.style.display = v ? "" : "none"; }
    stateSubscribe(stateH, update);
    update(stateGet(stateH));
}

function perry_ui_state_bind_foreach(stateH, parentH, templateFn) {
    const parent = getHandle(parentH);
    if (!parent || typeof templateFn !== "function") return;
    function update(items) {
        perry_ui_widget_remove_all_children(parentH);
        if (Array.isArray(items)) {
            for (let i = 0; i < items.length; i++) {
                templateFn(items[i], i);
            }
        }
    }
    stateSubscribe(stateH, update);
    update(stateGet(stateH));
}

function perry_ui_state_on_change(stateH, callback) {
    if (typeof callback === "function") {
        stateSubscribe(stateH, callback);
    }
}

// --- System APIs ---
function perry_system_open_url(url) {
    window.open(url, "_blank");
}

function perry_system_is_dark_mode() {
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? 1.0 : 0.0;
}

function perry_system_preferences_get(key) {
    return localStorage.getItem(key) || "";
}

function perry_system_preferences_set(key, value) {
    localStorage.setItem(key, value);
}

// --- Canvas Operations ---
function perry_ui_canvas_fill_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillRect(x, y, w, ht);
}

function perry_ui_canvas_stroke_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.strokeRect(x, y, w, ht);
}

function perry_ui_canvas_clear_rect(h, x, y, w, ht) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.clearRect(x, y, w, ht);
}

function perry_ui_canvas_set_fill_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillStyle = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_canvas_set_stroke_color(h, r, g, b, a) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.strokeStyle = `rgba(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)},${a})`;
}

function perry_ui_canvas_begin_path(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.beginPath();
}

function perry_ui_canvas_move_to(h, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.moveTo(x, y);
}

function perry_ui_canvas_line_to(h, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.lineTo(x, y);
}

function perry_ui_canvas_arc(h, x, y, radius, startAngle, endAngle) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.arc(x, y, radius, startAngle, endAngle);
}

function perry_ui_canvas_close_path(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.closePath();
}

function perry_ui_canvas_fill(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fill();
}

function perry_ui_canvas_stroke(h) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.stroke();
}

function perry_ui_canvas_set_line_width(h, w) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.lineWidth = w;
}

function perry_ui_canvas_fill_text(h, text, x, y) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.fillText(text, x, y);
}

function perry_ui_canvas_set_font(h, font) {
    const el = getHandle(h);
    if (el && el._ctx) el._ctx.font = font;
}

// --- Run App ---
function perry_ui_app_run() {
    // In browser, the app is already "running" once DOM is ready.
    // This is a no-op.
}

// --- Timer Functions ---
function perry_set_timeout(callback, ms) {
    return setTimeout(callback, ms);
}

function perry_set_interval(callback, ms) {
    return setInterval(callback, ms);
}

function perry_clear_timeout(id) {
    clearTimeout(id);
}

function perry_clear_interval(id) {
    clearInterval(id);
}

// --- Path Helpers (simplified browser versions) ---
const __path = {
    join: function(...parts) {
        return parts.join("/").replace(/\/+/g, "/");
    },
    dirname: function(p) {
        const i = p.lastIndexOf("/");
        return i >= 0 ? p.substring(0, i) : ".";
    },
    basename: function(p) {
        const i = p.lastIndexOf("/");
        return i >= 0 ? p.substring(i + 1) : p;
    },
    extname: function(p) {
        const b = __path.basename(p);
        const i = b.lastIndexOf(".");
        return i > 0 ? b.substring(i) : "";
    },
    resolve: function(...parts) {
        return __path.join(...parts);
    },
    isAbsolute: function(p) {
        return p.startsWith("/");
    }
};

// --- Expose API ---
window.__perry = {
    // Handle system
    allocHandle, getHandle,
    // State
    stateCreate, stateGet, stateSet, stateSubscribe,
    // UI widgets
    perry_ui_app_create,
    perry_ui_vstack_create,
    perry_ui_hstack_create,
    perry_ui_zstack_create,
    perry_ui_text_create,
    perry_ui_button_create,
    perry_ui_textfield_create,
    perry_ui_securefield_create,
    perry_ui_toggle_create,
    perry_ui_slider_create,
    perry_ui_scrollview_create,
    perry_ui_spacer_create,
    perry_ui_divider_create,
    perry_ui_progressview_create,
    perry_ui_image_create,
    perry_ui_picker_create,
    perry_ui_form_create,
    perry_ui_section_create,
    perry_ui_navigationstack_create,
    perry_ui_canvas_create,
    // Child management
    perry_ui_widget_add_child,
    perry_ui_widget_remove_all_children,
    // Styling
    perry_ui_set_background,
    perry_ui_set_foreground,
    perry_ui_set_font_size,
    perry_ui_set_font_weight,
    perry_ui_set_font_family,
    perry_ui_set_padding,
    perry_ui_set_frame,
    perry_ui_set_corner_radius,
    perry_ui_set_border,
    perry_ui_set_opacity,
    perry_ui_set_enabled,
    perry_ui_set_tooltip,
    perry_ui_set_control_size,
    // Animations
    perry_ui_animate_opacity,
    perry_ui_animate_position,
    // Events
    perry_ui_set_on_click,
    perry_ui_set_on_hover,
    perry_ui_set_on_double_click,
    // State bindings
    perry_ui_state_bind_text,
    perry_ui_state_bind_text_numeric,
    perry_ui_state_bind_slider,
    perry_ui_state_bind_toggle,
    perry_ui_state_bind_visibility,
    perry_ui_state_bind_foreach,
    perry_ui_state_on_change,
    // System
    perry_system_open_url,
    perry_system_is_dark_mode,
    perry_system_preferences_get,
    perry_system_preferences_set,
    // Canvas
    perry_ui_canvas_fill_rect,
    perry_ui_canvas_stroke_rect,
    perry_ui_canvas_clear_rect,
    perry_ui_canvas_set_fill_color,
    perry_ui_canvas_set_stroke_color,
    perry_ui_canvas_begin_path,
    perry_ui_canvas_move_to,
    perry_ui_canvas_line_to,
    perry_ui_canvas_arc,
    perry_ui_canvas_close_path,
    perry_ui_canvas_fill,
    perry_ui_canvas_stroke,
    perry_ui_canvas_set_line_width,
    perry_ui_canvas_fill_text,
    perry_ui_canvas_set_font,
    // App lifecycle
    perry_ui_app_run,
    // Timers
    perry_set_timeout,
    perry_set_interval,
    perry_clear_timeout,
    perry_clear_interval,
    // Path
    path: __path,
};

})();
