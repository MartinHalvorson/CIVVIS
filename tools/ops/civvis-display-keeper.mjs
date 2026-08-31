#!/usr/bin/env node
/**
 * Keep the dedicated, extension-free CIVVIS display alive beside a foreground
 * Civilization VI game.
 *
 * Civ VI must remain frontmost or macOS starves its frame-tied turn loop.  The
 * display therefore uses its own Chrome profile with background throttling
 * disabled and reports its complete-frame acknowledgement through /status.
 * This keeper never touches the game, controller, mirror follower, or an
 * existing browser process; it only owns the profile and DevTools port below.
 */

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";

// The durable operator halt (`gamelock.py --halt`). On 2026-08-31 this keeper
// survived a full halt teardown as a launchd orphan and kept forcing the
// dedicated display window open at the upper-left of a machine whose games
// were stopped — and `launchDisplay()` fired whenever DevTools was quiet,
// without asking whether any mirror existed to show. The contract now: the
// window exists only while a mirror is serving a session. Halted, the keeper
// closes the window and stays quiet; mirror absent, it never launches.
const haltFile = process.env.CIVVIS_OPERATOR_HALT_FILE
  || `${process.env.HOME}/.civvis-operator-halt.json`;

const mirrorPort = Number(process.env.CIVVIS_MIRROR_PORT || "8610");
const debugPort = Number(process.env.CIVVIS_DISPLAY_DEBUG_PORT || "9230");
const mirrorUrl = `http://127.0.0.1:${mirrorPort}/`;
const debugUrl = `http://127.0.0.1:${debugPort}`;
const chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = `${process.env.HOME}/civvis-display-profile`;
const pollMs = 5_000;
const paintGraceMs = 25_000;
const recoveryCooldownMs = 30_000;

let ticking = false;
let lastInstance = null;
let unhealthySince = null;
let lastRecoveryAt = 0;
let lastLaunchAt = 0;

function log(message) {
  console.log(`[${new Date().toISOString()}] ${message}`);
}

async function json(url, timeoutMs = 3_000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { signal: controller.signal, cache: "no-store" });
    if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
    return await response.json();
  } finally {
    clearTimeout(timer);
  }
}

async function displayPage() {
  const pages = await json(`${debugUrl}/json/list`);
  return pages.find(page => page.type === "page" && page.url.includes(`127.0.0.1:${mirrorPort}`)) || null;
}

function launchDisplay() {
  const now = Date.now();
  if (now - lastLaunchAt < recoveryCooldownMs) return;
  lastLaunchAt = now;
  const args = [
    "-na", "Google Chrome", "--args",
    `--app=${mirrorUrl}?display=dedicated`,
    `--user-data-dir=${profile}`,
    `--remote-debugging-port=${debugPort}`,
    "--remote-allow-origins=*",
    "--disable-extensions",
    "--disable-gpu",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--disable-backgrounding-occluded-windows",
    "--disable-features=CalculateNativeWinOcclusion,IntensiveWakeUpThrottling",
    "--no-first-run", "--no-default-browser-check",
    "--window-position=0,33", "--window-size=864,542",
  ];
  const child = spawn("/usr/bin/open", args, { detached: true, stdio: "ignore" });
  child.unref();
  log("requested dedicated CIVVIS display launch");
}

async function cdp(page, method, params) {
  return await new Promise((resolve, reject) => {
    const socket = new WebSocket(page.webSocketDebuggerUrl);
    const timer = setTimeout(() => {
      socket.close();
      reject(new Error(`DevTools timeout: ${method}`));
    }, 6_000);
    socket.addEventListener("open", () => {
      socket.send(JSON.stringify({ id: 1, method, params }));
    }, { once: true });
    socket.addEventListener("message", event => {
      let message;
      try {
        message = JSON.parse(String(event.data));
      } catch {
        return;
      }
      if (message.id !== 1) return;
      clearTimeout(timer);
      socket.close();
      if (message.error) reject(new Error(JSON.stringify(message.error)));
      else resolve(message.result);
    });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      reject(new Error(`DevTools connection failed: ${method}`));
    }, { once: true });
  });
}

async function reloadDisplay(page, instance, why) {
  const now = Date.now();
  if (now - lastRecoveryAt < recoveryCooldownMs) return;
  lastRecoveryAt = now;
  const target = `${mirrorUrl}?display=dedicated&instance=${encodeURIComponent(instance)}&recovery=${now}`;
  await cdp(page, "Page.navigate", { url: target });
  unhealthySince = null;
  log(`reloaded dedicated display for ${why} (server ${instance})`);
}

// A Chrome process can survive with its dedicated app window closed.  DevTools
// still answers, so launchDisplay is not reached, but the mirror sees zero
// viewers forever.  Create a target in that already-running browser instead
// of spawning a second Chrome instance against the same profile.
async function createDisplayPage(instance, why) {
  const now = Date.now();
  if (now - lastRecoveryAt < recoveryCooldownMs) return;
  lastRecoveryAt = now;
  const browser = await json(`${debugUrl}/json/version`);
  const target = `${mirrorUrl}?display=dedicated&instance=${encodeURIComponent(instance)}&recovery=${now}`;
  await cdp(browser, "Target.createTarget", { url: target });
  unhealthySince = null;
  log(`created dedicated display for ${why} (server ${instance})`);
}

// The window must not outlive the session it displays.  /json/close is the
// DevTools HTTP endpoint; it returns plain text, so this bypasses json().
async function closeDisplayPage(why) {
  let page;
  try {
    page = await displayPage();
  } catch {
    return; // no browser answering: nothing to close
  }
  if (!page) return;
  try {
    await fetch(`${debugUrl}/json/close/${page.id}`, { cache: "no-store" });
    log(`closed dedicated display (${why})`);
  } catch {}
}

async function tick() {
  if (ticking) return;
  ticking = true;
  try {
    if (existsSync(haltFile)) {
      await closeDisplayPage("operator halt in force");
      unhealthySince = null;
      return;
    }

    let status;
    try {
      status = await json(`${mirrorUrl}status`);
    } catch {
      // A follower intentionally frees :8610 between batches, and a halted or
      // idle machine serves no mirror at all.  With no source there is nothing
      // to show: never launch, create, or reload a display toward a dead port.
      unhealthySince = null;
      return;
    }

    let page;
    try {
      page = await displayPage();
    } catch {
      launchDisplay();
      return;
    }

    const instance = status.server_instance;
    if (instance === null || instance === undefined) return;
    if (!page) {
      await createDisplayPage(instance, "page missing");
      return;
    }
    if (lastInstance !== null && String(instance) !== String(lastInstance) && page) {
      await reloadDisplay(page, instance, "mirror process changed");
    }
    lastInstance = instance;

    const turn = Number(status.turn);
    const painted = Number(status.frames_painted);
    const healthy = Number(status.viewers) >= 1
      && Number.isFinite(painted)
      && (!Number.isFinite(turn) || painted >= turn - 1);
    if (healthy) {
      unhealthySince = null;
      return;
    }

    unhealthySince ??= Date.now();
    if (page && Date.now() - unhealthySince >= paintGraceMs) {
      await reloadDisplay(page, instance, "viewer or paint acknowledgement missing");
    }
  } catch (error) {
    log(`display check deferred: ${error.message}`);
  } finally {
    ticking = false;
  }
}

log(`watching dedicated display on DevTools :${debugPort} and mirror :${mirrorPort}`);
await tick();
setInterval(tick, pollMs);
