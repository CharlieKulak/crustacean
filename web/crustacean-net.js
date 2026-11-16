// Crustacean's network, storage and file picking for the browser build
// (the Rust side is src/net.rs). Results are queued as JSON events that the game
// collects every frame.
"use strict";
(function () {
    const events = [];
    const sockets = {};
    let nextId = 1;
    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const SETTINGS_KEY = "crustacean-settings";
    const OLD_SETTINGS_KEY = "opensnake-rs-settings"; // (from before Crustacean had its name)

    function str(ptr, len) {
        return len ? dec.decode(new Uint8Array(wasm_memory.buffer, ptr, len)) : "";
    }
    // A string handed to the game: it asks for the length, makes room, then takes it.
    let pending = null;
    function offer(text) {
        pending = enc.encode(text);
        return pending.length;
    }
    function take(ptr) {
        new Uint8Array(wasm_memory.buffer, ptr, pending.length).set(pending);
        pending = null;
    }
    function push(ev) {
        events.push(JSON.stringify(ev));
    }

    function os_http(mp, ml, up, ul, bp, bl, tp, tl) {
        const id = nextId++;
        const method = str(mp, ml), url = str(up, ul), body = str(bp, bl), token = str(tp, tl);
        const headers = {};
        if (token) headers["Authorization"] = "Bearer " + token;
        if (method === "POST") headers["Content-Type"] = "application/json";
        fetch(url, { method, headers, body: method === "POST" ? (body || "{}") : undefined, cache: "no-store" })
            .then(r => r.text().then(text => push({ k: "http", id, status: r.status, body: text })))
            .catch(() => push({ k: "http", id, status: 0, body: "{\"error\":\"Couldn't reach the server.\"}" }));
        return id;
    }

    function os_ws_open(up, ul) {
        const id = nextId++;
        let ws;
        try {
            ws = new WebSocket(str(up, ul));
        } catch (e) {
            push({ k: "closed", id });
            return id;
        }
        sockets[id] = ws;
        ws.onopen = () => push({ k: "open", id });
        ws.onmessage = m => { if (typeof m.data === "string") push({ k: "msg", id, text: m.data }); };
        ws.onclose = () => { delete sockets[id]; push({ k: "closed", id }); };
        return id;
    }

    function os_ws_send(id, tp, tl) {
        const ws = sockets[id];
        if (ws && ws.readyState === 1) ws.send(str(tp, tl));
    }

    function os_ws_close(id) {
        const ws = sockets[id];
        if (ws) ws.close();
    }

    function os_event_len() {
        return events.length ? offer(events.shift()) : -1;
    }

    function os_storage_len() {
        let v = null;
        try {
            v = window.localStorage.getItem(SETTINGS_KEY);
            if (v === null) v = window.localStorage.getItem(OLD_SETTINGS_KEY); // (keeps your login)
        } catch (e) { }
        return v === null ? -1 : offer(v);
    }

    function os_storage_set(vp, vl) {
        try { window.localStorage.setItem(SETTINGS_KEY, str(vp, vl)); } catch (e) { }
    }

    // A profile picture: pick a file, shrink it to a 256 x 256 PNG, hand it over.
    function os_pick_image() {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = "image/*";
        input.onchange = () => {
            const file = input.files && input.files[0];
            if (!file) return;
            const img = new Image();
            img.onload = () => {
                const side = 256, c = document.createElement("canvas");
                c.width = c.height = side;
                const s = Math.min(img.width, img.height);
                c.getContext("2d").drawImage(img, (img.width - s) / 2, (img.height - s) / 2, s, s, 0, 0, side, side);
                const data = c.toDataURL("image/png").split(",")[1];
                push({ k: "picked", data });
                URL.revokeObjectURL(img.src);
            };
            img.src = URL.createObjectURL(file);
        };
        input.click();
    }

    function os_origin_len() {
        return offer(window.location.origin);
    }

    function os_set_title(tp, tl) {
        document.title = str(tp, tl);
    }

    function os_now() {
        return Date.now();
    }

    miniquad_add_plugin({
        register_plugin: function (importObject) {
            Object.assign(importObject.env, {
                os_http, os_ws_open, os_ws_send, os_ws_close,
                os_event_len, os_event_take: take,
                os_storage_len, os_storage_take: take, os_storage_set,
                os_pick_image, os_origin_len, os_origin_take: take, os_set_title, os_now,
            });
        },
        name: "crustacean_net",
        version: 1,
    });
})();
