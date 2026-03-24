const machineStatusEl = document.getElementById("machineStatus");
const factorioStatusEl = document.getElementById("factorioStatus");
const wakeBtn = document.getElementById("wakeBtn");
const wakeResultEl = document.getElementById("wakeResult");
const wakeTimerEl = document.getElementById("wakeTimer");

const sessionsLoadingEl = document.getElementById("sessionsLoading");
const sessionsEmptyEl = document.getElementById("sessionsEmpty");
const sessionsListEl = document.getElementById("sessionsList");
const prevSessionsBtn = document.getElementById("prevSessionsBtn");
const nextSessionsBtn = document.getElementById("nextSessionsBtn");
const sessionsPageInfoEl = document.getElementById("sessionsPageInfo");

let wakeInProgressOverride = false;
let sleepAbortController = null;

let sessionsOffset = 0;
const sessionsLimit = 10;
let sessionsLastCount = 0;
const expandedSessions = new Map();

let sessionsRefreshScheduled = false;

let wakeRequestedAt = null;
let wakeTimerIntervalStarted = false;

function interruptSleep() {
    if (sleepAbortController) {
        sleepAbortController.abort();
        sleepAbortController = null;
    }
}

function ensureWakeTimerInterval() {
    if (wakeTimerIntervalStarted) {
        return;
    }

    wakeTimerIntervalStarted = true;

    setInterval(() => {
        if (wakeInProgressOverride && wakeRequestedAt) {
            const seconds = Math.floor((Date.now() - wakeRequestedAt) / 1000);
            wakeTimerEl.textContent = `${seconds}s since wake issued`;
        }
    }, 1000);
}

function sleep(ms) {
    sleepAbortController = new AbortController();

    return new Promise((resolve) => {
        const timeoutId = setTimeout(() => {
            sleepAbortController = null;
            resolve();
        }, ms);

        sleepAbortController.signal.addEventListener(
            "abort",
            () => {
                clearTimeout(timeoutId);
                sleepAbortController = null;
                resolve();
            },
            { once: true }
        );
    });
}

function setStatus(el, onlineText, offlineText, isOnline) {
    el.textContent = isOnline ? onlineText : offlineText;
    el.classList.remove("online", "offline", "starting");
    el.classList.add(isOnline ? "online" : "offline");
}

function applyStatusData(data) {
    setStatus(machineStatusEl, "Online", "Offline", data.machine_online);
    setStatus(factorioStatusEl, "Online", "Offline", data.factorio_online);

    if (data.machine_online) {
        wakeInProgressOverride = false;
        wakeRequestedAt = null;
    }

    if (wakeInProgressOverride && wakeRequestedAt) {
        const seconds = Math.floor((Date.now() - wakeRequestedAt) / 1000);
        wakeTimerEl.textContent = `${seconds}s since wake issued`;
    } else if (data.machine_online && !data.factorio_online) {
        wakeTimerEl.textContent = "Machine reachable, waiting for Factorio";
    } else if (data.machine_online) {
        wakeTimerEl.textContent = "Machine reachable";
    } else {
        wakeTimerEl.textContent = "—";
    }
}

const dateFormatter = new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "long",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
});

function formatTimestamp(ts) {
    try {
        return dateFormatter.format(new Date(ts));
    } catch {
        return ts;
    }
}

function sessionSummaryText(session) {
    const end = session.ended_at ? formatTimestamp(session.ended_at) : "ongoing";
    return `${formatTimestamp(session.started_at)} → ${end}`;
}

async function refreshStatus() {
    const response = await fetch("/api/status", { cache: "no-store" });
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
    }

    const data = await response.json();

    applyStatusData(data);
    wakeResultEl.textContent = data.last_wake_message ?? "—";

    if (data.machine_online) {
        wakeInProgressOverride = false;
        wakeRequestedAt = null;
    }

    return data;
}

function getPollIntervalMs(data) {
    const fastMs = 900;
    const slowMs = 30000;

    const wakeInProgress = wakeInProgressOverride || data.waiting_for_machine_online;
    const factorioStillStarting = data.machine_online && !data.factorio_online;

    if (wakeInProgress || factorioStillStarting) {
        return fastMs;
    }

    return slowMs;
}

async function loadSessions() {
    sessionsLoadingEl.classList.remove("hidden");
    sessionsEmptyEl.classList.add("hidden");

    const response = await fetch(
        `/api/sessions?limit=${sessionsLimit}&offset=${sessionsOffset}`,
        { cache: "no-store" }
    );

    if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
    }

    const sessions = await response.json();
    sessionsLastCount = sessions.length;

    renderSessions(sessions);

    sessionsLoadingEl.classList.add("hidden");
    sessionsEmptyEl.classList.toggle("hidden", sessions.length > 0);
    sessionsPageInfoEl.textContent = `Page ${Math.floor(sessionsOffset / sessionsLimit) + 1}`;
    prevSessionsBtn.disabled = sessionsOffset === 0;
    nextSessionsBtn.disabled = sessions.length < sessionsLimit;
}

function renderSessions(sessions) {
    sessionsListEl.innerHTML = "";

    for (const session of sessions) {
        const wrapper = document.createElement("div");
        wrapper.className = "session-item";

        const header = document.createElement("div");
        header.className = "session-header";

        const titleWrap = document.createElement("div");
        titleWrap.className = "session-title-wrap";

        const title = document.createElement("div");
        title.className = "session-title";
        title.textContent = `Session #${session.id}`;

        const range = document.createElement("div");
        range.className = "session-range mono";
        range.textContent = sessionSummaryText(session);

        titleWrap.appendChild(title);
        titleWrap.appendChild(range);
        header.appendChild(titleWrap);

        const summary = document.createElement("div");
        summary.className = "session-summary";
        summary.textContent =
            `${session.unique_players} unique players • ${session.total_events} events`;

        const actions = document.createElement("div");
        actions.className = "session-actions";

        const toggleBtn = document.createElement("button");
        toggleBtn.textContent = expandedSessions.has(session.id) ? "Hide details" : "Show details";

        const detailContainer = document.createElement("div");
        detailContainer.className = "session-detail";
        detailContainer.style.display = expandedSessions.has(session.id) ? "block" : "none";

        if (expandedSessions.has(session.id)) {
            renderSessionDetail(detailContainer, expandedSessions.get(session.id));
        }

        toggleBtn.addEventListener("click", async () => {
            if (detailContainer.style.display === "block") {
                detailContainer.style.display = "none";
                toggleBtn.textContent = "Show details";
                return;
            }

            detailContainer.style.display = "block";
            toggleBtn.textContent = "Hide details";

            if (!expandedSessions.has(session.id)) {
                detailContainer.innerHTML = `<div class="muted">Loading details…</div>`;

                try {
                    const detail = await loadSessionDetail(session.id);
                    expandedSessions.set(session.id, detail);
                    renderSessionDetail(detailContainer, detail);
                } catch {
                    detailContainer.innerHTML = `<div class="muted">Failed to load details.</div>`;
                }
            } else {
                renderSessionDetail(detailContainer, expandedSessions.get(session.id));
            }
        });

        actions.appendChild(toggleBtn);

        wrapper.appendChild(header);
        wrapper.appendChild(summary);
        wrapper.appendChild(actions);
        wrapper.appendChild(detailContainer);

        sessionsListEl.appendChild(wrapper);
    }
}

async function loadSessionDetail(sessionId) {
    const response = await fetch(`/api/sessions/${sessionId}`, { cache: "no-store" });
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
    }
    return await response.json();
}

function renderSessionDetail(container, detail) {
    container.innerHTML = "";

    const stats = document.createElement("div");
    stats.className = "session-stats";

    const uniquePlayers = document.createElement("div");
    uniquePlayers.className = "session-stat";
    uniquePlayers.innerHTML = `<strong>Unique players</strong><div>${detail.unique_players}</div>`;

    const totalEvents = document.createElement("div");
    totalEvents.className = "session-stat";
    totalEvents.innerHTML = `<strong>Total events</strong><div>${detail.total_events}</div>`;

    const maxConcurrent = document.createElement("div");
    maxConcurrent.className = "session-stat";
    maxConcurrent.innerHTML = `<strong>Max concurrent</strong><div>${detail.max_concurrent_players}</div>`;

    stats.appendChild(uniquePlayers);
    stats.appendChild(totalEvents);
    stats.appendChild(maxConcurrent);

    container.appendChild(stats);

    const eventsList = document.createElement("div");
    eventsList.className = "events-list";

    if (!detail.events || detail.events.length === 0) {
        const empty = document.createElement("div");
        empty.className = "muted";
        empty.textContent = "No player events in this session.";
        eventsList.appendChild(empty);
    } else {
        for (const event of detail.events) {
            const item = document.createElement("div");
            item.className = `event-item ${event.event_type}`;

            const accent = document.createElement("div");
            accent.className = "event-accent";

            const body = document.createElement("div");
            body.className = "event-body";

            const time = document.createElement("div");
            time.className = "event-time";
            time.textContent = formatTimestamp(event.timestamp);

            const text = document.createElement("div");
            text.className = "event-text";
            text.textContent =
                event.event_type === "join"
                    ? `${event.player_name} joined`
                    : `${event.player_name} left`;

            body.appendChild(time);
            body.appendChild(text);

            item.appendChild(accent);
            item.appendChild(body);

            eventsList.appendChild(item);
        }
    }

    container.appendChild(eventsList);
}

async function refreshExpandedSessions() {
    const ids = Array.from(expandedSessions.keys());
    for (const id of ids) {
        try {
            const detail = await loadSessionDetail(id);
            expandedSessions.set(id, detail);
        } catch {
            // ignore transient failures
        }
    }
}

async function refreshSessionsData() {
    await loadSessions();
    await refreshExpandedSessions();
    renderSessions(await fetchSessionsPageAgainForRender());
}

async function fetchSessionsPageAgainForRender() {
    const response = await fetch(
        `/api/sessions?limit=${sessionsLimit}&offset=${sessionsOffset}`,
        { cache: "no-store" }
    );
    if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
    }
    return await response.json();
}

function scheduleSessionsRefresh() {
    if (sessionsRefreshScheduled) {
        return;
    }

    sessionsRefreshScheduled = true;

    setTimeout(async () => {
        sessionsRefreshScheduled = false;

        try {
            const sessions = await fetchSessionsPageAgainForRender();

            for (const session of sessions) {
                if (expandedSessions.has(session.id)) {
                    try {
                        const detail = await loadSessionDetail(session.id);
                        expandedSessions.set(session.id, detail);
                    } catch {
                        // ignore
                    }
                }
            }

            sessionsLastCount = sessions.length;
            renderSessions(sessions);

            sessionsLoadingEl.classList.add("hidden");
            sessionsEmptyEl.classList.toggle("hidden", sessions.length > 0);
            sessionsPageInfoEl.textContent = `Page ${Math.floor(sessionsOffset / sessionsLimit) + 1}`;
            prevSessionsBtn.disabled = sessionsOffset === 0;
            nextSessionsBtn.disabled = sessions.length < sessionsLimit;
        } catch {
            // ignore transient websocket-driven refresh failures
        }
    }, 150);
}

function connectWebSocket() {
    const protocol = window.location.protocol === "https:" ? "wss" : "ws";
    const ws = new WebSocket(`${protocol}://${window.location.host}/ws`);

    ws.addEventListener("message", (event) => {
        try {
            const data = JSON.parse(event.data);

            if (data.type === "status") {
                applyStatusData(data);
                return;
            }

            if (
                data.type === "session_started" ||
                data.type === "session_event" ||
                data.type === "session_ended"
            ) {
                scheduleSessionsRefresh();
            }
        } catch {
            // ignore malformed messages
        }
    });

    ws.addEventListener("close", () => {
        setTimeout(connectWebSocket, 2000);
    });

    ws.addEventListener("error", () => {
        ws.close();
    });
}

async function pollingLoop() {
    while (true) {
        try {
            await refreshStatus();
        } catch {
            machineStatusEl.textContent = "Status check failed";
            machineStatusEl.classList.remove("online");
            machineStatusEl.classList.add("offline");

            factorioStatusEl.textContent = "Status check failed";
            factorioStatusEl.classList.remove("online");
            factorioStatusEl.classList.add("offline");
        }

        await sleep(30000);
    }
}

wakeBtn.addEventListener("click", async () => {
    wakeBtn.disabled = true;

    wakeInProgressOverride = true;
    wakeRequestedAt = Date.now();
    wakeTimerEl.textContent = "0s since wake issued";
    interruptSleep();

    try {
        const response = await fetch("/api/wake", {
            method: "POST",
            cache: "no-store",
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }

        const data = await response.json();
        wakeResultEl.textContent = data.message;
        interruptSleep();
    } catch (err) {
        wakeInProgressOverride = false;
        wakeRequestedAt = null;
        wakeTimerEl.textContent = "—";
        wakeResultEl.textContent = `Error calling /api/wake: ${err}`;
    } finally {
        wakeBtn.disabled = false;
    }
});

prevSessionsBtn.addEventListener("click", async () => {
    if (sessionsOffset === 0) return;
    sessionsOffset -= sessionsLimit;
    await loadSessions();
});

nextSessionsBtn.addEventListener("click", async () => {
    if (sessionsLastCount < sessionsLimit) return;
    sessionsOffset += sessionsLimit;
    await loadSessions();
});

(async function init() {
    ensureWakeTimerInterval();
    connectWebSocket();
    await Promise.allSettled([
        loadSessions(),
        refreshStatus(),
    ]);
    pollingLoop();
})();
