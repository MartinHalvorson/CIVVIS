"use strict";
// What goes up: the rockets every launch is drawn with, and the laser stations
// a civilization sends up to hurry its expedition. Carved out of app.js.
//
// ⚠ THIS SCRIPT IS LOADED BEFORE app.js, for the reason app_palette.js gives
// at length: every script the page loads shares one global scope, and a name
// moved EARLIER can never be read before its declaration has run, where one
// moved later can. Nothing here runs at load. The constants are plain numbers,
// and every function below is called by app.js once a world is on screen, so
// the renderer's own helpers they lean on (`cx`, `pcol`, `anim`,
// `planetProject`, `activeSkyLaunches`, `SKY_LAUNCH_FLIGHTS` and the rest)
// exist by the time they are needed.

// All launch views use one silhouette vocabulary. Apollo 11's Saturn V is a
// long, staged white stack; Mars uses the compact stainless Starship outline;
// the exoplanet ship is that same outline at exactly twice the scale — still
// unmistakably a rocket, just monumental. Widening it faster than lengthening
// it (an earlier 4x width) squashed the aspect ratio to ~1.5:1 and read as an
// egg with fins. `plume` scales the engine flame with the hull, so the doubled
// ship is not pushed by a normal-size flame. Keeping the dimensions here makes
// the doubling exact in both projections rather than two hand-tuned
// approximations.
function skyRocketGeometry(rocket, size) {
  // Low orbit needs far less rocket than the Moon does: the satellite rides the
  // short, slim R-7 that carried Sputnik.
  if (rocket === "orbital") return {
    family:"orbital", length:size * 3, halfWidth:size * .42, plume:.75,
  };
  const starshipLength = size * 3.4;
  const starshipHalfWidth = size * .55;
  if (rocket === "starship_xl") return {
    family:"starship", length:starshipLength * 2,
    halfWidth:starshipHalfWidth * 2, plume:2,
  };
  if (rocket === "starship") return {
    family:"starship", length:starshipLength, halfWidth:starshipHalfWidth,
    plume:1,
  };
  return {family:"saturn_v", length:size * 4.7, halfWidth:size * .52, plume:1};
}

// Draw in a local frame whose +x axis is the flight direction. `pixel` is one
// screen pixel in the caller's coordinate system: one on the globe, inverse
// zoom on the flat board. The return value lets labels and culling respect the
// very different silhouettes without duplicating their dimensions.
function drawMissionRocket(rocket, player, size, now, pixel = 1, alpha = 1, thrust = 1) {
  const shape = skyRocketGeometry(rocket, size);
  const nose = shape.length / 2, tail = -shape.length / 2;
  const colour = pcol(player.id), dark = pcol2(player.id);
  const flame = size * (shape.plume || 1);
  const flicker = thrust * flame * (1.8 + .32 * Math.sin(now / 75 + player.id));
  const flameHalf = Math.max(flame * .42, shape.halfWidth * .32);

  cx.save();
  // A warm, transparent plume ties the diagrammatic craft to the rest of the
  // game's effects without turning any of the silhouettes into clip art.
  if (thrust > 0) {
    const plume = cx.createLinearGradient(tail - flicker, 0, tail + flame * .3, 0);
    plume.addColorStop(0, "rgba(255,173,69,0)");
    plume.addColorStop(.62, "rgba(255,176,70,.68)");
    plume.addColorStop(1, "rgba(255,244,191,.94)");
    cx.globalAlpha = alpha * .92; cx.fillStyle = plume;
    cx.beginPath(); cx.moveTo(tail + flame * .18, 0);
    cx.lineTo(tail - flicker, -flameHalf);
    cx.lineTo(tail - flicker * .82, flameHalf); cx.closePath(); cx.fill();
  }

  cx.globalAlpha = alpha;
  cx.strokeStyle = "#071018"; cx.lineWidth = Math.max(.8 * pixel, size * .11);
  if (shape.family === "orbital") {
    // The R-7: strap-on boosters flaring out around the base of a slim core,
    // and the jersey band just under the payload it is carrying.
    cx.fillStyle = "#c3ccd0";
    for (const side of [-1, 1]) {
      cx.beginPath();
      cx.moveTo(tail + shape.length * .52, side * shape.halfWidth * .72);
      cx.lineTo(tail, side * shape.halfWidth * 1.95);
      cx.lineTo(tail, side * shape.halfWidth * .8);
      cx.closePath(); cx.fill(); cx.stroke();
    }
    cx.fillStyle = "#edf1eb";
    cx.beginPath(); cx.moveTo(nose, 0);
    cx.lineTo(nose - shape.length * .2, -shape.halfWidth * .66);
    cx.lineTo(nose - shape.length * .48, -shape.halfWidth * .74);
    cx.lineTo(tail, -shape.halfWidth);
    cx.lineTo(tail, shape.halfWidth);
    cx.lineTo(nose - shape.length * .48, shape.halfWidth * .74);
    cx.lineTo(nose - shape.length * .2, shape.halfWidth * .66);
    cx.closePath(); cx.fill(); cx.stroke();
    cx.fillStyle = colour;
    cx.fillRect(nose - shape.length * .46, -shape.halfWidth * .74,
                shape.length * .1, shape.halfWidth * 1.48);
  } else if (shape.family === "saturn_v") {
    // Apollo 11's Saturn V: command-module point, narrow staged cylinder,
    // alternating roll bands, and the fins around the first-stage engines.
    cx.fillStyle = "#edf1eb";
    cx.beginPath(); cx.moveTo(nose, 0);
    cx.lineTo(nose - shape.length * .14, -shape.halfWidth * .68);
    cx.lineTo(tail + shape.length * .08, -shape.halfWidth);
    cx.lineTo(tail, -shape.halfWidth * .72);
    cx.lineTo(tail, shape.halfWidth * .72);
    cx.lineTo(tail + shape.length * .08, shape.halfWidth);
    cx.lineTo(nose - shape.length * .14, shape.halfWidth * .68);
    cx.closePath(); cx.fill(); cx.stroke();
    cx.fillStyle = "#111820";
    for (const band of [.18, .43, .68])
      cx.fillRect(tail + shape.length * band, -shape.halfWidth,
                  shape.length * .075, shape.halfWidth * 2);
    cx.fillStyle = colour;
    cx.fillRect(tail + shape.length * .76, -shape.halfWidth * .7,
                shape.length * .045, shape.halfWidth * 1.4);
    cx.strokeStyle = "#58616a"; cx.lineWidth = Math.max(.55 * pixel, size * .055);
    for (const seam of [.34, .61]) {
      const x = tail + shape.length * seam;
      cx.beginPath(); cx.moveTo(x, -shape.halfWidth); cx.lineTo(x, shape.halfWidth); cx.stroke();
    }
    cx.fillStyle = dark;
    cx.beginPath(); cx.moveTo(tail + shape.length * .12, -shape.halfWidth * .75);
    cx.lineTo(tail - shape.length * .055, -shape.halfWidth * 1.75);
    cx.lineTo(tail + shape.length * .035, -.05 * pixel); cx.closePath(); cx.fill();
    cx.beginPath(); cx.moveTo(tail + shape.length * .12, shape.halfWidth * .75);
    cx.lineTo(tail - shape.length * .055, shape.halfWidth * 1.75);
    cx.lineTo(tail + shape.length * .035, .05 * pixel); cx.closePath(); cx.fill();
  } else {
    // Starship: a stainless tapered hull, dark heat-shield belly, forward
    // flaps and broad aft fins. The XL craft uses precisely the same drawing;
    // only skyRocketGeometry doubles every dimension.
    const steel = cx.createLinearGradient(0, -shape.halfWidth, 0, shape.halfWidth);
    steel.addColorStop(0, "#f2f5f3"); steel.addColorStop(.48, "#aeb9bd");
    steel.addColorStop(1, "#65737a");
    cx.fillStyle = steel;
    cx.beginPath(); cx.moveTo(nose, 0);
    cx.quadraticCurveTo(nose - shape.length * .12, -shape.halfWidth,
                        nose - shape.length * .25, -shape.halfWidth);
    cx.lineTo(tail + shape.length * .08, -shape.halfWidth);
    cx.lineTo(tail, -shape.halfWidth * .58);
    cx.lineTo(tail, shape.halfWidth * .58);
    cx.lineTo(tail + shape.length * .08, shape.halfWidth);
    cx.lineTo(nose - shape.length * .25, shape.halfWidth);
    cx.quadraticCurveTo(nose - shape.length * .12, shape.halfWidth, nose, 0);
    cx.closePath(); cx.fill(); cx.stroke();
    cx.globalAlpha = alpha * .58; cx.fillStyle = "#172129";
    cx.beginPath(); cx.moveTo(tail + shape.length * .08, 0);
    cx.lineTo(nose - shape.length * .2, 0);
    cx.quadraticCurveTo(nose - shape.length * .1, shape.halfWidth * .42, nose, 0);
    cx.lineTo(nose - shape.length * .25, shape.halfWidth);
    cx.lineTo(tail + shape.length * .08, shape.halfWidth); cx.closePath(); cx.fill();
    cx.globalAlpha = alpha; cx.fillStyle = dark;
    const front = nose - shape.length * .28, rear = tail + shape.length * .13;
    cx.beginPath(); cx.moveTo(front, -shape.halfWidth * .72);
    cx.lineTo(front - shape.length * .13, -shape.halfWidth * 1.72);
    cx.lineTo(front + shape.length * .04, -shape.halfWidth * .92); cx.closePath(); cx.fill();
    cx.beginPath(); cx.moveTo(rear, -shape.halfWidth * .72);
    cx.lineTo(tail - shape.length * .06, -shape.halfWidth * 1.62);
    cx.lineTo(rear + shape.length * .1, -shape.halfWidth * .9); cx.closePath(); cx.fill();
    cx.beginPath(); cx.moveTo(rear, shape.halfWidth * .72);
    cx.lineTo(tail - shape.length * .06, shape.halfWidth * 1.62);
    cx.lineTo(rear + shape.length * .1, shape.halfWidth * .9); cx.closePath(); cx.fill();
    cx.fillStyle = colour;
    cx.fillRect(tail + shape.length * .48, -shape.halfWidth,
                shape.length * .055, shape.halfWidth * 2);
    cx.fillStyle = "#1b313e";
    const windows = rocket === "starship_xl" ? 5 : 3;
    for (let index = 0; index < windows; index++) {
      const x = nose - shape.length * (.2 + index * .035);
      cx.beginPath(); cx.arc(x, -shape.halfWidth * .38,
        Math.max(.5 * pixel, shape.halfWidth * .07), 0, 7); cx.fill();
    }
  }
  cx.restore();
  return shape;
}

// Laser stations. Every one a civilization launches to hurry its expedition is
// a craft of its own, on its own orbit, sent up from the Spaceport that built
// it. They are told apart by the order they went up in, and each borrows the
// satellite's whole apparatus (its orbit, its climb, the handover from rocket
// to craft) under an id no civilization has.
const SKY_LASER_SEAT = 4096, SKY_LASER_MOST = 24;
// Stations fly half a ring out from their civilization's satellite, so the two
// are never drawn on top of each other where their orbits cross.
const SKY_LASER_RING = .5;
// Stations finished in one frame go up one after another, this far apart, and
// no more than `SKY_LASER_VOLLEY` of them: the rest are simply up there.
const SKY_LASER_STAGGER = 900, SKY_LASER_VOLLEY = 3;
function skyLaserStations(player) {
  return Math.max(0, Math.min(SKY_LASER_MOST, Math.floor(+player?.laser_stations || 0)));
}
function skyLaserOrbiter(playerId, station) {
  return {id:playerId + SKY_LASER_SEAT * station};
}
// Whose orbit a launch is flying to: the civilization's own for its satellite,
// the station's for a laser station.
function skyOrbiter(launch, player) {
  return launch.mission === "laser" ? skyLaserOrbiter(player.id, launch.station) : player;
}
// Which Spaceports sent up the stations a frame added, one pad for each. The
// frame before still has the project at the head of the queue of every city
// that was running it, and the ones that finished are the ones that have since
// dropped it or started their production over. Stations are built side by
// side, so one frame can finish several, each from its own pad. A station
// nobody was seen building leaves from a Spaceport of the empire that has not
// launched in this frame, and only an empire with no Spaceport to be seen
// gets no pad at all.
function skyLaserOrigins(prev, next, pid, count) {
  const {project} = SKY_LAUNCH_FLIGHTS.laser;
  const owned = st => (st?.cities || []).filter(city => city.owner === pid);
  const after = new Map(owned(next).map(city => [city.id, city]));
  const running = city => (city?.queue || [])[0]?.project === project;
  const finished = city => !running(after.get(city.id)) ||
    (+after.get(city.id).production || 0) < (+city.production || 0);
  const ran = owned(prev).filter(running);
  const pads = [...ran.filter(finished), ...ran.filter(city => !finished(city))]
    .map(city => citySpaceportPos(after.get(city.id) || city)).filter(Boolean);
  const standing = [...owned(next).map(citySpaceportPos).filter(Boolean),
    ...(next.map?.tiles || []).filter(tile => tile.owner === pid && tile.district &&
      (tile.district === "spaceport" ||
       RULES?.districts?.[tile.district]?.replaces === "spaceport")).map(tile => tile.pos)];
  for (const pos of standing)
    if (!pads.some(pad => pad[0] === pos[0] && pad[1] === pos[1])) pads.push(pos);
  return Array.from({length:count}, (_, index) => pads.length ? pads[index % pads.length] : null);
}
// A rocket for every laser station the count went up by, each from its own
// Spaceport. A count this frame did not move launches nothing, which is also
// what a page opened on a game already under way sees: the stations are
// standing in their orbits and no rocket is owed for them.
function queueSkyLaserLaunches(prev, next, player, old, now) {
  const stations = skyLaserStations(player), had = skyLaserStations(old);
  if (stations <= had) return;
  const flight = SKY_LAUNCH_FLIGHTS.laser;
  const fresh = Math.min(SKY_LASER_VOLLEY, stations - had);
  const pads = skyLaserOrigins(prev, next, player.id, fresh);
  for (let index = 0; index < fresh; index++) {
    const station = stations - fresh + index + 1;
    // Repeated snapshots of one transition must not restart the flight.
    if (anim.skyLaunches.some(launch => launch.playerId === player.id &&
        launch.mission === "laser" && launch.station === station)) continue;
    const origin = pads[index] || statePlayerAnchor(next, player.id)?.pos;
    const t0 = now + index * SKY_LASER_STAGGER;
    anim.skyLaunches.push({
      playerId:player.id, player:{id:player.id}, mission:"laser", station, t0,
      duration:flight.duration, endsAt:t0 + flight.duration,
      originPos:Array.isArray(origin) ? [...origin] : null, crew:[],
    });
  }
}
// How much of one station is out of its rocket, as `skyCraftDeployed` says it
// for a satellite. A civilization may have several in flight at once, so the
// station is named as well as its owner.
function skyLaserDeployed(playerId, station, now) {
  const launch = activeSkyLaunches(now).find(candidate => candidate.playerId === playerId &&
    candidate.mission === "laser" && candidate.station === station);
  return launch ? skyLaunchHandover(launch, now) : 1;
}

// A laser station has to read as something other than a satellite at three
// pixels: a diamond in the jersey with a lit lens in the middle of it, where a
// satellite is a bar with wings. The lens breathes, each station out of step
// with the next, because it is the one craft up there that is doing something.
function drawSkyLaserStation(x, y, size, player, station, alpha, now, outline = 0) {
  const reach = size * 1.3;
  cx.beginPath();
  cx.moveTo(x, y - reach); cx.lineTo(x + reach, y);
  cx.lineTo(x, y + reach); cx.lineTo(x - reach, y);
  cx.closePath();
  cx.globalAlpha = alpha;
  cx.fillStyle = pcol(player.id); cx.fill();
  if (outline) { cx.strokeStyle = "#07100f"; cx.lineWidth = outline; cx.stroke(); }
  cx.globalAlpha = alpha * (.62 + .38 * Math.sin(now / 420 + station * 2.4));
  cx.fillStyle = "#fff6dc";
  cx.beginPath(); cx.arc(x, y, Math.max(.7 * (outline || 1), reach * .38), 0, 7); cx.fill();
}

// Every laser station on the globe, on the orbit its own launch flew. They get
// no track: a ring apiece would bury the ones that say whose satellite is whose.
function drawSkyLaserStations(crew, view, radius, centerX, centerY, size, shown, now) {
  for (const player of crew) {
    const ring = skyOrbitSlot(crew, player) + SKY_LASER_RING;
    for (let station = 1, count = skyLaserStations(player); station <= count; station++) {
      const deployed = skyLaserDeployed(player.id, station, now);
      if (!deployed) continue;
      const basis = skySatelliteOrbit(skyLaserOrbiter(player.id, station), ring, radius);
      const at = planetProject(skySatelliteSite(basis, skySatelliteAngle(basis, now)),
                               view, radius, centerX, centerY);
      if (at.z < 0 && Math.hypot(at.x - centerX, at.y - centerY) < radius) continue;
      drawSkyLaserStation(at.x, at.y, size, player, station, shown * deployed, now);
    }
  }
}
// The same stations over a flat board: each above its own point on the ground,
// on a drop line like the satellite's, without the wake and the horizon that
// mark the satellite out.
function drawFlatLaserStations(crew, shown, pixel, lift, halfW, halfH, now) {
  for (const player of crew) {
    const colour = pcol(player.id);
    for (let station = 1, count = skyLaserStations(player); station <= count; station++) {
      const deployed = skyLaserDeployed(player.id, station, now);
      if (!deployed) continue;
      const orbit = skyOrbit(skyLaserOrbiter(player.id, station));
      const here = flatSkyProject(...flatSatelliteGround(orbit, flatSatelliteTheta(orbit, now)));
      if (Math.abs(here.x) > halfW || Math.abs(here.y) > halfH) continue;
      cx.setLineDash([]);
      cx.beginPath(); cx.moveTo(here.x, here.y); cx.lineTo(here.x, here.y - lift);
      cx.globalAlpha = shown * deployed * .2;
      cx.strokeStyle = "#07100f"; cx.lineWidth = 2.4 * pixel; cx.stroke();
      cx.globalAlpha = shown * deployed * .4;
      cx.strokeStyle = colour; cx.lineWidth = .9 * pixel; cx.stroke();
      drawSkyLaserStation(here.x, here.y - lift, 3.4 * pixel, player, station,
                          shown * deployed, now, .9 * pixel);
    }
  }
}
