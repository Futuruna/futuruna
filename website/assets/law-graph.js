/* A quiet technical drawing: outlined nodes, restrained tilts, and rare traces. */
(() => {
    window.__futurunaLawGraph?.destroy();
    const root = document.getElementById('law-graph');
    const hero = root?.closest('.hero');
    const canvas = document.getElementById('law-graph-canvas');
    const pauseButton = document.getElementById('law-graph-pause');
    const ctx = canvas?.getContext('2d');
    if (!hero || !ctx || !pauseButton) return;

    const preference = matchMedia('(prefers-reduced-motion: reduce)');
    const events = new AbortController();
    const styles = getComputedStyle(hero);
    const ink = styles.getPropertyValue('--graph-ink').trim() || '#aaa5ba';
    const activeInk = styles.getPropertyValue('--graph-active').trim() || '#d3cde3';
    const ground = styles.getPropertyValue('--graph-ground').trim() || '#08080d';
    const pointer = { x: 0, y: 0, tx: 0, ty: 0, strength: 0, active: false };
    const nodes = [], edges = [], signals = [];
    const additions = [];
    const viewport = { x: 0, y: 0, width: 0, height: 0 };
    const maxAdditions = 40;
    let additionSerial = 0;
    const introDuration = 3.2;
    const signalLegDuration = 1.6;
    let width = 0, height = 0, dpr = 1;
    let frame = 0, lastTime = 0, time = 0, nextSignal = 4.6;
    let introAge = preference.matches ? introDuration : 0;
    let visible = true, destroyed = false, paused = false;

    const noise = i => { const n = Math.sin(i * 127.1 + 311.7) * 43758.5453; return n - Math.floor(n); };
    const clamp = (x, a, b) => Math.max(a, Math.min(b, x));
    const smooth = (a, b, x) => {
        const t = clamp((x - a) / (b - a), 0, 1);
        return t * t * (3 - 2 * t);
    };
    const moving = () => !paused && !preference.matches && visible && !document.hidden;

    function makeGraph(preservePositions = false) {
        const previous = preservePositions ? new Map(nodes.map(n => [n.key, n])) : new Map();
        nodes.length = edges.length = signals.length = 0;
        const cols = clamp(Math.ceil(width / 145) + 1, 3, 20);
        const rows = clamp(Math.ceil(height / 125) + 1, 3, 14);
        for (let row = 0; row < rows; row++) {
            for (let col = 0; col < cols; col++) {
                const id = nodes.length;
                const x = (col - 0.25 + noise(id + 10) * 0.5) * width / (cols - 1);
                const y = (row - 0.25 + noise(id + 200) * 0.5) * height / (rows - 1);
                nodes.push({
                    key: `base-${id}`, born: null, selectedAt: -Infinity,
                    x, y, bx: x, by: y, angle: 0,
                    triangle: id % 2 === 0, radius: 5.2 + noise(id + 150) * 1.8,
                    heat: 0, neighbors: [], reveal: 1, signal: 0
                });
                if (col) edges.push([id - 1, id]);
                if (row && (col + row) % 2 === 0) edges.push([id - cols, id]);
                if (row && col && (col + row) % 3 === 0) edges.push([id - cols - 1, id]);
            }
        }
        additions.forEach(appendAddition);
        nodes.forEach(n => {
            const old = previous.get(n.key);
            if (old) {
                for (const field of ['x', 'y', 'angle', 'heat', 'selectedAt']) n[field] = old[field];
            }
        });
        edges.forEach(([a, b], edge) => {
            nodes[a].neighbors.push({ node: b, edge });
            nodes[b].neighbors.push({ node: a, edge });
        });
    }

    function appendAddition(placement) {
        const x = viewport.x + placement.u * viewport.width;
        const y = viewport.y + placement.v * viewport.height;
        const id = nodes.length;
        const nearby = nodes.map((n, index) => ({ index, distance: Math.hypot(n.bx - x, n.by - y) }))
            .sort((a, b) => a.distance - b.distance);
        let split = -1, splitDistance = 12;
        edges.forEach(([a, b], index) => {
            const p = nodes[a], q = nodes[b];
            const dx = q.bx - p.bx, dy = q.by - p.by;
            const lengthSquared = dx * dx + dy * dy;
            if (!lengthSquared) return;
            const t = ((x - p.bx) * dx + (y - p.by) * dy) / lengthSquared;
            if (t < 0.12 || t > 0.88) return;
            const distance = Math.hypot(x - p.bx - t * dx, y - p.by - t * dy);
            if (distance < splitDistance) { split = index; splitDistance = distance; }
        });
        nodes.push({
            key: `added-${placement.serial}`, born: placement.born, selectedAt: placement.born,
            x, y, bx: x, by: y, angle: 0, triangle: placement.serial % 2 === 0,
            radius: 6, heat: 0, neighbors: [], reveal: 0, signal: 0
        });
        if (split >= 0) {
            const [a, b] = edges.splice(split, 1)[0];
            edges.push([id, a, placement.born], [id, b, placement.born]);
            return;
        }

        // Short, separated links keep the new node part of the local structure.
        const chosen = [];
        const maxDistance = Math.max(220, nearby[0].distance * 2.5);
        const cross = (p, q, r) => (q.bx - p.bx) * (r.by - p.by) - (q.by - p.by) * (r.bx - p.bx);
        for (const { index, distance } of nearby) {
            if (chosen.length >= 3 || distance > maxDistance) break;
            const target = nodes[index];
            const angle = Math.atan2(target.by - y, target.bx - x);
            if (chosen.some(other => Math.cos(angle - other.angle) > 0.7)) continue;
            const intersects = edges.some(([a, b]) => {
                if (a === index || b === index || a === id || b === id) return false;
                const p = nodes[id], q = target, r = nodes[a], s = nodes[b];
                return cross(p, q, r) * cross(p, q, s) < -0.001 && cross(r, s, p) * cross(r, s, q) < -0.001;
            });
            if (intersects) continue;
            edges.push([id, index, placement.born]);
            chosen.push({ angle });
        }
        // Preserve connectivity even in an unusually crowded region.
        if (!chosen.length) edges.push([id, nearby[0].index, placement.born]);
    }

    function addNode(x, y) {
        if (!nodes.length || !viewport.width || !viewport.height) return;
        const existing = nodes.findIndex(n => Math.hypot(n.x - x, n.y - y) < 24);
        let id = existing;
        if (existing < 0) {
            // Bound the sketch; after forty additions, replace the oldest one.
            if (additions.length >= maxAdditions) additions.shift();
            const placement = {
                u: clamp((x - viewport.x) / viewport.width, 0, 1),
                v: clamp((y - viewport.y) / viewport.height, 0, 1),
                serial: additionSerial++, born: moving() ? time : time - 2
            };
            additions.push(placement);
            makeGraph(true);
            id = nodes.findIndex(n => n.key === `added-${placement.serial}`);
        }
        nodes[id].selectedAt = time;
        if (moving()) {
            startSignal(id, existing < 0 ? 1.1 : 0);
            nextSignal = time + 10;
        }
        schedule();
    }

    function startSignal(start = null, delay = 0) {
        // Start outside the central text clearing, where a quiet trace is visible.
        const candidates = nodes.map((n, id) => ({ n, id })).filter(({ n }) =>
            n.bx > 105 && n.bx < width - 105 && n.by > 105 && n.by < height - 105 &&
            (Math.abs(n.bx - width / 2) > width * 0.23 || Math.abs(n.by - height / 2) > height * 0.24));
        if (start === null && !candidates.length) return;
        let from = start ?? candidates[Math.floor(noise(time + 300) * candidates.length)].id;
        const visited = new Set([from]);
        const route = [];
        const length = 3 + Math.floor(noise(time + 500) * 2);
        for (let i = 0; i < length; i++) {
            const choices = nodes[from].neighbors.filter(({ node }) => !visited.has(node));
            if (!choices.length) break;
            const { node: to, edge } = choices[Math.floor(noise(time + i * 17) * choices.length)];
            route.push({ from, to, edge });
            visited.add(to);
            from = to;
        }
        if (route.length) {
            // One trace at a time, with quiet intervals between paths.
            signals.length = 0;
            signals.push({ route, age: -delay });
        }
    }

    function step(dt) {
        time += dt;
        introAge = Math.min(introDuration, introAge + dt);
        const pointerEase = 1 - Math.exp(-dt * 9);
        pointer.x += (pointer.tx - pointer.x) * pointerEase;
        pointer.y += (pointer.ty - pointer.y) * pointerEase;
        pointer.strength += ((pointer.active ? 1 : 0) - pointer.strength) * (1 - Math.exp(-dt * 4));
        // Ambient traces follow actual connections, even with a stationary mouse.
        if (nodes.length && time > nextSignal) {
            startSignal();
            nextSignal = time + 9 + noise(time + 700) * 4;
        }
        for (let i = signals.length - 1; i >= 0; i--) {
            signals[i].age += dt;
            if (signals[i].age > (signals[i].route.length + 1) * signalLegDuration) signals.splice(i, 1);
        }
        nodes.forEach(n => {
            let tx = n.bx, ty = n.by, targetAngle = 0;
            let proximity = 0;
            if (pointer.strength > 0.001) {
                const dx = pointer.x - n.x, dy = pointer.y - n.y;
                const distance = Math.hypot(dx, dy);
                const falloff = Math.max(0, 1 - distance / 260);
                proximity = falloff * falloff * (3 - 2 * falloff) * pointer.strength;
                tx += dx * proximity * 0.035;
                ty += dy * proximity * 0.035;
                targetAngle = Math.sin(Math.atan2(dy, dx)) * proximity * 0.14;
            }
            // Exponential easing settles without overshoot or continuous drift.
            const ease = 1 - Math.exp(-dt * 4);
            n.angle += (targetAngle - n.angle) * ease;
            n.heat += (proximity * 0.8 - n.heat) * ease;
            n.x += (tx - n.x) * ease;
            n.y += (ty - n.y) * ease;
        });
    }

    function draw() {
        ctx.clearRect(0, 0, width, height);
        ctx.lineCap = 'round';
        const progress = clamp((introAge - 0.12) / (introDuration - 0.12), 0, 1);
        const front = -160 + (Math.hypot(width, height) / 2 + 340) * (1 - (1 - progress) ** 1.3);
        nodes.forEach(n => {
            const distance = Math.hypot(n.bx - width / 2, n.by - height / 2);
            n.reveal = n.born !== null ? smooth(0, 0.5, time - n.born)
                : introAge >= introDuration ? 1 : smooth(distance - 100, distance + 100, front);
            n.signal = 0;
        });

        const edgeSignals = new Float32Array(edges.length);
        const heads = [];
        signals.forEach(signal => signal.route.forEach((leg, index) => {
            const phase = signal.age / signalLegDuration - index;
            const light = smooth(0, 0.25, phase) * (1 - smooth(1, 1.9, phase)) * 0.3;
            edgeSignals[leg.edge] = Math.max(edgeSignals[leg.edge], light);
            nodes[leg.from].signal = Math.max(nodes[leg.from].signal, light * (1 - smooth(0.2, 0.9, phase)));
            nodes[leg.to].signal = Math.max(nodes[leg.to].signal, light * smooth(0.5, 1, phase));
            if (phase > 0 && phase < 1) heads.push({ ...leg, phase });
        }));

        edges.forEach(([a, b, born], index) => {
            const from = nodes[a], to = nodes[b];
            const join = born === undefined ? 1 : smooth(0.12, 1, time - born);
            const heat = Math.max(from.heat, to.heat, edgeSignals[index]);
            ctx.strokeStyle = ink;
            ctx.globalAlpha = (0.17 + heat * 0.22) * Math.min(from.reveal, to.reveal);
            ctx.lineWidth = 0.7;
            ctx.beginPath();
            ctx.moveTo(from.x, from.y);
            ctx.lineTo(from.x + (to.x - from.x) * join, from.y + (to.y - from.y) * join);
            ctx.stroke();
        });
        heads.forEach(({ from, to, phase }) => {
            const a = nodes[from], b = nodes[to];
            const tail = Math.max(0, phase - 0.16);
            ctx.strokeStyle = activeInk;
            ctx.globalAlpha = Math.sin(phase * Math.PI) * 0.42 * Math.min(a.reveal, b.reveal);
            ctx.lineWidth = 1.1;
            ctx.beginPath();
            ctx.moveTo(a.x + (b.x - a.x) * tail, a.y + (b.y - a.y) * tail);
            ctx.lineTo(a.x + (b.x - a.x) * phase, a.y + (b.y - a.y) * phase);
            ctx.stroke();
        });
        nodes.forEach(n => {
            const selected = 1 - smooth(0.3, 1.8, time - n.selectedAt);
            const emphasis = Math.max(n.heat, n.signal, selected * 0.65);
            const radius = n.radius;
            ctx.save();
            ctx.translate(n.x, n.y);
            ctx.rotate(n.angle);
            ctx.beginPath();
            if (n.triangle) {
                for (let corner = 0; corner < 3; corner++) {
                    const angle = -Math.PI / 2 + corner * Math.PI * 2 / 3;
                    const x = Math.cos(angle) * radius;
                    const y = Math.sin(angle) * radius;
                    if (corner === 0) ctx.moveTo(x, y);
                    else ctx.lineTo(x, y);
                }
                ctx.closePath();
            } else {
                ctx.rect(-radius * 0.75, -radius * 0.75, radius * 1.5, radius * 1.5);
            }
            ctx.lineJoin = 'round';
            ctx.lineWidth = 0.9;
            // An opaque interior masks the connection beneath each hollow node.
            ctx.globalAlpha = n.reveal;
            ctx.fillStyle = ground;
            ctx.fill();
            ctx.globalAlpha = (0.58 + emphasis * 0.28) * n.reveal;
            ctx.strokeStyle = selected > 0.05 ? activeInk : ink;
            ctx.stroke();
            ctx.restore();
        });
        ctx.globalAlpha = 1;
    }

    function tick(now) {
        frame = 0;
        if (destroyed) return;
        const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.04) : 0;
        lastTime = now;
        if (moving()) step(dt);
        draw();
        if (moving() && !frame) frame = requestAnimationFrame(tick);
    }

    function schedule() {
        if (!frame && !destroyed && visible && !document.hidden) frame = requestAnimationFrame(tick);
    }

    function resize() {
        const rect = root.getBoundingClientRect();
        if (!rect.width || !rect.height) return;
        width = rect.width;
        height = rect.height;
        const heroRect = hero.getBoundingClientRect();
        viewport.x = heroRect.left - rect.left;
        viewport.y = heroRect.top - rect.top;
        viewport.width = heroRect.width;
        viewport.height = heroRect.height;
        dpr = Math.min(devicePixelRatio || 1, 2);
        canvas.width = Math.ceil(width * dpr);
        canvas.height = Math.ceil(height * dpr);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.imageSmoothingEnabled = true;
        makeGraph();
        schedule();
    }

    function point(event) {
        const rect = root.getBoundingClientRect();
        pointer.tx = clamp(event.clientX - rect.left, 0, width);
        pointer.ty = clamp(event.clientY - rect.top, 0, height);
        if (!pointer.active) {
            pointer.x = pointer.tx;
            pointer.y = pointer.ty;
        }
        pointer.active = true;
    }

    function updateMotion() {
        if (paused || preference.matches) introAge = introDuration;
        if (paused || preference.matches) {
            additions.forEach(p => { p.born = Math.min(p.born, time - 2); });
            nodes.forEach(n => { if (n.born !== null) n.born = Math.min(n.born, time - 2); });
            edges.forEach(edge => { if (edge[2] !== undefined) edge[2] = Math.min(edge[2], time - 2); });
        }
        pauseButton.textContent = preference.matches ? 'Reduced motion' : paused ? 'Resume motion' : 'Pause motion';
        pauseButton.setAttribute('aria-pressed', String(paused || preference.matches));
        pauseButton.disabled = preference.matches;
        lastTime = 0;
        schedule();
    }

    const on = (target, type, listener) => target.addEventListener(type, listener, { signal: events.signal, passive: true });
    on(hero, 'pointermove', point);
    on(hero, 'pointerleave', () => { pointer.active = false; });
    on(hero, 'pointercancel', () => { pointer.active = false; });
    on(hero, 'click', event => {
        if (event.button !== 0 || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
        if (event.target.closest('.hero-inner, a, button, input, textarea, select, [role="button"]')) return;
        point(event);
        addNode(pointer.tx, pointer.ty);
    });
    on(pauseButton, 'click', () => { paused = !paused; updateMotion(); });
    on(preference, 'change', updateMotion);
    on(document, 'visibilitychange', () => { lastTime = 0; schedule(); });
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(root);
    const intersectionObserver = new IntersectionObserver(entries => {
        visible = entries[0].isIntersecting;
        lastTime = 0;
        schedule();
    });
    intersectionObserver.observe(hero);

    window.__futurunaLawGraph = {
        destroy() {
            destroyed = true;
            cancelAnimationFrame(frame);
            events.abort();
            resizeObserver.disconnect();
            intersectionObserver.disconnect();
            delete window.__futurunaLawGraph;
        }
    };
    resize();
    updateMotion();
})();
