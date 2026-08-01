// The camera choreography, as a string that is evaluated once inside the page.
//
// It lives in the page rather than in the driver for one reason: every step of
// a camera move would otherwise be a `Runtime.evaluate` round trip, and on a
// Planet spectator those are the calls that time out when the main thread is
// busy. Installed once, the director runs on the page's own rAF and the driver
// only ever asks it to start something and reads a small status object back.
//
// It also paints. The spectator's animation ticker only redraws when it thinks
// something is moving, and a globe turned by writing the camera basis is not
// something it knows about — so a move that is not driven through the shipped
// easing would render at the idle cadence (90 ms in `balanced`) and read as a
// slideshow. Calling `draw()` from our own frame loop takes the rate back to
// whatever the machine can actually do.
module.exports = String.raw`
(() => {
  if (window.__dir) return "already installed";
  const dir = window.__dir = {
    busy: null,          // name of the running move, or null
    done: [],            // names of finished moves, newest last
    note: "",
  };

  function frames(ms, step, name) {
    dir.busy = name;
    dir.cut = false;
    return new Promise(finish => {
      const t0 = performance.now();
      const tick = now => {
        const t = Math.min(1, (now - t0) / ms);
        try { step(t, now); } catch (err) { dir.note = String(err); }
        try { draw(); } catch (err) { dir.note = String(err); }
        if (t < 1 && !dir.cut) requestAnimationFrame(tick);
        else { dir.busy = null; dir.cut = false; dir.done.push(name); finish(); }
      };
      requestAnimationFrame(tick);
    });
  }

  // Ease in and out, so a move starts and stops like a hand rather than like a
  // switch. Every move here uses it except the spin, which is meant to be a
  // steady turn and gets a gentler ramp at the ends only.
  const ease = t => t < .5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
  // A trapezoidal speed profile, integrated: the turn accelerates over the
  // first "edge" of the move, holds one steady rate through the middle, and
  // slows over the last one. A plain smoothstep spends the whole move
  // changing speed, which on a globe reads as a wobble rather than a turn.
  const ramp = (t, edge = .15) => {
    const total = 1 - edge;
    if (t < edge) return (t * t / (2 * edge)) / total;
    if (t > 1 - edge) { const u = 1 - t; return 1 - (u * u / (2 * edge)) / total; }
    return (t - edge / 2) / total;
  };

  // --- the moves -----------------------------------------------------------

  // Turn the globe the way a drag turns it: from the orientation it was grabbed
  // at, by a total number of pixels of arc. planetTurn takes the *original*
  // basis and the whole delta, exactly as the pointermove handler does.
  dir.spin = (degrees, ms) => {
    const basis0 = planetBasisNow();
    const reach = degrees / 360 * 2 * Math.PI * planetRadiusPx();
    return frames(ms, t => {
      applyPlanetBasis(planetTurn(basis0, reach * ramp(t), 0));
    }, "spin");
  };

  // Hold the picture still but keep painting it, so whatever is animating in
  // the sky — an orbit, an exhaust, a ship on its lane — is actually recorded.
  dir.hold = (ms, name = "hold") => frames(ms, () => {}, name);

  // Zoom toward a point on the canvas the way the wheel does, one notch at a
  // time, so the shipped gearing decides how far each notch goes.
  dir.zoom = (notches, ms, at) => {
    const rect = cv.getBoundingClientRect();
    const x = at ? at[0] : rect.width / 2, y = at ? at[1] : rect.height / 2;
    let sent = 0;
    return frames(ms, t => {
      const want = Math.round(ease(t) * Math.abs(notches));
      while (sent < want) {
        cv.dispatchEvent(new WheelEvent("wheel", {
          deltaY: notches > 0 ? -100 : 100, clientX: rect.left + x, clientY: rect.top + y,
          bubbles: true, cancelable: true}));
        sent++;
      }
    }, "zoom");
  };

  // Pull back until the Earth is a given number of pixels across. This is a
  // size and not a number of notches on purpose: two thresholds in the client
  // are written in drawn radius, and the shot only works between them.
  // Satellites and the expedition only animate once the world is under
  // SKY_SURFACE (96 px), and cities, borders and frontiers stop being drawn
  // under SKY_MARKERS (58) — so "the world with its traffic around it" is a
  // band, and a fixed count of wheel notches lands wherever the gearing feels
  // like putting it.
  dir.earth = (px, ms) => {
    let next = 0;
    return frames(ms, (t, now) => {
      if (now < next) return;
      const radius = planetEarthRadius();
      if (Math.abs(radius - px) < px * .06) return;
      next = now + 150;
      const rect = cv.getBoundingClientRect();
      cv.dispatchEvent(new WheelEvent("wheel", {
        deltaY: radius > px ? 100 : -100,
        clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2,
        bubbles: true, cancelable: true}));
    }, "earth");
  };

  // Press one of the sky bar's own buttons and then sit still while the flight
  // it starts plays out. skyTravel eases on cameraZoom.path inside the page's
  // ticker, so the wait is what records it.
  dir.fly = (stop, settleMs = 1000) => {
    const button = document.querySelector('[data-sky-stop="' + stop + '"]');
    if (!button) { dir.note = "no sky stop " + stop; return Promise.resolve(); }
    button.click();
    // A jump is capped at 5.1s but a short hop is much less, and sitting out the
    // cap on every one of them is half a minute of dead clip. cameraZoom is
    // nulled when the flight lands, so wait on the flight and not on the cap.
    let landed = 0;
    return frames(5400 + settleMs, (t, now) => {
      if (!landed && typeof cameraZoom !== "undefined" && !cameraZoom && t > .06) landed = now;
      if (landed && now - landed > settleMs) dir.cut = true;
    }, "fly:" + stop);
  };

  dir.skyStops = () => Array.from(document.querySelectorAll("[data-sky-stop]"))
    .map(button => button.dataset.skyStop + "|" + button.title);

  // Frame the whole world again — the shot the spectator opens on.
  dir.home = (ms = 1600) => {
    const button = document.querySelector('[data-sky-stop="world:earth"]');
    if (button) { button.click(); return frames(ms + 3000, () => {}, "home"); }
    fitFullWorldView();
    return frames(ms, () => {}, "home");
  };

  dir.status = () => JSON.stringify({
    busy: dir.busy, done: dir.done.slice(-6), note: dir.note,
    turn: state && state.turn, victory: state && state.victory_type,
    scale: cam && cam.scale, span: (() => { try { return skySpanLabel(skyStageWidth()); }
                                            catch (e) { return ""; } })(),
    projects: (state && state.players || []).filter(p => !p.is_minor && !p.is_barbarian)
      .map(p => p.civ + ":" + (p.science_projects || []).length +
                (p.exoplanet_distance ? "/" + Math.round(p.exoplanet_distance) : "")),
  });
  return "installed";
})()
`;
