// Browser harness for the web-channel MCP client (Phase 103, the EXAMPLE).
//
// This file is the explicit, teachable orchestration layer (D-01/D-07/D-09):
//   1. Full-page-redirect PKCE login — begin_login() returns the authorize URL,
//      we navigate to it; on return we detect ?code=&state= and call
//      complete_login() to exchange the code for a bearer (all wasm-side).
//   2. The MCP Tasks lifecycle over Fetch — invoke the task, then an EXPLICIT
//      ~500ms setTimeout poll loop on tasks/get until terminal, then tasks/result.
//      A Cancel button drives tasks/cancel mid-run.
//
// The poll loop is intentionally inline and visible (not hidden behind a helper)
// because demonstrating the Tasks lifecycle is the point of this example.

import init, { WasmClient } from './pkg/web_channel_client.js';

// --- Demo constants — MUST match the bundled server (examples/.../server). ---
// The IdP pre-registers this client_id and this exact redirect_uri; the PKCE
// redirect_uri passed at authorize-time must equal a registered URI.
const CLIENT_ID = 'web-channel-client';
const REDIRECT_URI = 'http://127.0.0.1:8080/callback';
const TASK_NAME = 'slow_summarize';
const POLL_INTERVAL_MS = 500; // D-09: fixed interval (not exponential backoff).

let client = null;
let currentTaskId = null;

const $ = (id) => document.getElementById(id);

function log(msg) {
    $('logs').textContent += `[${new Date().toLocaleTimeString()}] ${msg}\n`;
}

function setStatus(msg) {
    $('status').textContent = msg;
}

function setTaskStatus(msg) {
    $('task-status').textContent = `Task status: ${msg}`;
}

function originUrls() {
    const origin = $('server-origin').value.replace(/\/+$/, '');
    return {
        authorize: `${origin}/oauth2/authorize`,
        token: `${origin}/oauth2/token`,
        mcp: `${origin}/`,
    };
}

// Connect the high-level Client with the stored bearer, then enable task actions.
async function connectAndReady() {
    const { mcp } = originUrls();
    log(`Connecting MCP client to ${mcp} ...`);
    await client.connect(mcp);
    setStatus('Logged in and connected.');
    $('invoke-btn').disabled = false;
    $('logout-btn').disabled = false;
    $('login-btn').disabled = true;
    log('Connected. Ready to run the task.');
}

// Step 1a: kick off the full-page-redirect PKCE login (D-07).
async function onLogin() {
    try {
        const { authorize } = originUrls();
        log('Beginning PKCE login (generating verifier/challenge/state) ...');
        const authorizeUrl = client.begin_login(authorize, CLIENT_ID, REDIRECT_URI);
        log(`Redirecting to ${authorizeUrl}`);
        window.location = authorizeUrl; // full-page redirect
    } catch (e) {
        log(`Login error: ${e.message || e}`);
    }
}

// Step 1b: on return, detect ?code=&state= in the URL and finish the exchange.
async function handleRedirectIfPresent() {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    const state = params.get('state');
    if (!code || !state) {
        return false;
    }
    try {
        const { token } = originUrls();
        log('Detected ?code=&state= on return — exchanging code for a bearer ...');
        await client.complete_login(token, code, state, CLIENT_ID, REDIRECT_URI);
        // Strip the OAuth params from the address bar (avoid re-processing on reload).
        window.history.replaceState({}, document.title, window.location.pathname);
        log('Token exchange complete; bearer stored in sessionStorage.');
        await connectAndReady();
        return true;
    } catch (e) {
        log(`Token exchange failed: ${e.message || e}`);
        return false;
    }
}

// Step 2: invoke the task, then poll explicitly until terminal (D-09).
async function onInvoke() {
    try {
        $('result').textContent = '';
        $('invoke-btn').disabled = true;
        $('cancel-btn').disabled = false;
        setTaskStatus('invoking ...');
        currentTaskId = await client.invoke_task(TASK_NAME, { text: 'hello from the browser' });
        log(`Task created: ${currentTaskId}`);
        pollOnce();
    } catch (e) {
        log(`Invoke failed: ${e.message || e}`);
        $('invoke-btn').disabled = false;
        $('cancel-btn').disabled = true;
    }
}

// One step of the EXPLICIT ~500ms poll loop. Re-arms itself via setTimeout
// until the task reaches a terminal status, then fetches the result.
async function pollOnce() {
    if (!currentTaskId) {
        return;
    }
    try {
        const status = await client.poll_task(currentTaskId);
        setTaskStatus(status);
        log(`Polled tasks/get -> ${status}`);
        if (status === 'working' || status === 'input_required') {
            // Not terminal yet — schedule the next poll at the fixed interval.
            setTimeout(pollOnce, POLL_INTERVAL_MS);
            return;
        }
        // Terminal: completed | failed | cancelled.
        $('cancel-btn').disabled = true;
        $('invoke-btn').disabled = false;
        if (status === 'completed') {
            const result = await client.task_result(currentTaskId);
            $('result').textContent = JSON.stringify(result, null, 2);
            log('Fetched tasks/result.');
        }
        currentTaskId = null;
    } catch (e) {
        // A poll failure is terminal for this loop (e.g. the bearer expired or the
        // server went away). Stop rather than re-arming every 500ms forever — an
        // unbounded retry would spin indefinitely and leave the buttons stuck. Surface
        // it and restore the controls so the user can retry.
        log(`Poll error, stopping: ${e.message || e}`);
        setTaskStatus('error');
        $('cancel-btn').disabled = true;
        $('invoke-btn').disabled = false;
        currentTaskId = null;
    }
}

// Cancel button (D-09): drive tasks/cancel on the in-flight task.
async function onCancel() {
    if (!currentTaskId) {
        return;
    }
    try {
        const status = await client.cancel_task(currentTaskId);
        setTaskStatus(status);
        log(`Cancelled task -> ${status}`);
    } catch (e) {
        log(`Cancel failed: ${e.message || e}`);
    }
}

async function onLogout() {
    try {
        client.logout();
        currentTaskId = null;
        setStatus('Not logged in.');
        setTaskStatus('idle');
        $('result').textContent = '';
        $('invoke-btn').disabled = true;
        $('cancel-btn').disabled = true;
        $('logout-btn').disabled = true;
        $('login-btn').disabled = false;
        log('Logged out (sessionStorage cleared).');
    } catch (e) {
        log(`Logout error: ${e.message || e}`);
    }
}

async function main() {
    await init();
    client = new WasmClient();
    log('WASM module initialized.');

    $('login-btn').addEventListener('click', onLogin);
    $('logout-btn').addEventListener('click', onLogout);
    $('invoke-btn').addEventListener('click', onInvoke);
    $('cancel-btn').addEventListener('click', onCancel);

    // If we are returning from the IdP redirect, finish login automatically.
    // Otherwise, if a bearer survived in this tab's sessionStorage, reconnect.
    const resumed = await handleRedirectIfPresent();
    if (!resumed && client.is_logged_in()) {
        log('Existing bearer found in sessionStorage — reconnecting.');
        try {
            await connectAndReady();
        } catch (e) {
            log(`Reconnect failed: ${e.message || e}`);
        }
    }
}

main();
