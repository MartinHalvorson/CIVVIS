// Wizard Chess, the laptop version: the board, the pieces that move
// themselves, the player's seat at it, and the game.
//
// Three parts, kept apart so that a headset can take the first as it is
// (docs/WIZARD_CHESS.md):
//
//   the world   the hall, the board and the pieces, and how a move is
//               played out on it (`World`). It is built in squares, one
//               unit a square, and knows nothing of who is watching.
//   the seat    how a laptop sees and commands the world: a camera at the
//               player's side of the board or on the back of one of their
//               knights, a pointer, a keyboard and a microphone (`Seat`,
//               `Voice`, `Sound`).
//   the game    whose move it is and what may be played (`rules.js`), what a
//               command means (`command.js`) and what the opponent answers
//               (`engine.js`).

import * as THREE from "./vendor/three.module.min.js";
import {START, fromFen, toFen, legalMoves, play, san, uci, moveFromUci, standing, spoken, positionKey,
  squareName, colourOf, inCheck, kingOf, PIECE_NAMES} from "./rules.js";
import {readCommand, candidates} from "./command.js";
import {Opponent} from "./engine.js";

const CALM = window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const KEPT = "civvis-wizard-chess-v1";
const TAU = Math.PI * 2;

// ---------------------------------------------------------------- the clock
// Everything that moves is a tween: a promise that is kept when a stretch of
// the world's own time has run. One loop advances them all, so a move is a
// plain sequence of awaits.
const running = new Set();
const ease = {
  inOut: t => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2),
  out: t => 1 - Math.pow(1 - t, 3),
  in: t => t * t * t,
  line: t => t,
};
function tween(seconds, step, curve = ease.inOut) {
  if (CALM) seconds *= 0.45;
  return new Promise(done => {
    if (seconds <= 0) { step(1); done(); return; }
    running.add({at: 0, seconds, step, curve, done});
  });
}
const wait = seconds => tween(seconds, () => {}, ease.line);
// The short way from one angle to another.
const turnBy = (from, to) => ((to - from + Math.PI) % TAU + TAU) % TAU - Math.PI;
function advance(dt) {
  for (const held of [...running]) {
    held.at += dt;
    const t = Math.min(1, held.at / held.seconds);
    held.step(held.curve(t));
    if (t >= 1) { running.delete(held); held.done(); }
  }
}

// ---------------------------------------------------------------- the world
const sqX = sq => (sq & 7) - 3.5;
const sqZ = sq => 3.5 - (sq >> 3);
const sqPoint = sq => new THREE.Vector3(sqX(sq), 0, sqZ(sq));

function glowTexture(inner = "rgba(255,255,255,1)", outer = "rgba(255,255,255,0)") {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 128;
  const pen = canvas.getContext("2d"), fill = pen.createRadialGradient(64, 64, 2, 64, 64, 64);
  fill.addColorStop(0, inner);
  fill.addColorStop(0.35, inner.replace(/[\d.]+\)$/, "0.45)"));
  fill.addColorStop(1, outer);
  pen.fillStyle = fill;
  pen.fillRect(0, 0, 128, 128);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

// A marble's veins, or a slab's grain: noise drawn once and laid over a
// colour, so that stone does not look like plastic.
function stoneTexture(base, vein, seed) {
  const size = 256, canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const pen = canvas.getContext("2d");
  pen.fillStyle = base;
  pen.fillRect(0, 0, size, size);
  let n = seed;
  const dice = () => { n = (n * 1664525 + 1013904223) >>> 0; return n / 4294967296; };
  for (let at = 0; at < 1400; at++) {
    pen.fillStyle = `rgba(${dice() < 0.5 ? "0,0,0" : "255,255,255"},${0.012 + dice() * 0.03})`;
    pen.fillRect(dice() * size, dice() * size, 1 + dice() * 3, 1 + dice() * 3);
  }
  pen.strokeStyle = vein;
  for (let at = 0; at < 7; at++) {
    pen.lineWidth = 0.6 + dice() * 1.6;
    pen.beginPath();
    let x = dice() * size, y = dice() * size;
    pen.moveTo(x, y);
    for (let leg = 0; leg < 9; leg++) { x += (dice() - 0.5) * 70; y += (dice() - 0.3) * 60; pen.lineTo(x, y); }
    pen.stroke();
  }
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
  return texture;
}

// Night through a tall window: deep blue paling toward the top, a few stars,
// and the stone bars that hold the glass, drawn once.
function nightSky() {
  const canvas = document.createElement("canvas");
  canvas.width = 128; canvas.height = 384;
  const pen = canvas.getContext("2d"), sky = pen.createLinearGradient(0, 0, 0, 384);
  sky.addColorStop(0, "#2a4472");
  sky.addColorStop(0.55, "#132443");
  sky.addColorStop(1, "#0a1428");
  pen.fillStyle = sky;
  pen.fillRect(0, 0, 128, 384);
  let n = 17;
  const dice = () => { n = (n * 1664525 + 1013904223) >>> 0; return n / 4294967296; };
  for (let star = 0; star < 26; star++) {
    pen.fillStyle = `rgba(235,240,255,${0.35 + dice() * 0.6})`;
    pen.fillRect(dice() * 128, dice() * 300, 1 + dice() * 1.6, 1 + dice() * 1.6);
  }
  pen.fillStyle = "#17151b";
  pen.fillRect(60, 0, 8, 384);
  for (const y of [120, 250]) pen.fillRect(0, y, 128, 6);
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

// The shapes of the six pieces: each a profile turned on a lathe, with what
// a lathe cannot make set on top (a rook's battlements, a king's cross, a
// knight's head). One unit is a square; a king stands two of them tall.
const arc = (radius, height, tall, from, to, steps = 10) => Array.from({length: steps + 1}, (_, at) => {
  const angle = from + (to - from) * at / steps;
  return [Math.max(0, radius * Math.cos(angle)), height + tall * Math.sin(angle)];
});
const foot = wide => [[0, 0], [wide, 0], [wide, 0.05], [wide * 0.93, 0.09], [wide * 0.8, 0.12], [wide * 0.72, 0.17]];
const PROFILES = {
  p: [...foot(0.3), [0.2, 0.2], [0.13, 0.34], [0.11, 0.48], [0.19, 0.52], [0.2, 0.55], [0.12, 0.58], ...arc(0.17, 0.74, 0.17, -1.1, Math.PI / 2)],
  r: [...foot(0.34), [0.25, 0.22], [0.21, 0.35], [0.2, 0.72], [0.27, 0.78], [0.29, 0.82], [0.29, 1.02], [0.21, 1.02], [0.21, 0.92], [0, 0.92]],
  n: [...foot(0.34), [0.26, 0.22], [0.22, 0.3], [0.24, 0.36], [0, 0.36]],
  b: [...foot(0.33), [0.22, 0.22], [0.13, 0.4], [0.1, 0.72], [0.2, 0.78], [0.21, 0.82], [0.12, 0.86], ...arc(0.17, 1.08, 0.26, -1.2, Math.PI / 2)],
  q: [...foot(0.36), [0.25, 0.22], [0.14, 0.45], [0.1, 0.95], [0.22, 1.02], [0.23, 1.06], [0.13, 1.1], [0.16, 1.3], [0.25, 1.5], [0.2, 1.5], [0.12, 1.44], [0, 1.46]],
  k: [...foot(0.37), [0.26, 0.22], [0.15, 0.45], [0.11, 1.0], [0.24, 1.08], [0.25, 1.12], [0.14, 1.16], [0.17, 1.4], [0.26, 1.6], [0.2, 1.64], [0, 1.7]],
};
const HEIGHTS = {p: 0.91, r: 1.1, n: 1.3, b: 1.42, q: 1.63, k: 2.0};

function knightHead() {
  // The side of a horse's head, nose toward +x, cut from a slab.
  const outline = [[-0.21, 0.36], [-0.27, 0.6], [-0.27, 0.85], [-0.2, 1.05], [-0.12, 1.2], [-0.1, 1.36], [-0.03, 1.2],
    [0.06, 1.18], [0.2, 1.02], [0.36, 0.84], [0.43, 0.74], [0.4, 0.63], [0.3, 0.6], [0.2, 0.66], [0.12, 0.78], [0.06, 0.62],
    [0.12, 0.48], [0.22, 0.36]];
  const shape = new THREE.Shape();
  outline.forEach(([x, y], at) => (at ? shape.lineTo(x, y) : shape.moveTo(x, y)));
  shape.closePath();
  const head = new THREE.ExtrudeGeometry(shape, {depth: 0.17, bevelEnabled: true, bevelThickness: 0.05, bevelSize: 0.035, bevelSegments: 3, steps: 1});
  head.translate(0, 0, -0.085);
  head.computeVertexNormals();
  return head;
}

class World {
  constructor() {
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x040406);
    this.scene.fog = new THREE.FogExp2(0x040406, 0.028);
    this.pieces = new Map();
    // The knight the player rides, if they ride one (`ride`).
    this.ridden = null;
    // The piece that is moving now, if one is (`play`), and the one it is
    // about to take.
    this.moving = null;
    this.doomed = null;
    // The hall's light as the game goes (`dusk`, `spotlight`, `blaze`, and a
    // flare each time a piece falls): what it is now, and what it goes to.
    this.mood = {flare: 0, falls: 0, gloom: 0, gloomWant: 0, spot: null, spotNow: 0, blaze: 0, blazeNow: 0};
    // The side thinking, if one is, and how many times a side has (`muse`).
    this.musing = null;
    this.musings = 0;
    // How many times the pieces have been set down afresh (`set`). A move
    // still being played out when they are (a new game begun, a game come
    // back to) is the old board's, and stops touching this one.
    this.era = 0;
    this.glow = glowTexture();
    this.flames = [];
    this.debris = [];
    this.time = 0;
    this.light();
    this.hall();
    this.board();
    this.forms();
    this.marks();
  }

  light() {
    const scene = this.scene;
    this.sky = new THREE.HemisphereLight(0x9fb4d8, 0x1a120c, 0.5);
    scene.add(this.sky);
    // The key: warm, high and to the left of the seat, the one that casts.
    const key = this.key = new THREE.DirectionalLight(0xffe6c4, 2.6);
    key.position.set(-7, 13, 7);
    key.castShadow = true;
    key.shadow.mapSize.set(2048, 2048);
    Object.assign(key.shadow.camera, {left: -8, right: 8, top: 8, bottom: -8, near: 2, far: 40});
    key.shadow.bias = -0.0004;
    key.shadow.normalBias = 0.03;
    scene.add(key);
    // A cold rim from behind the far side picks the pieces out of the dark.
    const rim = new THREE.DirectionalLight(0x6f93ff, 0.7);
    rim.position.set(6, 5, -10);
    scene.add(rim);
    // What flares when something breaks.
    this.flash = new THREE.PointLight(0xfff0c8, 0, 12, 2);
    scene.add(this.flash);
  }

  hall() {
    const scene = this.scene;
    const floor = new THREE.Mesh(new THREE.CircleGeometry(60, 48),
      new THREE.MeshStandardMaterial({color: 0x16151a, roughness: 0.92, metalness: 0.05, map: stoneTexture("#3a3940", "rgba(0,0,0,0.25)", 7)}));
    floor.material.map.repeat.set(26, 26);
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = -0.62;
    floor.receiveShadow = true;
    scene.add(floor);
    // Braziers at the four corners: a pillar, a bowl, and a flame that is a
    // light and a glow.
    const stone = new THREE.MeshStandardMaterial({color: 0x2a282e, roughness: 0.85});
    for (const [x, z] of [[-7.2, -6.4], [7.2, -6.4], [-10.5, 1.5], [10.5, 1.5]]) {
      const pillar = new THREE.Mesh(new THREE.CylinderGeometry(0.34, 0.44, 2.6, 10), stone);
      pillar.position.set(x, 0.68, z);
      pillar.castShadow = pillar.receiveShadow = true;
      const bowl = new THREE.Mesh(new THREE.CylinderGeometry(0.62, 0.3, 0.36, 12), new THREE.MeshStandardMaterial({color: 0x3b2b1c, roughness: 0.5, metalness: 0.6}));
      bowl.position.set(x, 2.16, z);
      const flame = new THREE.Sprite(new THREE.SpriteMaterial({map: glowTexture("rgba(255,170,70,1)", "rgba(255,90,20,0)"), blending: THREE.AdditiveBlending, depthWrite: false, transparent: true}));
      flame.position.set(x, 2.75, z);
      flame.scale.set(2.4, 3, 1);
      const lamp = new THREE.PointLight(0xff8a3a, 26, 20, 2);
      lamp.position.set(x, 3, z);
      scene.add(pillar, bowl, flame, lamp);
      this.flames.push({flame, lamp, phase: x * 1.7 + z});
    }
    this.vault();
    // Motes in the air, which is what the light is seen by.
    const count = 260, spots = new Float32Array(count * 3);
    for (let at = 0; at < count; at++) {
      spots[at * 3] = (Math.random() - 0.5) * 22;
      spots[at * 3 + 1] = Math.random() * 7 + 0.2;
      spots[at * 3 + 2] = (Math.random() - 0.5) * 22;
    }
    const motes = new THREE.BufferGeometry();
    motes.setAttribute("position", new THREE.BufferAttribute(spots, 3));
    this.motes = new THREE.Points(motes, new THREE.PointsMaterial({map: this.glow, size: 0.07, color: 0xffd9a0, transparent: true, opacity: 0.5,
      blending: THREE.AdditiveBlending, depthWrite: false}));
    scene.add(this.motes);
  }

  // The hall the board stands in, for a seat low among the pieces to look up
  // at: a ring of twelve columns under a dome, a wall beyond them with tall
  // windows of night, and a shaft of pale light from the dome's eye onto the
  // board. Far enough out that the seat at the board is inside the ring, and
  // with no column on the line from either seat to the board. Nothing here
  // casts or takes a shadow, and the columns are one draw each part.
  vault() {
    const scene = this.scene, FLOOR_AT = -0.62, RING = 16, WALL = 20, TALL = 10;
    const stone = (repeatX, repeatY, seed, base = "#4a4852") => {
      const map = stoneTexture(base, "rgba(0,0,0,0.3)", seed);
      map.repeat.set(repeatX, repeatY);
      return new THREE.MeshStandardMaterial({color: 0x3a3842, roughness: 0.88, metalness: 0.04, map});
    };
    // The columns: a shaft, a base and a capital, each twelve at once.
    const parts = [
      [new THREE.CylinderGeometry(0.5, 0.58, TALL - 1.1, 18), FLOOR_AT + 0.5 + (TALL - 1.1) / 2],
      [new THREE.BoxGeometry(1.5, 0.5, 1.5), FLOOR_AT + 0.25],
      [new THREE.CylinderGeometry(0.95, 0.55, 0.6, 18), FLOOR_AT + TALL - 0.3],
    ];
    const columnStone = stone(1, 3, 37), place = new THREE.Object3D();
    for (const [geometry, y] of parts) {
      const ring = new THREE.InstancedMesh(geometry, columnStone, 12);
      for (let at = 0; at < 12; at++) {
        const round = (at + 0.5) / 12 * TAU;
        place.position.set(Math.sin(round) * RING, y, Math.cos(round) * RING);
        place.rotation.set(0, round, 0);
        place.updateMatrix();
        ring.setMatrixAt(at, place.matrix);
      }
      scene.add(ring);
    }
    // The ring the columns carry, and the wall beyond.
    const band = new THREE.Mesh(new THREE.CylinderGeometry(RING + 1.1, RING + 1.1, 1.3, 64, 1, true), stone(30, 1, 43));
    band.material.side = THREE.DoubleSide;
    band.position.y = FLOOR_AT + TALL + 0.65;
    const wall = new THREE.Mesh(new THREE.CylinderGeometry(WALL, WALL, TALL + 4, 64, 1, true), stone(26, 3, 47, "#3e3c46"));
    wall.material.side = THREE.BackSide;
    wall.position.y = FLOOR_AT + (TALL + 4) / 2;
    scene.add(band, wall);
    // Tall windows in the wall, between the columns: night beyond them.
    const arch = new THREE.Shape();
    arch.moveTo(-1.1, 0);
    arch.lineTo(1.1, 0);
    arch.lineTo(1.1, 5.2);
    arch.absarc(0, 5.2, 1.1, 0, Math.PI, false);
    arch.lineTo(-1.1, 0);
    const pane = new THREE.ShapeGeometry(arch, 12);
    // A shape's texture runs over its own units: map them onto 0..1.
    const uv = pane.attributes.uv, at = pane.attributes.position;
    for (let i = 0; i < uv.count; i++) uv.setXY(i, (at.getX(i) + 1.1) / 2.2, at.getY(i) / 6.3);
    const night = new THREE.MeshBasicMaterial({map: nightSky(), fog: false});
    for (let at = 0; at < 12; at++) {
      const round = at / 12 * TAU, window = new THREE.Mesh(pane, night);
      window.position.set(Math.sin(round) * (WALL - 0.05), FLOOR_AT + 2.4, Math.cos(round) * (WALL - 0.05));
      window.lookAt(0, FLOOR_AT + 2.4, 0);
      scene.add(window);
    }
    // The dome, its ribs, and its eye.
    const top = FLOOR_AT + TALL + 4;
    const dome = new THREE.Mesh(new THREE.SphereGeometry(WALL, 48, 16, 0, TAU, 0, Math.PI / 2), stone(16, 6, 53, "#36343d"));
    dome.material.side = THREE.BackSide;
    dome.position.y = top;
    scene.add(dome);
    const ribStone = stone(20, 1, 59);
    for (let at = 0; at < 6; at++) {
      const rib = new THREE.Mesh(new THREE.TorusGeometry(WALL - 0.2, 0.28, 6, 48, Math.PI), ribStone);
      rib.position.y = top;
      rib.rotation.y = at / 6 * Math.PI + Math.PI / 12;
      scene.add(rib);
    }
    const eye = new THREE.Mesh(new THREE.CircleGeometry(2.4, 32), new THREE.MeshBasicMaterial({color: 0xbcd2ff, fog: false}));
    eye.rotation.x = Math.PI / 2;
    eye.position.y = top + WALL - 0.4;
    scene.add(eye);
    // The light that falls from it: a shaft seen in the dusty air, and the
    // pale pool it makes on the board.
    const shaft = new THREE.Mesh(new THREE.CylinderGeometry(2.3, 4.6, top + WALL + 0.6, 32, 1, true),
      new THREE.MeshBasicMaterial({color: 0x8fb0ff, transparent: true, opacity: 0.045, blending: THREE.AdditiveBlending,
        depthWrite: false, side: THREE.DoubleSide, fog: false}));
    shaft.position.y = (top + WALL + 0.6) / 2 + FLOOR_AT;
    scene.add(shaft);
    const moon = this.moon = new THREE.SpotLight(0xa8c2ff, 420, 60, 0.16, 0.7, 2);
    moon.position.set(0, top + WALL - 1, 0);
    moon.target.position.set(0, 0, 0);
    scene.add(moon, moon.target);
    // The beam that comes down from the eye onto a king in check, seen in the
    // dusty air: drawn only while there is one (`tick`).
    this.eyeAt = new THREE.Vector3(0, top + WALL - 0.4, 0);
    this.beam = new THREE.Mesh(new THREE.CylinderGeometry(0.12, 0.7, 1, 24, 1, true),
      new THREE.MeshBasicMaterial({color: 0xc4d8ff, transparent: true, opacity: 0, blending: THREE.AdditiveBlending,
        depthWrite: false, side: THREE.DoubleSide, fog: false}));
    this.beam.visible = false;
    scene.add(this.beam);
  }

  board() {
    const scene = this.scene, group = this.table = new THREE.Group();
    const light = new THREE.MeshStandardMaterial({color: 0xd9d0b8, roughness: 0.34, metalness: 0.02, map: stoneTexture("#e8e0cc", "rgba(120,104,80,0.35)", 11)});
    const dark = new THREE.MeshStandardMaterial({color: 0x595668, roughness: 0.3, metalness: 0.1, map: stoneTexture("#6d6a7c", "rgba(210,210,230,0.2)", 23)});
    const tile = new THREE.BoxGeometry(0.985, 0.14, 0.985);
    this.tiles = [];
    for (let sq = 0; sq < 64; sq++) {
      const shade = ((sq & 7) + (sq >> 3)) & 1;
      const mesh = new THREE.Mesh(tile, shade ? light : dark);
      mesh.position.set(sqX(sq), -0.07, sqZ(sq));
      mesh.receiveShadow = true;
      mesh.userData.square = sq;
      group.add(mesh);
      this.tiles.push(mesh);
    }
    // The slab the squares are set in, and the plinth under it.
    const rim = new THREE.MeshStandardMaterial({color: 0x1c1a20, roughness: 0.5, metalness: 0.25, map: stoneTexture("#3c3a44", "rgba(215,182,106,0.2)", 41)});
    const slab = new THREE.Mesh(new THREE.BoxGeometry(9.4, 0.24, 9.4), rim);
    slab.position.y = -0.14;
    slab.receiveShadow = slab.castShadow = true;
    const plinth = new THREE.Mesh(new THREE.BoxGeometry(10.4, 0.5, 10.4), new THREE.MeshStandardMaterial({color: 0x121116, roughness: 0.8}));
    plinth.position.y = -0.4;
    plinth.receiveShadow = plinth.castShadow = true;
    const inlay = new THREE.Mesh(new THREE.BoxGeometry(8.16, 0.02, 8.16), new THREE.MeshStandardMaterial({color: 0xb08d4a, roughness: 0.35, metalness: 0.8}));
    inlay.position.y = -0.025;
    group.add(slab, plinth, inlay);
    this.labels = new THREE.Group();
    group.add(this.labels);
    scene.add(group);
  }

  // The names of the files and ranks along the rim, set to be read from the
  // side the player sits on.
  letter(side) {
    this.labels.clear();
    const turned = side === "b";
    const mark = (text, x, z) => {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 96;
      const pen = canvas.getContext("2d");
      pen.fillStyle = "rgba(226,198,130,0.92)";
      pen.font = "600 62px Georgia, serif";
      pen.textAlign = "center";
      pen.textBaseline = "middle";
      pen.fillText(text, 48, 52);
      const texture = new THREE.CanvasTexture(canvas);
      texture.colorSpace = THREE.SRGBColorSpace;
      const plate = new THREE.Mesh(new THREE.PlaneGeometry(0.42, 0.42), new THREE.MeshBasicMaterial({map: texture, transparent: true, depthWrite: false}));
      plate.rotation.x = -Math.PI / 2;
      plate.rotation.z = turned ? Math.PI : 0;
      plate.position.set(x, -0.012, z);
      this.labels.add(plate);
    };
    for (let at = 0; at < 8; at++) {
      mark("abcdefgh"[at], at - 3.5, turned ? -4.37 : 4.37);
      mark(String(at + 1), turned ? 4.37 : -4.37, 3.5 - at);
    }
  }

  forms() {
    const turned = {};
    for (const [kind, profile] of Object.entries(PROFILES)) {
      const form = new THREE.LatheGeometry(profile.map(([r, y]) => new THREE.Vector2(r, y)), 36);
      form.computeVertexNormals();
      turned[kind] = form;
    }
    this.geometry = {turned, head: knightHead(), merlon: new THREE.BoxGeometry(0.12, 0.1, 0.08), bead: new THREE.SphereGeometry(0.045, 10, 8),
      orb: new THREE.SphereGeometry(0.07, 12, 10), upright: new THREE.BoxGeometry(0.07, 0.3, 0.07), bar: new THREE.BoxGeometry(0.22, 0.07, 0.07),
      ring: new THREE.TorusGeometry(0.36, 0.014, 6, 40), chunk: new THREE.DodecahedronGeometry(0.09, 0),
      // What a ridden knight wears: a cloth over its mane, and its rider.
      cloth: new THREE.BoxGeometry(0.2, 0.36, 0.34), trim: new THREE.BoxGeometry(0.21, 0.035, 0.35),
      cloak: new THREE.ConeGeometry(0.13, 0.5, 14), hood: new THREE.SphereGeometry(0.075, 14, 10),
      // And what makes it a living thing to the one on its back: a mane, two
      // ears that turn, an eye, a bit and the reins to the rider's hands.
      tuft: new THREE.ConeGeometry(0.03, 0.075, 5), ear: new THREE.ConeGeometry(0.04, 0.16, 4),
      eye: new THREE.SphereGeometry(0.032, 12, 8), bit: new THREE.TorusGeometry(0.035, 0.009, 6, 16),
      // The rider's own hands, gauntleted, and their forearms: what a rider
      // sees of themself.
      fist: new THREE.SphereGeometry(0.034, 12, 9), cuff: new THREE.CylinderGeometry(0.03, 0.026, 0.05, 12, 1, true),
      forearm: new THREE.CylinderGeometry(0.022, 0.027, 1, 10)};
    this.material = {
      w: new THREE.MeshStandardMaterial({color: 0xe6dcc6, roughness: 0.36, metalness: 0.02, map: stoneTexture("#f1ead9", "rgba(140,120,90,0.3)", 5)}),
      b: new THREE.MeshStandardMaterial({color: 0x3f3b4c, roughness: 0.22, metalness: 0.55, map: stoneTexture("#8a8598", "rgba(160,170,255,0.25)", 9)}),
    };
    // The light a piece stands in: cold for White's, ember for Black's.
    this.rune = {w: 0x8fd8ff, b: 0xff8a3c};
    this.cloth = {
      w: new THREE.MeshStandardMaterial({color: 0x1d3c78, roughness: 0.85, emissive: 0x0a1630, emissiveIntensity: 0.4}),
      b: new THREE.MeshStandardMaterial({color: 0x6e1c16, roughness: 0.85, emissive: 0x2a0806, emissiveIntensity: 0.4}),
      gold: new THREE.MeshStandardMaterial({color: 0xc9a04c, roughness: 0.35, metalness: 0.85}),
      cloak: new THREE.MeshStandardMaterial({color: 0x23201f, roughness: 0.95}),
      leather: new THREE.MeshStandardMaterial({color: 0x3b2416, roughness: 0.7}),
      eye: {w: new THREE.MeshBasicMaterial({color: 0x9fe4ff}), b: new THREE.MeshBasicMaterial({color: 0xffa25a})},
    };
  }

  form(piece) {
    const kind = piece.toLowerCase(), colour = colourOf(piece), stone = this.material[colour], made = this.geometry;
    const root = new THREE.Group(), body = new THREE.Group();
    root.add(body);
    const add = (geometry, x = 0, y = 0, z = 0) => {
      const mesh = new THREE.Mesh(geometry, stone);
      mesh.position.set(x, y, z);
      mesh.castShadow = mesh.receiveShadow = true;
      body.add(mesh);
      return mesh;
    };
    add(made.turned[kind]);
    if (kind === "r") for (let at = 0; at < 6; at++) {
      const merlon = add(made.merlon, Math.cos(at / 6 * TAU) * 0.25, 1.07, Math.sin(at / 6 * TAU) * 0.25);
      merlon.rotation.y = -at / 6 * TAU + Math.PI / 2;
    }
    if (kind === "n") add(made.head);
    if (kind === "b") add(made.bead, 0, 1.385, 0);
    if (kind === "q") {
      for (let at = 0; at < 8; at++) add(made.bead, Math.cos(at / 8 * TAU) * 0.235, 1.53, Math.sin(at / 8 * TAU) * 0.235);
      add(made.orb, 0, 1.55, 0);
    }
    if (kind === "k") { add(made.upright, 0, 1.84, 0); add(made.bar, 0, 1.88, 0); }
    // A knight looks at the enemy: White's toward the far side. `face`
    // turns it a little toward the middle of the board, which is how a
    // head cut from a slab is read from the seat behind it.
    root.userData.facing = colour === "w" ? Math.PI / 2 : -Math.PI / 2;
    body.rotation.y = root.userData.facing;
    const ring = new THREE.Mesh(made.ring, new THREE.MeshBasicMaterial({color: this.rune[colour], transparent: true, opacity: 0.35,
      blending: THREE.AdditiveBlending, depthWrite: false}));
    ring.rotation.x = Math.PI / 2;
    ring.position.y = 0.03;
    root.add(ring);
    root.userData = {...root.userData, piece, kind, colour, ring, body, awake: 0};
    root.traverse(part => { part.userData.owner = root; });
    return root;
  }

  // The pieces as a position has them, set down with no ceremony: a new
  // game, a move taken back, a game come back to.
  set(position) {
    for (const held of this.pieces.values()) this.scene.remove(held);
    this.pieces.clear();
    this.ridden = null;
    this.moving = null;
    this.doomed = null;
    this.mood.blaze = 0;
    this.mood.spot = null;
    this.era += 1;
    position.board.forEach((piece, sq) => {
      if (!piece) return;
      const held = this.form(piece);
      held.position.copy(sqPoint(sq));
      held.userData.square = sq;
      this.face(held);
      this.scene.add(held);
      this.pieces.set(sq, held);
    });
  }

  face(held) {
    if (held.userData.kind === "n" || held === this.ridden) held.userData.body.rotation.y = this.rest(held);
  }
  // Where a knight looks when nothing calls it: a ridden knight where its
  // rider does, straight at the enemy; the others a little in toward the
  // middle of the board.
  rest(held) {
    if (held === this.ridden || held.userData.kind !== "n") return held.userData.facing;
    const inward = sqX(held.userData.square) < 0 ? -1 : 1, side = held.userData.colour === "w" ? 1 : -1;
    return held.userData.facing + inward * side * 0.95;
  }

  // The knight the player rides: it wears a cloth over its mane in its
  // side's colour, and its rider sits behind its ears (where the seat's eye
  // is, `Seat.rider`). `null` is nobody riding.
  ride(held) {
    if (this.ridden && this.ridden !== held) this.undress(this.ridden);
    this.ridden = held || null;
    if (!held) return;
    this.dress(held);
    this.face(held);
  }
  undress(held) {
    if (!held.userData.saddle) return;
    held.userData.body.remove(held.userData.saddle);
    held.userData.saddle = null;
  }
  // A piece with its rider on it, ridden by the seat or not: with two
  // players at one board each sees the other's rider on their piece.
  dress(held) {
    if (!held.userData.saddle && held.userData.kind !== "n") {
      // Any other piece is ridden standing on top of it: on a pawn's head, in
      // a rook's battlements, on a bishop's mitre, in a queen's or a king's
      // crown. Its rider is seen there from the board.
      const made = this.geometry, saddle = new THREE.Group(), rider = new THREE.Group();
      const cloak = new THREE.Mesh(made.cloak, this.cloth.cloak), hood = new THREE.Mesh(made.hood, this.cloth.cloak);
      cloak.position.set(0, 0.25, 0);
      hood.position.set(0.02, 0.54, 0);
      for (const mesh of [cloak, hood]) { mesh.castShadow = true; rider.add(mesh); }
      rider.position.set(0, RIDER_TOPS[held.userData.kind], 0);
      rider.scale.setScalar(0.85);
      saddle.add(rider);
      saddle.userData = {rider, ears: [], eyes: []};
      // Their hands held out before them, braced, low enough to be out of the
      // way and high enough to be seen; arms from the shoulders below the eye.
      const top = RIDER_TOPS[held.userData.kind];
      saddle.userData.hands = this.arms(saddle, [-1, 1].map(side => [
        new THREE.Vector3(0.02, top + 0.3, side * 0.14), new THREE.Vector3(0.4, top + 0.21, side * 0.15)]));
      saddle.traverse(mesh => { if (mesh !== saddle) mesh.userData.owner = held; });
      held.userData.body.add(saddle);
      held.userData.saddle = saddle;
    }
    if (!held.userData.saddle) {
      const made = this.geometry, saddle = new THREE.Group(), colour = held.userData.colour;
      const part = (geometry, material, x, y, z, tilt = 0) => {
        const mesh = new THREE.Mesh(geometry, material);
        mesh.position.set(x, y, z);
        mesh.rotation.z = tilt;
        mesh.castShadow = true;
        saddle.add(mesh);
        return mesh;
      };
      // Over the back of the neck, along the line of the mane.
      part(made.cloth, this.cloth[colour], -0.25, 0.86, 0, -0.38);
      part(made.trim, this.cloth.gold, -0.18, 0.69, 0, -0.38);
      // The mane, from the cloth up to between the ears, pointing back.
      [[-0.235, 0.98], [-0.2, 1.07], [-0.165, 1.15], [-0.135, 1.23], [-0.115, 1.3]].forEach(([x, y], at) => {
        part(made.tuft, this.material[colour], x + 0.005, y, at % 2 ? 0.015 : -0.015, 1.35).scale.set(1, 1, 0.35);
      });
      // Ears, each on a pivot so that it can turn toward what it hears.
      const ears = [-1, 1].map(side => {
        const pivot = new THREE.Group();
        pivot.position.set(-0.07, 1.31, side * 0.055);
        const ear = new THREE.Mesh(made.ear, this.material[colour]);
        ear.position.y = 0.075;
        ear.scale.set(1, 1, 0.45);
        ear.castShadow = true;
        pivot.add(ear);
        pivot.rotation.z = 0.25;
        saddle.add(pivot);
        return pivot;
      });
      // An eye on each side, lit from inside with its side's light.
      const eyes = [-1, 1].map(side => part(made.eye, this.cloth.eye[colour], 0.19, 1.02, side * 0.112));
      // The bit, and the reins from it to the rider's hands.
      const hands = new THREE.Vector3(-0.08, 1.1, 0.34);
      for (const side of [-1, 1]) {
        const bit = part(made.bit, this.cloth.gold, 0.33, 0.74, side * 0.115);
        bit.rotation.y = Math.PI / 2;
        const from = new THREE.Vector3(0.33, 0.74, side * 0.115), hand = hands.clone().add(new THREE.Vector3(0, 0, side * 0.05));
        const sag = from.clone().lerp(hand, 0.5).add(new THREE.Vector3(0, -0.12, 0));
        const rein = new THREE.Mesh(new THREE.TubeGeometry(new THREE.CatmullRomCurve3([from, sag, hand]), 18, 0.009, 5), this.cloth.leather);
        rein.castShadow = true;
        saddle.add(rein);
      }
      saddle.userData.ears = ears;
      saddle.userData.eyes = eyes;
      // Holding the reins: a fist at the end of each, the arms back to the
      // rider's shoulders, below their eye.
      saddle.userData.hands = this.arms(saddle, [-1, 1].map(side => [
        new THREE.Vector3(EYE.x + 0.03, EYE.y - 0.24, EYE.z + side * 0.13), hands.clone().add(new THREE.Vector3(0, 0, side * 0.05))]));
      const rider = new THREE.Group();
      const cloak = new THREE.Mesh(made.cloak, this.cloth.cloak);
      cloak.position.set(0, 0.25, 0);
      const hood = new THREE.Mesh(made.hood, this.cloth.cloak);
      hood.position.set(0.02, 0.54, 0);
      for (const mesh of [cloak, hood]) { mesh.castShadow = true; rider.add(mesh); }
      rider.position.set(-0.42, 0.7, 0.36);
      saddle.add(rider);
      saddle.userData.rider = rider;
      saddle.traverse(mesh => { mesh.userData.owner = held; });
      held.userData.body.add(saddle);
      held.userData.saddle = saddle;
    }
  }

  // The rider's two arms in the frame of the piece they ride, each from a
  // shoulder to a fist: a gauntlet, its cuff, and the forearm between.
  arms(saddle, pairs) {
    const made = this.geometry, leather = this.cloth.leather, gold = this.cloth.gold;
    return pairs.map(([shoulder, hand]) => {
      const fist = new THREE.Mesh(made.fist, leather), cuff = new THREE.Mesh(made.cuff, gold), forearm = new THREE.Mesh(made.forearm, this.cloth.cloak);
      fist.scale.set(1.25, 0.85, 1);
      for (const mesh of [fist, cuff, forearm]) { mesh.castShadow = true; saddle.add(mesh); }
      const arm = {fist, cuff, forearm, shoulder, hand, grip: 0};
      this.reach(arm);
      return arm;
    });
  }
  // An arm laid from its shoulder to its fist, the fist drawn back and
  // clenched by how hard it grips.
  reach(arm) {
    const at = arm.hand.clone().lerp(arm.shoulder, 0.12 * arm.grip).add(new THREE.Vector3(0, 0.02 * arm.grip, 0));
    const way = at.clone().sub(arm.shoulder), long = way.length(), along = way.clone().normalize();
    arm.fist.position.copy(at);
    arm.fist.scale.set(1.25 - 0.2 * arm.grip, 0.85, 1 - 0.1 * arm.grip);
    arm.cuff.position.copy(at).addScaledVector(along, -0.045);
    arm.cuff.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), along);
    arm.forearm.position.copy(arm.shoulder).addScaledVector(along, (long - 0.06) / 2);
    arm.forearm.scale.set(1, Math.max(0.05, long - 0.06), 1);
    arm.forearm.quaternion.copy(arm.cuff.quaternion);
  }

  // The pieces that could take the knight ridden: their light turns red.
  menace(squares) {
    for (const held of this.pieces.values()) {
      const data = held.userData, threat = squares.includes(data.square);
      if (threat === !!data.menace) continue;
      data.menace = threat;
      data.ring.material.color.set(threat ? 0xff3b2b : this.rune[data.colour]);
    }
  }

  // What the stone reflects: a dark room with the lamps in it, drawn once.
  // Polished black stone with nothing to reflect is a black hole.
  surround(renderer) {
    const room = new THREE.Scene();
    room.add(new THREE.Mesh(new THREE.SphereGeometry(40, 24, 16), new THREE.MeshBasicMaterial({side: THREE.BackSide, color: 0x0b0b12})));
    const lamp = (colour, strength, x, y, z, size) => {
      const pane = new THREE.Mesh(new THREE.PlaneGeometry(size, size), new THREE.MeshBasicMaterial({color: new THREE.Color(colour).multiplyScalar(strength), side: THREE.DoubleSide}));
      pane.position.set(x, y, z);
      pane.lookAt(0, 0, 0);
      room.add(pane);
    };
    lamp(0xffe6c4, 7, -14, 22, 12, 16);
    lamp(0x6f93ff, 3, 14, 10, -20, 14);
    lamp(0xff8a3a, 9, -16, 5, -14, 5);
    lamp(0xff8a3a, 9, 16, 5, -14, 5);
    lamp(0xff8a3a, 6, -22, 5, 3, 5);
    lamp(0xff8a3a, 6, 22, 5, 3, 5);
    lamp(0x8898c0, 0.8, 0, 30, 0, 40);
    const baker = new THREE.PMREMGenerator(renderer);
    this.scene.environment = baker.fromScene(room, 0.03).texture;
    baker.dispose();
  }

  marks() {
    // What is laid on a square to say something about it.
    const lay = (colour, opacity) => {
      const mesh = new THREE.Mesh(new THREE.PlaneGeometry(0.96, 0.96), new THREE.MeshBasicMaterial({color: colour, transparent: true, opacity,
        blending: THREE.AdditiveBlending, depthWrite: false}));
      mesh.rotation.x = -Math.PI / 2;
      mesh.position.y = 0.006;
      mesh.visible = false;
      this.scene.add(mesh);
      return mesh;
    };
    this.mark = {from: lay(0xffb648, 0.13), to: lay(0xffb648, 0.2), chosen: lay(0x6fc8ff, 0.3), check: lay(0xff3b2b, 0.6), hover: lay(0xffffff, 0.1)};
    this.dots = [];
    for (let at = 0; at < 30; at++) {
      const dot = new THREE.Mesh(new THREE.CircleGeometry(0.15, 24), new THREE.MeshBasicMaterial({color: 0x9fe0ff, transparent: true, opacity: 0.75,
        blending: THREE.AdditiveBlending, depthWrite: false}));
      const ring = new THREE.Mesh(new THREE.RingGeometry(0.38, 0.47, 32), new THREE.MeshBasicMaterial({color: 0xff9a6b, transparent: true, opacity: 0.85,
        blending: THREE.AdditiveBlending, depthWrite: false}));
      for (const mesh of [dot, ring]) { mesh.rotation.x = -Math.PI / 2; mesh.position.y = 0.008; mesh.visible = false; this.scene.add(mesh); }
      this.dots.push({dot, ring});
    }
  }

  show(name, sq) {
    const mesh = this.mark[name];
    mesh.visible = sq !== null && sq !== undefined && sq >= 0;
    if (mesh.visible) mesh.position.set(sqX(sq), mesh.position.y, sqZ(sq));
  }

  // Where a chosen piece may go: a dot on an empty square, a ring round a
  // piece it would take.
  offer(moves) {
    this.dots.forEach((held, at) => {
      const move = moves[at];
      held.dot.visible = !!move && !move.captured;
      held.ring.visible = !!move && !!move.captured;
      if (move) for (const mesh of [held.dot, held.ring]) mesh.position.set(sqX(move.to), mesh.position.y, sqZ(move.to));
    });
  }

  wake(sq) {
    for (const held of this.pieces.values()) held.userData.awake = held.userData.square === sq ? 1 : 0;
  }

  // ---- a move, played out
  //
  // `hooks` is how the seat hears of it: `sound(name, strength, at, kind)`, and
  // `look(point)` / `look(null)` when there is something to lean in for.
  async play(move, hooks) {
    const mover = this.pieces.get(move.from), kind = mover.userData.kind, era = this.era;
    // Set down afresh while this was being played out: it is not this board's.
    const gone = () => { if (this.era === era) return false; hooks.moving(null); return true; };
    const to = sqPoint(move.to);
    const victimAt = move.flag === "ep" ? move.to + (mover.userData.colour === "w" ? -8 : 8) : move.to;
    const victim = move.captured ? this.pieces.get(victimAt) : null;
    this.pieces.delete(move.from);
    mover.userData.awake = 1;
    hooks.moving(mover);
    this.moving = mover;
    // A knight with a rider turns to where it is going before it goes, and
    // the stone groans under them as it turns.
    if (mover === this.ridden) await this.turnTo(mover, to.clone().sub(mover.position), hooks);
    if (gone()) return;
    if (victim) {
      this.pieces.delete(victimAt);
      // What is about to be taken trembles until it breaks.
      this.doomed = victim;
      hooks.look(victim.position.clone().lerp(mover.position, 0.3));
      await this.strike(mover, victim, to, hooks);
    } else if (kind === "n") {
      await this.leap(mover, to, 1.25, hooks);
    } else {
      await this.glide(mover, to, hooks);
    }
    if (gone()) return;
    mover.userData.square = move.to;
    this.pieces.set(move.to, mover);
    if (kind === "n" || mover === this.ridden) {
      const was = mover.userData.body.rotation.y;
      this.face(mover);
      const now = mover.userData.body.rotation.y;
      tween(0.3, t => { mover.userData.body.rotation.y = was + (now - was) * t; });
    }
    if (move.flag === "castle-k" || move.flag === "castle-q") {
      // The rook comes round the king, over its head.
      const short = move.flag === "castle-k", rookFrom = short ? move.to + 1 : move.to - 2, rookTo = short ? move.to - 1 : move.to + 1;
      const rook = this.pieces.get(rookFrom);
      this.pieces.delete(rookFrom);
      await this.leap(rook, sqPoint(rookTo), 1.5, hooks);
      if (gone()) return;
      rook.userData.square = rookTo;
      this.pieces.set(rookTo, rook);
    }
    if (move.promo) await this.crown(mover, move, hooks);
    if (gone()) return;
    mover.userData.awake = 0;
    this.moving = null;
    hooks.moving(null);
    hooks.look(null);
  }

  turnTo(piece, way, hooks) {
    const body = piece.userData.body, was = body.rotation.y;
    let want = Math.atan2(-way.z, way.x), by = want - was;
    by = ((by + Math.PI) % TAU + TAU) % TAU - Math.PI;
    if (Math.abs(by) < 0.05) return Promise.resolve();
    if (hooks) hooks.sound("groan", Math.min(1.2, 0.5 + Math.abs(by) * 0.4), piece.position, "n");
    return tween(0.22 + Math.abs(by) * 0.08, t => { body.rotation.y = was + by * t; });
  }

  // A knight rears as it leaps and comes down nose first.
  rear(piece, heading, t) {
    if (piece.userData.kind === "n") this.lean(piece, heading, -0.24 * Math.cos(t * Math.PI));
  }
  async settle(piece) {
    if (piece.userData.kind !== "n") return;
    const tilt = piece.quaternion.clone(), level = new THREE.Quaternion();
    await tween(0.16, t => piece.quaternion.slerpQuaternions(tilt, level, t), ease.out);
    piece.quaternion.identity();
  }

  lean(piece, toward, angle) {
    const axis = new THREE.Vector3(toward.z, 0, -toward.x);
    if (axis.lengthSq() < 1e-6) { piece.quaternion.identity(); return; }
    piece.quaternion.setFromAxisAngle(axis.normalize(), angle);
  }

  async glide(piece, to, hooks, pace = 1) {
    const from = piece.position.clone(), way = to.clone().sub(from), far = way.length();
    if (far < 1e-4) return;
    const heading = way.clone().normalize();
    hooks.sound("slide", Math.min(1, 0.4 + far * 0.12), from, piece.userData.kind);
    await tween((0.3 + far * 0.13) / pace, t => {
      piece.position.lerpVectors(from, to, t);
      // It tips into the move and rights itself as it arrives.
      this.lean(piece, heading, Math.sin(t * Math.PI) * 0.11);
    });
    piece.quaternion.identity();
    hooks.sound("set", 0.5, to, piece.userData.kind);
    this.dust(to, 0.5);
  }

  async leap(piece, to, high, hooks) {
    const from = piece.position.clone(), far = from.distanceTo(to), heading = to.clone().sub(from).normalize();
    hooks.sound("lift", 0.6, from, piece.userData.kind);
    await tween(0.42 + far * 0.07, t => {
      piece.position.lerpVectors(from, to, t);
      piece.position.y = Math.sin(t * Math.PI) * high;
      this.rear(piece, heading, t);
    });
    piece.position.copy(to);
    hooks.sound("set", piece === this.ridden ? 1.3 : 0.9, to, piece.userData.kind);
    hooks.shake(piece === this.ridden ? 0.12 : 0.05);
    this.dust(to, 0.8);
    // The knight ridden breathes out as it lands: a puff at its nostrils.
    if (piece === this.ridden) {
      const body = piece.userData.body;
      body.updateWorldMatrix(true, false);
      const nose = body.localToWorld(new THREE.Vector3(0.44, 0.68, 0));
      this.dust(nose, 0.3, true);
      hooks.sound("snort", 1, nose, "n");
    }
    await this.settle(piece);
  }

  // A capture: how the piece that takes comes at the one it takes is its
  // own. A pawn and a king lunge. A rook rams. A bishop comes in spinning.
  // A knight comes down on it. A queen does not touch it at all.
  async strike(mover, victim, to, hooks) {
    const kind = mover.userData.kind, from = mover.position.clone(), prey = victim.position.clone();
    const way = prey.clone().sub(from), far = way.length(), heading = way.clone().normalize();
    const near = prey.clone().addScaledVector(heading, -0.82);
    if (kind === "n") {
      hooks.sound("lift", 0.7, from, kind);
      await tween(0.5 + far * 0.06, t => {
        mover.position.lerpVectors(from, prey, t);
        mover.position.y = Math.sin(t * Math.PI) * 2.1;
        this.rear(mover, heading, t);
      }, ease.in);
      this.shatter(victim, heading, hooks, 1.1);
    } else if (kind === "q") {
      // She rises where she stands, and what she looks at breaks.
      hooks.sound("lift", 0.5, from, kind);
      await tween(0.32, t => { mover.position.y = t * 0.45; mover.userData.awake = 1 + t; });
      const bolt = new THREE.Sprite(new THREE.SpriteMaterial({map: this.glow, color: this.rune[mover.userData.colour], blending: THREE.AdditiveBlending, depthWrite: false, transparent: true}));
      bolt.scale.setScalar(0.9);
      this.scene.add(bolt);
      const start = mover.position.clone().setY(1.5), end = prey.clone().setY(0.8);
      hooks.sound("bolt", 0.9, start, kind);
      await tween(0.2 + far * 0.02, t => { bolt.position.lerpVectors(start, end, t); bolt.scale.setScalar(0.9 + t * 0.9); }, ease.in);
      this.scene.remove(bolt);
      this.shatter(victim, heading, hooks, 1.2);
      await wait(0.22);
      await tween(0.24 + far * 0.1, t => { mover.position.lerpVectors(from, to, t); mover.position.y = (1 - t) * 0.45; }, ease.inOut);
      hooks.sound("set", 0.6, to, kind);
    } else if (kind === "r") {
      // No stopping: it gathers itself and goes through.
      await tween(0.2, t => this.lean(mover, heading, -0.16 * t));
      hooks.sound("slide", 1, from, kind);
      await tween(0.12 + far * 0.07, t => { mover.position.lerpVectors(from, near, t); this.lean(mover, heading, -0.16 + 0.34 * t); }, ease.in);
      this.shatter(victim, heading, hooks, 1.25);
    } else {
      if (far > 1.1) await this.glide(mover, near, hooks, 1.15);
      const stood = mover.position.clone();
      const spin = kind === "b" ? TAU : 0;
      // Back, and then forward into it.
      await tween(0.2, t => { this.lean(mover, heading, -0.32 * t); mover.position.lerpVectors(stood, stood.clone().addScaledVector(heading, -0.12), t); }, ease.out);
      hooks.sound("lift", 0.5, stood, kind);
      const back = mover.position.clone(), reach = stood.clone().addScaledVector(heading, 0.42);
      await tween(0.11, t => {
        mover.position.lerpVectors(back, reach, t);
        this.lean(mover, heading, -0.32 + 0.78 * t);
      }, ease.in);
      if (spin) {
        const facing = mover.userData.body.rotation.y;
        tween(0.3, t => { mover.userData.body.rotation.y = facing + spin * t; }, ease.out);
      }
      this.shatter(victim, heading, hooks, kind === "k" ? 1.15 : 0.9);
    }
    const struck = mover.position.clone();
    const tilt = mover.quaternion.clone(), level = new THREE.Quaternion();
    await tween(0.3, t => { mover.position.lerpVectors(struck, to, t); mover.quaternion.slerpQuaternions(tilt, level, t); }, ease.out);
    mover.position.copy(to);
    mover.quaternion.identity();
  }

  // A piece broken: it is gone, and what it was made of flies.
  shatter(victim, heading, hooks, force = 1) {
    this.scene.remove(victim);
    if (this.doomed === victim) this.doomed = null;
    this.mood.flare = 1;
    this.mood.falls += 1;
    // The knight ridden is broken under its rider, who is thrown on the way
    // the blow came.
    if (victim === this.ridden) {
      this.ridden = null;
      hooks.thrown(victim.position.clone(), heading.clone(), victim.userData.colour, victim.userData.kind);
    }
    const {kind, colour} = victim.userData, tall = HEIGHTS[kind], count = Math.round(26 + tall * 22);
    const stones = new THREE.InstancedMesh(this.geometry.chunk, this.material[colour], count);
    stones.castShadow = true;
    const bits = [];
    for (let at = 0; at < count; at++) {
      const up = Math.random() * tall, round = Math.random() * TAU, out = Math.random() * 0.22;
      const spot = victim.position.clone().add(new THREE.Vector3(Math.cos(round) * out, up, Math.sin(round) * out));
      const fling = new THREE.Vector3(Math.cos(round), 0, Math.sin(round)).multiplyScalar(0.8 + Math.random() * 2.2)
        .addScaledVector(heading, 1.6 + Math.random() * 2.4).multiplyScalar(force);
      fling.y = (1.2 + Math.random() * 3.4 + up * 0.8) * force;
      bits.push({spot, fling, turn: new THREE.Euler(Math.random() * 3, Math.random() * 3, Math.random() * 3),
        spin: new THREE.Vector3((Math.random() - 0.5) * 12, (Math.random() - 0.5) * 12, (Math.random() - 0.5) * 12),
        size: 0.5 + Math.random() * 1.5});
    }
    this.scene.add(stones);
    this.debris.push({stones, bits, age: 0});
    this.dust(victim.position, 1.6);
    this.flash.position.copy(victim.position).setY(1);
    this.flash.intensity = 60 * force;
    hooks.sound("shatter", force, victim.position, kind);
    hooks.shake(0.16 * force);
    hooks.impact(victim.position.clone(), force);
    // A blow beside the knight ridden rocks it, and its rider.
    if (this.ridden && victim !== this.ridden) this.shudder(victim.position, force);
  }

  // The knight ridden rocked by a blow at `at`: thrown away from it, and back,
  // the more the nearer (`regard` plays it out).
  shudder(at, force = 1) {
    const prey = this.ridden;
    if (!prey) return;
    const away = prey.position.clone().sub(at).setY(0), far = away.length();
    if (far > 2.4) return;
    const much = 0.16 * force * (1 - far / 2.4);
    prey.userData.jolt = {away: far > 1e-3 ? away.normalize() : new THREE.Vector3(0, 0, 1), much: Math.max(much, prey.userData.jolt ? prey.userData.jolt.much : 0), age: 0};
  }

  // Dust kicked up off the board, or (`aloft`) a breath at a point in the air.
  dust(at, much, aloft = false) {
    const count = Math.round(14 * much), spots = new Float32Array(count * 3), drift = [];
    for (let i = 0; i < count; i++) {
      const round = Math.random() * TAU, out = (aloft ? 0.02 : 0.1) + Math.random() * (aloft ? 0.05 : 0.25);
      spots.set([at.x + Math.cos(round) * out, (aloft ? at.y : 0.05) + Math.random() * 0.2 * much, at.z + Math.sin(round) * out], i * 3);
      drift.push([Math.cos(round) * (0.3 + Math.random() * 0.8) * much, 0.2 + Math.random() * 0.5 * much, Math.sin(round) * (0.3 + Math.random() * 0.8) * much]);
    }
    const cloud = new THREE.BufferGeometry();
    cloud.setAttribute("position", new THREE.BufferAttribute(spots, 3));
    const puff = new THREE.Points(cloud, new THREE.PointsMaterial({map: this.glow, size: 0.5 * Math.sqrt(much), color: 0xb9ad98, transparent: true, opacity: 0.32, depthWrite: false}));
    this.scene.add(puff);
    tween(0.9 + much * 0.4, t => {
      const held = cloud.attributes.position;
      for (let i = 0; i < count; i++) held.setXYZ(i, held.getX(i) + drift[i][0] * 0.016, held.getY(i) + drift[i][1] * 0.016, held.getZ(i) + drift[i][2] * 0.016);
      held.needsUpdate = true;
      puff.material.opacity = 0.32 * (1 - t);
    }, ease.line).then(() => { this.scene.remove(puff); cloud.dispose(); puff.material.dispose(); });
  }

  // A pawn that has crossed the board becomes what it was told to.
  async crown(pawn, move, hooks) {
    const at = pawn.position.clone();
    const flare = new THREE.Sprite(new THREE.SpriteMaterial({map: this.glow, color: 0xfff2c2, blending: THREE.AdditiveBlending, depthWrite: false, transparent: true}));
    flare.position.copy(at).setY(0.9);
    this.scene.add(flare);
    hooks.sound("crown", 1, at, move.promo.toLowerCase());
    hooks.look(at);
    const era = this.era;
    await tween(0.35, t => { flare.scale.setScalar(0.2 + t * 4); pawn.scale.setScalar(1 - t * 0.7); });
    this.scene.remove(pawn);
    if (this.era !== era) { this.scene.remove(flare); return; }
    const made = this.form(move.promo);
    made.position.copy(at);
    made.userData.square = move.to;
    made.scale.setScalar(0.3);
    this.scene.add(made);
    this.pieces.set(move.to, made);
    // A pawn ridden across the board carries its rider up onto what it becomes.
    if (pawn === this.ridden) {
      this.ridden = null;
      this.ride(made);
      hooks.carried(made);
    }
    await tween(0.45, t => { made.scale.setScalar(0.3 + 0.7 * t); flare.scale.setScalar(4.2 * (1 - t)); flare.material.opacity = 1 - t; }, ease.out);
    this.scene.remove(flare);
    this.dust(at, 1);
  }

  // A king that has been mated falls.
  async fall(colour, hooks) {
    const king = [...this.pieces.values()].find(held => held.userData.kind === "k" && held.userData.colour === colour);
    if (!king) return;
    hooks.look(king.position.clone());
    await wait(0.5);
    const away = new THREE.Vector3(king.position.x >= 0 ? 1 : -1, 0, colour === "w" ? 0.4 : -0.4).normalize();
    const pivot = king.position.clone();
    hooks.sound("slide", 0.8, pivot, "k");
    await tween(0.75, t => {
      const angle = t * (Math.PI / 2 - 0.08);
      this.lean(king, away, angle);
      // It turns about the edge of its foot, not its middle.
      king.position.copy(pivot).addScaledVector(away, Math.sin(angle) * 0.3).setY(Math.sin(angle) * 0.34);
    }, ease.in);
    hooks.sound("shatter", 0.7, king.position, "k");
    hooks.shake(0.2);
    this.dust(king.position.clone().addScaledVector(away, 1.4), 1.6);
    await wait(0.6);
    hooks.look(null);
  }

  // How each piece standing holds itself, most pressing first:
  //   a piece that could take the knight ridden leans toward it, as if about
  //     to come, and a knight among them turns its head to it;
  //   a king in check flinches away from what checks it, shaking (`flinch`);
  //   every other piece watches the piece that is moving: a knight turns its
  //     head to it, the rest lean a little its way, less the further off.
  // The piece about to be taken trembles until it breaks (`doomed`). A piece
  // that is moving is left to its move, and the knight ridden to its rider.
  regard(dt) {
    const prey = this.ridden, moving = this.moving, ease = Math.min(1, dt * 4);
    for (const held of this.pieces.values()) {
      const data = held.userData;
      if (held === moving) { data.loom = 0; data.toward = null; continue; }
      let toward = null, want = 0, look = null;
      if (data.menace && prey) {
        toward = look = prey.position.clone().sub(held.position).setY(0);
        want = 0.13 + 0.035 * Math.sin(this.time * 5 + data.square);
      } else if (data.brace) {
        // Named as what is to be taken: it faces what is coming and braces.
        toward = data.brace.clone().negate();
        look = data.brace;
        want = 0.06;
      } else if (data.check) {
        toward = data.check;
        want = 0.075 + 0.03 * Math.sin(this.time * 23);
      } else if (moving && held !== prey) {
        const way = moving.position.clone().sub(held.position).setY(0), far = way.length();
        if (far > 0.4) { toward = look = way; want = 0.05 * Math.min(1, 2.5 / far); }
      } else if (this.musing === data.colour && held !== prey) {
        // Its side is thinking: it shifts its weight, slowly, its own way.
        const phase = this.time * (0.5 + (data.square % 5) * 0.09) + data.square * 1.7;
        toward = new THREE.Vector3(Math.sin(phase), 0, Math.cos(phase * 0.8));
        want = 0.025 + 0.015 * Math.sin(phase * 1.3);
      }
      if (toward) data.toward = toward;
      data.loom = (data.loom || 0) + (want - (data.loom || 0)) * ease;
      if (held !== prey && data.toward) {
        if (data.loom > 0.003) this.lean(held, data.toward, data.loom);
        else { held.quaternion.identity(); data.loom = 0; data.toward = null; }
      }
      if (data.kind === "n" && held !== prey) {
        const body = data.body, aim = look ? Math.atan2(-look.z, look.x) : this.rest(held);
        body.rotation.y += turnBy(body.rotation.y, aim) * Math.min(1, dt * 5);
      }
    }
    // The knight ridden rocking from a blow beside it: away, back past
    // upright, and still, in about a second.
    const jolt = prey && prey !== moving && prey.userData.jolt;
    if (jolt) {
      jolt.age += dt;
      const swing = jolt.much * Math.exp(-jolt.age * 4.5) * Math.cos(jolt.age * 14);
      if (jolt.age > 1.2) { prey.userData.jolt = null; prey.quaternion.identity(); }
      else this.lean(prey, jolt.away, swing);
    }
    if (this.doomed) {
      const shake = this.time * 37;
      this.lean(this.doomed, new THREE.Vector3(Math.sin(shake), 0, Math.cos(shake * 1.3)), 0.045);
    }
  }

  // The hall's light follows the game. The room darkens as the board empties
  // (`dusk`, the pieces still standing), so that the braziers and the shaft
  // stand out more the further it has gone; the shaft of light from the
  // dome's eye narrows onto a king in check (`spotlight`); and at a mate the
  // fires blaze and hold (`blaze`). A piece falling flares the fires.
  dusk(standing) { this.mood.gloomWant = Math.max(0, Math.min(1, (32 - standing) / 28)) * 0.55; }
  spotlight(sq) {
    const king = this.pieces.get(sq);
    this.mood.spot = king ? king.position.clone() : null;
  }
  blaze() { this.mood.blaze = 1; }

  // A side thinking: its pieces shift their weight, each a little and in its
  // own time, until it has chosen (`regard`). `null` is nobody thinking.
  muse(colour) {
    if (colour && this.musing !== colour) this.musings += 1;
    this.musing = colour || null;
  }

  // What the board hears while a command is being given: the pieces it could
  // be meant for stir (they lift, and their light brightens), and a piece it
  // would take turns to face them and braces (`regard`).
  stir(squares = [], prey = -1) {
    for (const held of this.pieces.values()) { held.userData.stir = squares.includes(held.userData.square); held.userData.brace = null; }
    const victim = this.pieces.get(prey);
    if (!victim || !squares.length) return;
    const toward = new THREE.Vector3();
    for (const sq of squares) toward.add(sqPoint(sq).sub(victim.position));
    if (toward.lengthSq() > 1e-6) victim.userData.brace = toward.setY(0).normalize();
  }

  // The king in check, and the pieces that check it: it flinches away from
  // them. `-1` is no king in check.
  flinch(sq, from = []) {
    for (const held of this.pieces.values()) held.userData.check = null;
    const king = this.pieces.get(sq);
    if (!king) return;
    const away = new THREE.Vector3();
    for (const at of from) away.add(king.position.clone().sub(sqPoint(at)).setY(0).normalize());
    if (away.lengthSq() < 1e-6) away.set(0, 0, king.userData.colour === "w" ? 1 : -1);
    king.userData.check = away.normalize();
  }

  // The knight ridden listens: its ears turn toward the piece that is
  // moving and prick up, and between moves they drift and twitch. Its eyes
  // close now and then.
  attend(dt) {
    const saddle = this.ridden && this.ridden.userData.saddle;
    if (!saddle) return;
    const body = this.ridden.userData.body;
    let turn = 0.22 * Math.sin(this.time * 0.37) * Math.sin(this.time * 1.3), prick = 0.25;
    if (this.moving && this.moving !== this.ridden) {
      body.updateWorldMatrix(true, false);
      const heard = body.worldToLocal(this.moving.position.clone().setY(1.2));
      turn = Math.max(-1.2, Math.min(1.2, Math.atan2(-heard.z, heard.x)));
      prick = -0.1;
    }
    const follow = Math.min(1, dt * 7);
    saddle.userData.ears.forEach((ear, at) => {
      ear.rotation.y += (turn * (at ? 1 : 0.85) - ear.rotation.y) * follow;
      ear.rotation.z += (prick - ear.rotation.z) * follow;
    });
    const shut = this.time % 4.7 < 0.13;
    for (const eye of saddle.userData.eyes) eye.scale.y = shut ? 0.12 : 1;
    // The rider's hands grip while their piece moves under them, and ease
    // when it stands.
    const hold = this.moving === this.ridden ? 1 : 0;
    for (const arm of saddle.userData.hands || []) {
      const was = arm.grip;
      arm.grip += (hold - arm.grip) * Math.min(1, dt * (hold ? 12 : 3));
      if (Math.abs(arm.grip - was) > 1e-4) this.reach(arm);
    }
  }

  // What goes on whether or not anyone moves.
  tick(dt) {
    this.time += dt;
    const mood = this.mood, follow = Math.min(1, dt * 0.8);
    mood.flare *= Math.pow(0.12, dt);
    mood.gloom += (mood.gloomWant - mood.gloom) * follow;
    mood.blazeNow += (mood.blaze - mood.blazeNow) * Math.min(1, dt * 1.5);
    const fire = 1 + mood.flare * 1.6 + mood.blazeNow * 1.5, blown = 1 + mood.flare * 0.45 + mood.blazeNow * 0.6;
    this.sky.intensity = 0.5 * (1 - mood.gloom * 0.7) * (1 + mood.blazeNow * 0.4);
    this.key.intensity = 2.6 * (1 - mood.gloom * 0.55) * (1 + mood.blazeNow * 0.25);
    // The shaft narrows and brightens onto a king in check, and opens again.
    const onto = mood.spot || new THREE.Vector3();
    this.moon.target.position.lerp(onto, Math.min(1, dt * 2.5));
    this.moon.angle += ((mood.spot ? 0.05 : 0.16) - this.moon.angle) * Math.min(1, dt * 2.5);
    this.moon.intensity += ((mood.spot ? 2600 : 420) * (1 + mood.blazeNow * 0.5) - this.moon.intensity) * Math.min(1, dt * 2.5);
    mood.spotNow += ((mood.spot ? 1 : 0) - mood.spotNow) * Math.min(1, dt * 2.5);
    this.beam.visible = mood.spotNow > 0.01;
    if (this.beam.visible) {
      const down = this.moon.target.position.clone().setY(0.2), up = this.eyeAt.clone().sub(down);
      this.beam.position.copy(down).addScaledVector(up, 0.5);
      this.beam.scale.set(1, up.length(), 1);
      this.beam.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), up.normalize());
      this.beam.material.opacity = 0.13 * mood.spotNow;
    }
    for (const {flame, lamp, phase} of this.flames) {
      const flicker = 0.86 + 0.1 * Math.sin(this.time * 9 + phase) + 0.07 * Math.sin(this.time * 23.7 + phase * 2) + 0.05 * Math.sin(this.time * 4.1 + phase);
      lamp.intensity = 26 * flicker * fire;
      flame.scale.set(2.4 * flicker * blown, 3 * (0.9 + (flicker - 0.86) * 1.3) * blown, 1);
    }
    this.motes.rotation.y += dt * 0.012;
    this.flash.intensity *= Math.pow(0.0006, dt);
    // A piece's ring breathes, and is bright while it is awake.
    for (const held of this.pieces.values()) {
      const data = held.userData, want = 0.3 + 0.07 * Math.sin(this.time * 1.6 + data.square) + Math.min(1.4, data.awake) * 0.55 +
        (data.menace ? 0.45 + 0.25 * Math.sin(this.time * 7) : 0) + (data.stir ? 0.4 : 0);
      data.ring.material.opacity += (want - data.ring.material.opacity) * Math.min(1, dt * 8);
      const lift = data.awake > 0 || data.stir ? 0.05 : 0;
      data.body.position.y += (lift - data.body.position.y) * Math.min(1, dt * 10);
    }
    const pulse = 0.4 + 0.3 * Math.sin(this.time * 6);
    this.mark.check.material.opacity = pulse;
    this.attend(dt);
    this.regard(dt);
    // Rubble: thrown, falling, coming to rest on the board, and then gone.
    const place = new THREE.Object3D();
    for (const heap of [...this.debris]) {
      heap.age += dt;
      heap.bits.forEach((bit, at) => {
        bit.fling.y -= 13 * dt;
        bit.spot.addScaledVector(bit.fling, dt);
        if (bit.spot.y < 0.05) {
          bit.spot.y = 0.05;
          bit.fling.y *= -0.28;
          bit.fling.x *= 0.6; bit.fling.z *= 0.6;
          bit.spin.multiplyScalar(0.5);
        }
        bit.turn.x += bit.spin.x * dt; bit.turn.y += bit.spin.y * dt; bit.turn.z += bit.spin.z * dt;
        place.position.copy(bit.spot);
        place.rotation.copy(bit.turn);
        place.scale.setScalar(bit.size * Math.max(0, Math.min(1, (3.2 - heap.age) / 0.8)));
        place.updateMatrix();
        heap.stones.setMatrixAt(at, place.matrix);
      });
      heap.stones.instanceMatrix.needsUpdate = true;
      if (heap.age > 3.2) { this.scene.remove(heap.stones); heap.stones.dispose(); this.debris.splice(this.debris.indexOf(heap), 1); }
    }
  }
}

// ----------------------------------------------------------------- the sound
// How a room of stone the hall's size answers a sound: two channels of noise
// dying away over 2.6 s, darker as it goes (each sample leans on the one
// before), with the first reflections off the floor and the near columns
// standing out at its start.
function hallEcho(audio) {
  const rate = audio.sampleRate, length = Math.round(rate * 2.6), echo = audio.createBuffer(2, length, rate);
  for (let channel = 0; channel < 2; channel++) {
    const data = echo.getChannelData(channel);
    let dark = 0;
    for (let at = 0; at < length; at++) {
      const t = at / rate, fade = Math.exp(-t * 2.4);
      dark = dark * (0.35 + Math.min(0.55, t * 0.3)) + (Math.random() * 2 - 1) * (1 - 0.35);
      data[at] = dark * fade * 0.6;
    }
    for (const [t, much] of [[0.021, 0.7], [0.043, 0.5], [0.067, 0.42], [0.11, 0.3]]) data[Math.round((t + channel * 0.004) * rate)] += much;
  }
  return echo;
}

// Each piece's own stone (`Sound.play`'s `kind`): `pitch` scales every filter
// and tone it makes, `mass` its loudness and length, and `ring` the chime a
// finer stone keeps after it is set down. A pawn scrapes light and high; a
// rook is a grinding mass; a queen rings; a knight's hooves strike.
const VOICES = {
  p: {pitch: 1.45, mass: 0.6, ring: 0, bell: 0},
  n: {pitch: 1.1, mass: 0.85, ring: 0, bell: 0},
  b: {pitch: 1.25, mass: 0.75, ring: 0.35, bell: 784},
  r: {pitch: 0.55, mass: 1.5, ring: 0, bell: 0},
  q: {pitch: 0.95, mass: 1.15, ring: 1, bell: 659},
  k: {pitch: 0.7, mass: 1.35, ring: 0.6, bell: 523},
};
const PLAIN = {pitch: 1, mass: 1, ring: 0, bell: 0};
// Stone on stone, made here rather than recorded: noise through a filter for
// what grinds and breaks, a falling tone for what lands. A sound the world
// makes is made where it happens (a panner at that point of the hall), and
// heard from where the seat's eye is (`hear`): from the saddle a capture
// beside you is loud and on that side, and one behind you is behind you.
class Sound {
  constructor() { this.on = true; this.audio = null; this.near = 14; this.heard = []; this.panning = "HRTF"; }
  start() {
    if (this.audio || !this.on) return;
    const Audio = window.AudioContext || window.webkitAudioContext;
    if (!Audio) return;
    this.wire(new Audio(), true);
  }
  // The sound's own room, built on an audio context: noise to make stone of,
  // the way out, the hall answering (`send`: a stone room's reverberation,
  // after the time a sound takes to the dome and back), the low breath of
  // the air, and, on a context that plays (`live`), the braziers burning
  // where they stand (`hearth`).
  wire(audio, live = false) {
    this.audio = audio;
    const rate = audio.sampleRate, noise = audio.createBuffer(1, rate * 2, rate), data = noise.getChannelData(0);
    for (let at = 0; at < data.length; at++) data[at] = Math.random() * 2 - 1;
    this.noise = noise;
    this.out = audio.createGain();
    this.out.gain.value = 0.9;
    this.out.connect(audio.destination);
    // The hall answers: what is made in it comes back off the dome and the
    // wall, late and dark, and dies away over two seconds and more.
    const room = audio.createConvolver(), back = audio.createDelay(0.5), wet = audio.createGain();
    room.buffer = hallEcho(audio);
    back.delayTime.value = 0.09;
    wet.gain.value = 0.34;
    this.send = audio.createGain();
    this.send.connect(back).connect(room).connect(wet).connect(this.out);
    // The air: a low breath under everything.
    const air = audio.createBufferSource(), low = audio.createBiquadFilter(), quiet = audio.createGain();
    air.buffer = noise; air.loop = true;
    low.type = "lowpass"; low.frequency.value = 140;
    quiet.gain.value = 0.05;
    air.connect(low).connect(quiet).connect(this.out);
    air.start();
    this.air = quiet;
    if (live) this.burn();
  }
  // Where the fires are (the braziers' flames, from the world).
  hearth(points) { this.fires = points.map(point => point.clone()); }
  // Each brazier burns where it stands: a low roar of its own, and now and
  // then a crack, heard from wherever the seat is.
  burn() {
    const audio = this.audio, fires = (this.fires || []).map(at => {
      const panner = audio.createPanner();
      Object.assign(panner, {panningModel: "HRTF", distanceModel: "inverse", refDistance: 4, rolloffFactor: 1.2, maxDistance: 60});
      if (panner.positionX) { panner.positionX.value = at.x; panner.positionY.value = at.y; panner.positionZ.value = at.z; }
      else panner.setPosition(at.x, at.y, at.z);
      panner.connect(this.out);
      panner.connect(this.send);
      const roar = audio.createBufferSource(), body = audio.createBiquadFilter(), soft = audio.createGain();
      roar.buffer = this.noise; roar.loop = true;
      body.type = "bandpass"; body.frequency.value = 380; body.Q.value = 0.7;
      soft.gain.value = 0.05;
      roar.connect(body).connect(soft).connect(panner);
      roar.start(0, Math.random() * 1.5);
      return panner;
    });
    this.burning = fires;
    if (!fires.length) return;
    setInterval(() => {
      if (!this.on || Math.random() > 0.4) return;
      const into = fires[Math.floor(Math.random() * fires.length)];
      this.burst(0.02 + Math.random() * 0.05, "highpass", 2200 + Math.random() * 1800, 3800, 0.05 + Math.random() * 0.1, 1, into);
    }, 140);
  }
  // A point in the hall to make a sound at. `near` is how close a sound has
  // to be to be heard at its full strength: a rider is among the pieces, and
  // a player at the board is some way off all of them.
  place(at) {
    const audio = this.audio, panner = audio.createPanner();
    Object.assign(panner, {panningModel: this.panning, distanceModel: "inverse", refDistance: this.near, rolloffFactor: 1.3, maxDistance: 80});
    if (panner.positionX) { panner.positionX.value = at.x; panner.positionY.value = at.y + 0.5; panner.positionZ.value = at.z; }
    else panner.setPosition(at.x, at.y + 0.5, at.z);
    panner.connect(this.out);
    // and to the hall, which answers it.
    panner.connect(this.send);
    setTimeout(() => panner.disconnect(), 3000);
    return panner;
  }
  // The ear: where the seat's eye is, and which way it faces.
  hear(camera, near) {
    this.near = near;
    if (!this.audio) return;
    const ear = this.audio.listener, at = camera.position, ahead = new THREE.Vector3(0, 0, -1).applyQuaternion(camera.quaternion),
      up = new THREE.Vector3(0, 1, 0).applyQuaternion(camera.quaternion);
    if (ear.positionX) {
      ear.positionX.value = at.x; ear.positionY.value = at.y; ear.positionZ.value = at.z;
      ear.forwardX.value = ahead.x; ear.forwardY.value = ahead.y; ear.forwardZ.value = ahead.z;
      ear.upX.value = up.x; ear.upY.value = up.y; ear.upZ.value = up.z;
    } else {
      ear.setPosition(at.x, at.y, at.z);
      ear.setOrientation(ahead.x, ahead.y, ahead.z, up.x, up.y, up.z);
    }
  }
  // A finer stone set down keeps ringing a moment: a bell's note and the
  // higher, out-of-tune partial that makes it stone and not a bell.
  chime(note, much, into = this.out) {
    this.tone(1.6, note, note * 0.997, 0.045 * much, "sine", into);
    this.tone(1.1, note * 2.76, note * 2.75, 0.018 * much, "sine", into);
  }
  // A sound is made on the audio context of where it is going (`into`), so
  // that one sent somewhere is never made on another context.
  burst(seconds, kind, from, to, loud, q = 1, into = this.out) {
    const audio = into.context, now = audio.currentTime;
    const source = audio.createBufferSource(), filter = audio.createBiquadFilter(), gain = audio.createGain();
    source.buffer = this.noise;
    source.loop = true;
    filter.type = kind;
    filter.Q.value = q;
    filter.frequency.setValueAtTime(from, now);
    filter.frequency.exponentialRampToValueAtTime(Math.max(30, to), now + seconds);
    gain.gain.setValueAtTime(0.0001, now);
    gain.gain.exponentialRampToValueAtTime(loud, now + Math.min(0.03, seconds / 4));
    gain.gain.exponentialRampToValueAtTime(0.0001, now + seconds);
    source.connect(filter).connect(gain).connect(into);
    source.start(now, Math.random());
    source.stop(now + seconds + 0.05);
  }
  tone(seconds, from, to, loud, wave = "sine", into = this.out) {
    const audio = into.context, now = audio.currentTime, voice = audio.createOscillator(), gain = audio.createGain();
    voice.type = wave;
    voice.frequency.setValueAtTime(from, now);
    voice.frequency.exponentialRampToValueAtTime(Math.max(20, to), now + seconds);
    gain.gain.setValueAtTime(loud, now);
    gain.gain.exponentialRampToValueAtTime(0.0001, now + seconds);
    voice.connect(gain).connect(into);
    voice.start(now);
    voice.stop(now + seconds + 0.05);
  }
  play(name, strength = 1, where = null, kind = null) {
    // What was sounded, and whose, kept for the checks.
    this.heard.push([name, kind]);
    if (this.heard.length > 60) this.heard.shift();
    if (!this.on) return;
    this.start();
    if (!this.audio) return;
    if (this.audio.state === "suspended") this.audio.resume();
    const s = Math.max(0.2, Math.min(1.4, strength)), into = where ? this.place(where) : this.out;
    const voice = VOICES[kind] || PLAIN, f = voice.pitch, m = voice.mass;
    if (name === "slide") {
      this.burst((0.34 + 0.3 * s) * m, "bandpass", 420 * f, 170 * f, 0.22 * s * m, 0.9, into);
      // A rook's weight is heard under its grinding.
      if (kind === "r") this.tone(0.7, 52, 40, 0.16 * s, "triangle", into);
    } else if (name === "lift") this.burst(0.16, "bandpass", 700 * f, 1400 * f, 0.1 * s * m, 1.5, into);
    else if (name === "set") {
      this.tone(0.16 * m, 120 * f, 48 * f, 0.5 * s * Math.min(1.3, m), "sine", into);
      this.burst(0.09, "lowpass", 900 * f, 200 * f, 0.2 * s, 1, into);
      // A knight comes down on its hooves: two sharp strikes.
      if (kind === "n") for (const after of [0, 75]) setTimeout(() => this.on && this.burst(0.05, "bandpass", 2400, 1100, 0.3 * s, 4, into), after);
      if (voice.ring) this.chime(voice.bell, voice.ring * s, into);
    } else if (name === "shatter") {
      this.tone(0.3 * m, 150 * f, 34 * f, 0.9 * s, "sine", into);
      this.burst(0.5, "highpass", 500 * f, 2600 * f, 0.5 * s, 1, into);
      this.burst(0.9 * m, "bandpass", 1500 * f, 300 * f, 0.25 * s, 0.7, into);
      for (let at = 0; at < Math.round(7 * m); at++) setTimeout(() => this.on && this.tone(0.05, (500 + Math.random() * 1400) * f, 200, 0.12 * s, "triangle", into), 60 + Math.random() * 520 * m);
    } else if (name === "snort") {
      // Breath through stone nostrils: a rush, and the low end of it.
      this.burst(0.4, "bandpass", 950, 380, 0.3 * s, 1.1, into);
      this.burst(0.3, "lowpass", 420, 140, 0.22 * s, 1, into);
    } else if (name === "bolt") { this.tone(0.26, 1500, 240, 0.16 * s, "sawtooth", into); this.burst(0.24, "bandpass", 3000, 600, 0.14 * s, 3, into); }
    else if (name === "crown") { for (const [at, pitch] of [[0, 523], [90, 659], [180, 784], [300, 1047]]) setTimeout(() => this.on && this.tone(0.7, pitch, pitch, 0.1, "sine", into), at); }
    else if (name === "pick") this.tone(0.22, 880, 880, 0.05, "sine");
    else if (name === "no") this.tone(0.18, 170, 120, 0.12, "square");
    else if (name === "groan") { this.burst(0.6, "lowpass", 260, 70, 0.32 * s, 2.5, into); this.tone(0.55, 68, 50, 0.2 * s, "triangle", into); }
    else if (name === "peril") { this.tone(0.5, 82, 60, 0.4); setTimeout(() => this.on && this.tone(0.5, 82, 58, 0.32), 210); }
    else if (name === "check") { this.tone(0.7, 196, 185, 0.12, "sawtooth"); this.tone(0.7, 207, 196, 0.1, "sawtooth"); }
  }
}

// ----------------------------------------------------------------- the voice
// What a laptop hears and says: the browser's own speech recognition, where
// it has one, and its speech synthesis.
class Voice {
  constructor(onHeard, onListening) {
    const Recognition = window.SpeechRecognition || window.webkitSpeechRecognition;
    this.can = !!Recognition;
    this.listening = false;
    this.speaks = true;
    this.onHeard = onHeard;
    this.onListening = onListening;
    if (!this.can) return;
    const ear = this.ear = new Recognition();
    ear.lang = "en-US";
    ear.continuous = false;
    ear.interimResults = true;
    ear.maxAlternatives = 6;
    ear.onresult = event => {
      const result = event.results[event.results.length - 1];
      const heard = Array.from(result, option => option.transcript.trim()).filter(Boolean);
      this.onHeard(heard, result.isFinal);
      if (result.isFinal) this.got = true;
    };
    ear.onerror = event => { this.fault = event.error; };
    ear.onend = () => {
      this.listening = false;
      this.onListening(false, this.got ? null : this.fault || "nothing");
    };
  }
  listen() {
    if (!this.can || this.listening) return;
    this.got = false;
    this.fault = null;
    if (window.speechSynthesis) window.speechSynthesis.cancel();
    try { this.ear.start(); this.listening = true; this.onListening(true, null); } catch (_) { /* already started */ }
  }
  done() {
    if (this.can && this.listening) try { this.ear.stop(); } catch (_) { /* not started */ }
  }
  say(text) {
    if (!this.speaks || !window.speechSynthesis || !window.SpeechSynthesisUtterance) return;
    const spoken = new SpeechSynthesisUtterance(text);
    spoken.rate = 0.92;
    spoken.pitch = 0.7;
    spoken.volume = 0.9;
    window.speechSynthesis.cancel();
    window.speechSynthesis.speak(spoken);
  }
}

// ------------------------------------------------------------------ the seat
// Where the player sits. At the board (`table`): at their own side of it,
// far enough back that the whole of it is on the screen, and able to lean.
// On a knight (`ride`): on the back of one of their own knights, behind its
// ears, carried wherever it goes and looking where it looks. Thrown
// (`thrown`), when that knight is taken under them, and then standing at the
// side of the board (`aside`), from where they still command.
// The rider's eye in the mount's own frame (nose toward +x, the board's
// unit a square): low on its neck and to its right, so that the head is seen
// beside them in profile and the pieces stand over them, a pawn near their
// eye and a king half again its height. On the right, the head is on the
// left of the screen, where nothing written stands over it.
const EYE = new THREE.Vector3(-0.46, 1.24, 0.5);
const AHEAD = new THREE.Vector3(1.6, 1.02, 0.5);
// Where a rider stands on any other piece (`World.ride`), in its own frame;
// their eye is a little above it.
const RIDER_TOPS = {p: 0.91, r: 0.92, b: 1.34, q: 1.47, k: 1.72};
const FLOOR = -0.62, STANDING = FLOOR + 0.8;
const FIELD = {table: 36, ride: 64, thrown: 70, aside: 58};
class Seat {
  constructor(canvas, world) {
    this.canvas = canvas;
    this.world = world;
    this.renderer = new THREE.WebGLRenderer({canvas, antialias: true, powerPreference: "high-performance"});
    this.renderer.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.camera = new THREE.PerspectiveCamera(FIELD.table, 1, 0.1, 200);
    this.side = "w";
    this.mode = "table";
    // Where the seat is (`rest`), where the player has leaned to (`lean`),
    // and where the view is now, which follows.
    this.rest = {yaw: 0, pitch: 0.53, zoom: 1};
    this.lean = {yaw: 0, pitch: 0, zoom: 1};
    this.now = {yaw: 0, pitch: 0.53, far: 16, target: new THREE.Vector3(0, 0, 0.3)};
    // A rider's head: where they have turned it (`gaze`), and where it is
    // now, which follows what is happening unless they have just turned it.
    this.gaze = {yaw: 0, pitch: 0, field: 1, at: -10};
    this.head = {yaw: 0, pitch: 0, roll: 0};
    this.mounted = null;
    this.stand = null;
    this.rise = 0;
    this.flight = 0;
    this.blend = null;
    this.focus = null;
    this.watching = null;
    this.alarm = null;
    this.follow = true;
    this.shaking = 0;
    this.punch = 0;
    this.fit = 16;
    this.clock = 0;
    this.ray = new THREE.Raycaster();
    this.resize();
    window.addEventListener("resize", () => this.resize());
  }

  sit(side, at_once = false) {
    this.side = side;
    this.rest.yaw = side === "w" ? 0 : Math.PI;
    this.lean = {yaw: 0, pitch: 0, zoom: 1};
    if (at_once) this.now.yaw = this.rest.yaw;
  }

  // From one seat to another the view travels; it does not cut.
  carry(seconds) {
    this.blend = seconds > 0 && !CALM ? {position: this.camera.position.clone(), quaternion: this.camera.quaternion.clone(), fov: this.camera.fov, at: 0, seconds} : null;
  }
  mount(held, at_once = false) {
    if (this.mode === "ride" && this.mounted === held) return;
    this.carry(at_once ? 0 : 1.1);
    this.flight += 1;
    this.mode = "ride";
    this.mounted = held;
    this.gaze = {yaw: 0, pitch: 0, field: 1, at: -10};
    this.head = null;
  }
  // Back to the board's side, or to the side of the board with no knight.
  unmount(at_once = false, aside = false) {
    if (this.mode === "table" && !aside) return;
    this.carry(at_once ? 0 : 1);
    this.flight += 1;
    this.mounted = null;
    if (aside) {
      this.mode = "aside";
      this.stand = new THREE.Vector3(this.side === "w" ? 5.9 : -5.9, STANDING, this.side === "w" ? 2.6 : -2.6);
      this.head = null;
      this.gaze = {yaw: 0, pitch: 0, field: 1, at: -10};
    } else this.mode = "table";
  }

  // The knight ridden is taken: the rider is thrown clear on the way the blow
  // came, over the edge of the board, lands, and gets up at its side.
  async thrown(from, heading) {
    const flight = ++this.flight;
    this.mode = "thrown";
    this.mounted = null;
    this.blend = null;
    const way = heading.clone().setY(0);
    if (way.lengthSq() < 1e-6) way.set(0, 0, this.side === "w" ? 1 : -1);
    way.normalize();
    // Out along the blow to beyond the plinth.
    const land = from.clone();
    for (let at = 0; at < 160 && Math.max(Math.abs(land.x), Math.abs(land.z)) < 5.9; at++) land.addScaledVector(way, 0.1);
    land.y = FLOOR + 0.14;
    const start = this.camera.position.clone(), head = this.head || {yaw: 0, pitch: 0, roll: 0};
    const pose = this.pose = {position: start.clone(), yaw: head.yaw, pitch: head.pitch, roll: 0};
    const spin = (Math.random() < 0.5 ? -1 : 1) * (CALM ? 0 : 1);
    const gone = () => this.flight !== flight;
    await tween(0.95, t => {
      pose.position.lerpVectors(start, land, t);
      pose.position.y = start.y + (land.y - start.y) * t + Math.sin(t * Math.PI) * 1.4;
      pose.roll = spin * t * 2.2;
      pose.pitch = head.pitch - spin * Math.sin(t * Math.PI) * 1.2 + t * 0.4;
    }, ease.line);
    if (gone()) return;
    this.shake(0.3);
    // On the floor, looking up for a moment.
    const lay = {...pose};
    await tween(0.75, t => { pose.roll = lay.roll + (spin * 0.35 - lay.roll) * t; pose.pitch = lay.pitch + (0.9 - lay.pitch) * t; }, ease.out);
    if (gone()) return;
    // And up, turned to the board.
    const toBoard = new THREE.Vector3(0, 0.4, 0).sub(land), faced = Math.atan2(-toBoard.x, -toBoard.z), up = {...pose};
    await tween(1, t => {
      pose.position.y = land.y + (STANDING - land.y) * t;
      pose.roll = up.roll * (1 - t);
      pose.yaw = up.yaw + turnBy(up.yaw, faced) * t;
      pose.pitch = up.pitch + (-0.12 - up.pitch) * t;
    });
    if (gone()) return;
    this.mode = "aside";
    this.stand = land.clone().setY(STANDING);
    this.head = {yaw: pose.yaw, pitch: pose.pitch, roll: 0};
    this.gaze = {yaw: 0, pitch: 0, field: 1, at: -10};
  }

  place(camera, yaw, pitch, far, target) {
    camera.position.set(target.x + far * Math.sin(yaw) * Math.cos(pitch), target.y + far * Math.sin(pitch), target.z + far * Math.cos(yaw) * Math.cos(pitch));
    camera.lookAt(target);
    camera.updateMatrixWorld();
  }

  // How far back the seat has to be for the whole board to be on the
  // screen, between what is written at its top and the command at its foot.
  resize() {
    const width = window.innerWidth, height = window.innerHeight;
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    const field = this.camera.fov;
    this.camera.fov = FIELD.table;
    this.camera.updateProjectionMatrix();
    const pitch = this.rest.pitch, target = new THREE.Vector3(0, 0, 0.3);
    const corners = [];
    for (const x of [-4.75, 4.75]) for (const z of [-4.75, 4.75]) corners.push(new THREE.Vector3(x, 0, z));
    for (const x of [-3.9, 3.9]) corners.push(new THREE.Vector3(x, 2.05, -3.5));
    const fits = far => {
      this.place(this.camera, 0, pitch, far, target);
      return corners.every(corner => {
        const seen = corner.clone().project(this.camera);
        return Math.abs(seen.x) < 0.97 && seen.y < 0.9 && seen.y > -0.66;
      });
    };
    let near = 7, far = 60;
    for (let at = 0; at < 22; at++) { const mid = (near + far) / 2; if (fits(mid)) far = mid; else near = mid; }
    this.fit = far;
    this.camera.fov = field;
    this.camera.updateProjectionMatrix();
  }

  // A drag, two fingers or an arrow: at the board the view goes round it;
  // on a knight or beside the board it is the rider's head that turns, and
  // the world is taken hold of (drag it left and the head turns right).
  turn(yaw, pitch) {
    if (this.mode !== "table") {
      this.gaze.yaw = Math.max(-2.7, Math.min(2.7, this.gaze.yaw - yaw * 1.3));
      this.gaze.pitch = Math.max(-1.05, Math.min(0.8, this.gaze.pitch + pitch * 1.3));
      this.gaze.at = this.clock;
      return;
    }
    this.lean.yaw = Math.max(-1.35, Math.min(1.35, this.lean.yaw + yaw));
    this.lean.pitch = Math.max(0.2 - this.rest.pitch, Math.min(1.35 - this.rest.pitch, this.lean.pitch + pitch));
  }
  zoom(by) {
    if (this.mode !== "table") { this.gaze.field = Math.max(0.6, Math.min(1.3, this.gaze.field * by)); return; }
    this.lean.zoom = Math.max(0.55, Math.min(1.5, this.lean.zoom * by));
  }
  straighten() { this.lean = {yaw: 0, pitch: 0, zoom: 1}; this.gaze = {yaw: 0, pitch: 0, field: 1, at: -10}; }

  look(point) { this.focus = point && this.follow ? point.clone() : null; }
  // A rider's head snaps round to a new threat, and holds there a moment.
  glance(point, seconds = 1.8) { this.alarm = point ? {point: point.clone().setY(0.9), until: this.clock + seconds} : null; }
  shake(much) { if (!CALM) this.shaking = Math.max(this.shaking, much); }
  // A blow close by: the view jars wider for an instant and comes back.
  kick(much) { if (!CALM) this.punch = Math.max(this.punch, much); }
  // How near a point is to the eye, 1 at arm's length and 0 from four
  // squares off.
  nearness(point) { return Math.max(0, Math.min(1, 1 - (this.camera.position.distanceTo(point) - 1) / 3)); }

  // What is under the pointer, nearest first: the square of each piece the
  // line of sight passes through, and then the square of the board it lands
  // on. From a low seat a piece stands in front of the ones behind it, so a
  // press is taken to mean the first of these that it can mean (`meant`).
  under(event) {
    const box = this.canvas.getBoundingClientRect();
    const at = new THREE.Vector2((event.clientX - box.left) / box.width * 2 - 1, -((event.clientY - box.top) / box.height) * 2 + 1);
    this.ray.setFromCamera(at, this.camera);
    const pieces = [];
    let mounted;
    for (const hit of this.ray.intersectObjects([...this.world.pieces.values()], true)) {
      const owner = hit.object.userData.owner;
      // The knight ridden is under the rider, not in front of them: a press
      // on its ears is a press on what is beyond them (`meant`).
      if (this.mode === "ride" && owner === this.mounted) { mounted = owner.userData.square; continue; }
      if (owner && !pieces.includes(owner.userData.square)) pieces.push(owner.userData.square);
    }
    const spot = new THREE.Vector3();
    let floor = -1;
    if (this.ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), spot)) {
      const file = Math.floor(spot.x + 4), rank = Math.floor(4 - spot.z);
      if (file >= 0 && file <= 7 && rank >= 0 && rank <= 7) floor = rank * 8 + file;
    }
    return mounted === undefined ? {pieces, floor} : {pieces, floor, mounted};
  }

  table(dt) {
    const settle = 1 - Math.exp(-dt * (this.focus ? 3.2 : 4.2));
    let yaw = this.rest.yaw + this.lean.yaw, pitch = this.rest.pitch + this.lean.pitch, far = this.fit * this.lean.zoom;
    const target = new THREE.Vector3(0, 0, this.side === "w" ? 0.3 : -0.3);
    if (this.focus) {
      // Lean in: nearer, lower, and looking at where it is happening.
      target.lerp(this.focus.clone().setY(0.5), 0.55);
      far *= 0.7;
      pitch = Math.max(0.32, pitch - 0.16);
    }
    // The short way round, when the seat changes sides.
    this.now.yaw += turnBy(this.now.yaw, yaw) * settle;
    this.now.pitch += (pitch - this.now.pitch) * settle;
    this.now.far += (far - this.now.far) * settle;
    this.now.target.lerp(target, settle);
    this.place(this.camera, this.now.yaw, this.now.pitch, this.now.far, this.now.target);
  }

  // A head at `eye`, facing `yaw`/`pitch` when nothing is happening, and
  // turned toward what is: a capture most of all, and any piece that moves.
  // A player who has just turned their own head keeps it where they put it.
  aim(eye, yaw, pitch, dt, roll = 0) {
    yaw += this.gaze.yaw;
    pitch += this.gaze.pitch;
    const free = this.clock - this.gaze.at > 2.2;
    const alarmed = this.alarm && this.clock < this.alarm.until ? this.alarm.point : null;
    const seen = this.focus || alarmed || (this.watching && this.watching !== this.mounted ? this.watching.position.clone().setY(0.7) : null);
    if (seen && free) {
      const way = seen.clone().sub(eye), across = Math.hypot(way.x, way.z);
      // What is nearly underneath the eye (the prey of the knight ridden as
      // it comes down on it) gives no way to face: the head is not pulled to
      // it, or it would look straight down and spin.
      if (across > 0.9) {
        const much = this.focus || alarmed ? 0.85 : 0.6;
        yaw += turnBy(yaw, Math.atan2(-way.x, -way.z)) * much;
        pitch += (Math.atan2(way.y, across) - pitch) * much;
      }
    }
    // A head tips so far down and so far up, and no further.
    pitch = Math.max(-1, Math.min(0.85, pitch));
    if (!this.head) this.head = {yaw, pitch, roll};
    const settle = 1 - Math.exp(-dt * (seen && free ? 3.6 : 7));
    this.head.yaw += turnBy(this.head.yaw, yaw) * settle;
    this.head.pitch += (pitch - this.head.pitch) * settle;
    this.head.roll += (roll - this.head.roll) * Math.min(1, dt * 10);
    this.camera.position.copy(eye);
    this.camera.rotation.set(this.head.pitch, this.head.yaw, this.head.roll, "YXZ");
  }

  // On the knight: the eye is a place on the knight itself, so the rider is
  // carried through its leap, rears with it and lurches as it lands.
  rider(dt) {
    const body = this.mounted.userData.body;
    body.updateWorldMatrix(true, false);
    if (this.mounted.userData.kind !== "n") {
      // Standing on top of any other piece: carried as it goes, looking
      // where it looks.
      const top = RIDER_TOPS[this.mounted.userData.kind], at = new THREE.Vector3(0.05, top + 0.43, 0);
      const eye = body.localToWorld(at.clone()), ahead = body.localToWorld(new THREE.Vector3(1.6, top + 0.13, 0)).sub(eye);
      if (!CALM) eye.y += 0.008 * Math.sin(this.clock * 1.3);
      this.aim(eye, Math.atan2(-ahead.x, -ahead.z), Math.atan2(ahead.y, Math.hypot(ahead.x, ahead.z)), dt);
      return;
    }
    const ahead = body.localToWorld(AHEAD.clone()).sub(body.localToWorld(EYE.clone())), forward = Math.atan2(-ahead.x, -ahead.z);
    // The knight's head is to the rider's left. Turned toward it, they rise
    // in the saddle and lean in, so as to see over it, the more the further
    // round they look.
    const across = this.head ? turnBy(forward, this.head.yaw) : 0;
    this.rise += (Math.max(0, Math.min(1, (across - 0.3) / 0.8)) - this.rise) * Math.min(1, dt * 5);
    const seat = EYE.clone();
    seat.y += 0.42 * this.rise;
    seat.z -= 0.2 * this.rise;
    const eye = body.localToWorld(seat);
    // A living thing under you breathes.
    if (!CALM) eye.y += 0.012 * Math.sin(this.clock * 1.7);
    this.aim(eye, Math.atan2(-ahead.x, -ahead.z), Math.atan2(ahead.y, Math.hypot(ahead.x, ahead.z)), dt);
  }

  frame(dt) {
    this.clock += dt;
    if (this.mode === "ride" && !(this.mounted && this.mounted.parent)) this.mode = "table";
    if (this.mode === "table") this.table(dt);
    else if (this.mode === "ride") this.rider(dt);
    else if (this.mode === "thrown") {
      this.camera.position.copy(this.pose.position);
      this.camera.rotation.set(this.pose.pitch, this.pose.yaw, this.pose.roll, "YXZ");
    } else {
      const toBoard = new THREE.Vector3(0, 0.3, 0).sub(this.stand);
      this.aim(this.stand, Math.atan2(-toBoard.x, -toBoard.z), -0.1, dt);
    }
    let field = FIELD[this.mode] * (this.mode === "table" ? 1 : this.gaze.field) * (1 + this.punch);
    this.punch *= Math.pow(0.002, dt);
    if (this.blend) {
      const blend = this.blend;
      blend.at += dt;
      const t = ease.inOut(Math.min(1, blend.at / blend.seconds));
      this.camera.position.lerpVectors(blend.position, this.camera.position, t);
      this.camera.quaternion.slerpQuaternions(blend.quaternion, this.camera.quaternion.clone(), t);
      field = blend.fov + (field - blend.fov) * t;
      if (t >= 1) this.blend = null;
    }
    // A rider does not see themself.
    const saddle = this.mounted && this.mounted.userData.saddle;
    if (saddle) saddle.userData.rider.visible = !(this.mode === "ride" && (!this.blend || this.blend.at > this.blend.seconds * 0.6));
    const near = this.mode === "table" ? 0.1 : 0.03;
    if (Math.abs(this.camera.fov - field) > 0.01 || this.camera.near !== near) {
      this.camera.fov = field;
      this.camera.near = near;
      this.camera.updateProjectionMatrix();
    }
    if (this.shaking > 0.002) {
      this.camera.position.x += (Math.random() - 0.5) * this.shaking;
      this.camera.position.y += (Math.random() - 0.5) * this.shaking;
      this.shaking *= Math.pow(0.004, dt);
    }
    this.camera.updateMatrixWorld();
    this.renderer.render(this.world.scene, this.camera);
  }
}

// ------------------------------------------------------------------ the game
const el = id => document.getElementById(id);
const GLYPH = {P: "♙", N: "♘", B: "♗", R: "♖", Q: "♕", K: "♔", p: "♟", n: "♞", b: "♝", r: "♜", q: "♛", k: "♚"};
const WORTH = {p: 1, n: 3, b: 3, r: 5, q: 9, k: 0};

const kept = (() => { try { return JSON.parse(localStorage.getItem(KEPT) || "{}") || {}; } catch (_) { return {}; } })();
const game = {
  position: fromFen(START), played: [], seen: new Map(), side: kept.side === "b" ? "b" : "w",
  elo: Number.isFinite(kept.elo) ? kept.elo : 1500, busy: false, over: null, chosen: -1, thinking: false, turn: 0,
  // Riding: whether the player rides a knight, and which: the square that
  // knight stood on when the game began (`origins`), which names it through
  // every move it makes and every move taken back.
  ride: kept.ride === true, mount: Number.isInteger(kept.mount) ? kept.mount : null, thrown: false,
  // Who plays the other side: the engine, or a person at the same laptop
  // ("person"), the two taking turns at it. With two, `side` is whoever is to
  // move, and each side keeps the piece it rides (`mounts`, by origin).
  versus: kept.versus === "person" ? "person" : "engine",
  mounts: {w: Number.isInteger(kept.mounts && kept.mounts.w) ? kept.mounts.w : null, b: Number.isInteger(kept.mounts && kept.mounts.b) ? kept.mounts.b : null},
  // How many games have been begun: a move still being played out when the
  // next is begun belongs to the one before (`make`).
  epoch: 0,
};

// A browser that will not draw in three dimensions is told so, and nothing
// else is started for it.
function refuse(why) {
  const box = el("loading");
  box.classList.remove("gone");
  box.innerHTML = `<div><b>Wizard Chess</b>${why}</div>`;
  throw new Error(why);
}
const world = new World();
let seat;
try {
  seat = new Seat(el("stage"), world);
} catch (error) {
  refuse("This browser will not draw the board: Wizard Chess needs WebGL, which is switched off or missing here.");
}
world.surround(seat.renderer);
const sound = new Sound();
sound.hearth(world.flames.map(fire => fire.lamp.position));
const opponent = new Opponent(new URL("./vendor/stockfish-18-lite-single.js", import.meta.url).href);
const voice = new Voice(heard, listening);
const hooks = {sound: (name, strength, at, kind) => sound.play(name, strength, at, kind), look: point => seat.look(point), shake: much => seat.shake(much),
  moving: piece => { seat.watching = piece; },
  // A blow landing at the rider's side: the view jars and shakes, and the
  // world's time slows for a moment (`pace`), so that it is seen.
  impact: (at, force) => {
    if (seat.mode === "table") return;
    const near = seat.nearness(at);
    if (near <= 0) return;
    seat.shake(0.22 * near * force);
    seat.kick(0.09 * near * force);
    if (near > 0.55 && !CALM) { pace.until = performance.now() + 480; pace.scale = 0.3; pace.count += 1; }
  },
  thrown: (at, heading, colour, kind) => { game.thrown = kind || "n"; seat.thrown(at, heading); sound.play("shatter", 1.4); },
  // A pawn ridden to the last rank becomes something else under its rider.
  carried: held => { seat.mount(held, true); }};

sound.on = kept.sound !== false;
voice.speaks = kept.speak !== false;
seat.follow = kept.follow !== false;

function keep() {
  try {
    localStorage.setItem(KEPT, JSON.stringify({side: game.side, elo: game.elo, sound: sound.on, speak: voice.speaks, follow: seat.follow,
      ride: game.ride, mount: game.mount, versus: game.versus, mounts: game.mounts, moves: game.played.map(entry => uci(entry.move))}));
  } catch (_) { /* a browser that keeps nothing */ }
}

function say(text, kind = "") {
  const said = el("said");
  said.textContent = text;
  said.className = kind;
}
const sideName = colour => (colour === "w" ? "White" : "Black");

function hint() {
  const yours = !game.over && game.position.turn === game.side && !game.busy;
  const riding = world.ridden ? ` You ride the ${mountName()} on ${squareName(world.ridden.userData.square)}: “ride to …” moves it.` : " Double-click any of your pieces to ride it.";
  const how = (voice.can ? "Hold <kbd>Space</kbd> and say a move, type one, or click a piece." : "Type a move, or click a piece. (This browser has no speech recognition.)") + riding;
  el("hint").innerHTML = game.over ? "The game is over." : yours ? how : game.thinking ? `${sideName(game.position.turn)} is thinking…` : "&nbsp;";
  el("typed").placeholder = yours ? (game.played.length < 2 ? "knight to f3" : "your move") : "";
  el("take-back").disabled = game.busy || !game.played.some(entry => entry.by === game.side);
}

function draw() {
  // The game as it is written, and what has fallen on each side.
  const rows = [];
  game.played.forEach((entry, at) => {
    if (at % 2 === 0) rows.push(`<i>${at / 2 + 1}.</i>`);
    rows.push(`<b${at === game.played.length - 1 ? ' class="last"' : ""}>${entry.san}</b>`);
  });
  const list = el("moves");
  list.innerHTML = rows.join("");
  list.scrollTop = list.scrollHeight;
  const taken = {w: [], b: []};
  for (const entry of game.played) if (entry.move.captured) taken[colourOf(entry.move.captured)].push(entry.move.captured);
  const worth = colour => taken[colour].reduce((sum, piece) => sum + WORTH[piece.toLowerCase()], 0);
  const ahead = worth("b") - worth("w");
  const line = (colour, lead) => taken[colour].sort((a, b) => WORTH[b.toLowerCase()] - WORTH[a.toLowerCase()]).map(piece => GLYPH[piece]).join("") + (lead > 0 ? ` +${lead}` : "");
  el("fallen-b").textContent = line("b", ahead);
  el("fallen-w").textContent = line("w", -ahead);
  el("score").hidden = !game.played.length;
  // The marks on the board: the last move, a king in check.
  const last = game.played[game.played.length - 1];
  world.show("from", last ? last.move.from : null);
  world.show("to", last ? last.move.to : null);
  const checked = inCheck(game.position) ? kingOf(game.position.board, game.position.turn) : null;
  world.show("check", checked);
  // The king in check flinches away from the pieces that check it.
  const checkers = checked === null ? [] : legalMoves({...game.position, turn: game.position.turn === "w" ? "b" : "w", ep: -1})
    .filter(move => move.to === checked).map(move => move.from);
  world.flinch(checked === null ? -1 : checked, checkers);
  // The hall's light follows the game: darker as the board empties, the
  // shaft onto a king in check.
  world.dusk(game.position.board.filter(Boolean).length);
  world.spotlight(checked === null ? -1 : checked);
  for (const side of ["w", "b"]) el(side === "w" ? "play-white" : "play-black").setAttribute("aria-pressed", String(game.side === side));
  // Where the player sits.
  const knights = ownKnights();
  el("seat-board").setAttribute("aria-pressed", String(!game.ride));
  el("seat-knight").setAttribute("aria-pressed", String(game.ride));
  el("seat-knight").textContent = game.ride && !world.ridden && knights.length ? "Ride again" : world.ridden ? `On a ${mountName()}` : "On a knight";
  el("seat-knight").disabled = !game.ride && !knights.length;
  el("seat-other").hidden = !(game.ride && world.ridden && knights.length > 1);
  el("vs-engine").setAttribute("aria-pressed", String(game.versus === "engine"));
  el("vs-person").setAttribute("aria-pressed", String(game.versus === "person"));
  document.body.classList.toggle("two-players", game.versus === "person");
  // In the saddle what is written folds to what a rider needs.
  document.body.classList.toggle("riding", game.ride);
  hint();
}

// ---- riding
// Where each piece now standing began the game: its square at the start,
// carried through every move. A knight's is its name for the rider.
function origins(played = game.played) {
  const began = Array.from({length: 64}, (_, sq) => sq);
  for (const {move} of played) {
    if (move.flag === "ep") began[move.to + (colourOf(move.piece) === "w" ? -8 : 8)] = null;
    began[move.to] = began[move.from];
    began[move.from] = null;
    if (move.flag === "castle-k") { began[move.to - 1] = began[move.to + 1]; began[move.to + 1] = null; }
    if (move.flag === "castle-q") { began[move.to + 1] = began[move.to - 2]; began[move.to - 2] = null; }
  }
  return began;
}
// The squares of the player's own pieces, of a kind or of every kind.
function ownPieces(kind = null) {
  const board = game.position.board;
  return board.map((piece, sq) => (piece && colourOf(piece) === game.side && (!kind || piece.toLowerCase() === kind) ? sq : -1)).filter(sq => sq >= 0);
}
function ownKnights() {
  const board = game.position.board;
  return board.map((piece, sq) => (piece && piece.toLowerCase() === "n" && colourOf(piece) === game.side ? sq : -1)).filter(sq => sq >= 0);
}
// The square of the piece the player rides, or -1 when it has been taken.
function mountSquare() {
  if (game.mount === null) return -1;
  const began = origins();
  return ownPieces().find(sq => began[sq] === game.mount) ?? -1;
}
const mountName = () => (world.ridden ? PIECE_NAMES[world.ridden.userData.kind] : "knight");
// Ride a piece of the player's own: any of them, by its square.
function rideSquare(sq) {
  const piece = game.position.board[sq];
  if (!piece || colourOf(piece) !== game.side) return false;
  game.ride = true;
  game.mount = origins()[sq];
  remount();
  choose(-1);
  keep();
  draw();
  say(`You ride the ${PIECE_NAMES[piece.toLowerCase()]} on ${squareName(sq)}.`);
  return true;
}
// The knight the player rides, set on the world and the seat as the game
// now stands. With none chosen yet it is the king's knight, or the other.
function remount(at_once = false) {
  if (!game.ride) {
    world.ride(null);
    seat.unmount(at_once);
    world.menace([]);
    return;
  }
  let sq = mountSquare();
  if (sq < 0 && game.mount === null) {
    const knights = ownKnights(), began = origins(), home = game.side === "w" ? 6 : 62;
    sq = knights.find(at => began[at] === home) ?? (knights.length ? knights[0] : -1);
    game.mount = sq >= 0 ? began[sq] : null;
  }
  const held = sq >= 0 ? world.pieces.get(sq) : null;
  if (held) {
    world.ride(held);
    seat.mount(held, at_once);
  } else {
    world.ride(null);
    if (seat.mode !== "thrown" && seat.mode !== "aside") seat.unmount(at_once, true);
  }
  peril(true);
}
function ride(on, other = false) {
  game.ride = on;
  if (on && (other || (game.mount !== null && mountSquare() < 0))) {
    // The next knight along, or another one when this one is gone.
    const knights = ownKnights(), began = origins(), now = mountSquare();
    const next = knights[(knights.indexOf(now) + 1) % knights.length];
    game.mount = next !== undefined ? began[next] : null;
  }
  remount();
  keep();
  draw();
  if (on) say(world.ridden ? `You ride the ${mountName()} on ${squareName(world.ridden.userData.square)}.` : "You have no knight left to ride.");
}
// The pieces that could take the knight ridden, as the board stands: their
// light turns red, and when the threat is new it is said.
function peril(quiet = false) {
  const sq = world.ridden ? world.ridden.userData.square : -1;
  if (sq < 0 || game.over) { world.menace([]); game.threats = ""; return; }
  const foe = game.side === "w" ? "b" : "w";
  const threats = legalMoves({...game.position, turn: foe, ep: -1}).filter(move => move.to === sq);
  world.menace(threats.map(move => move.from));
  const named = threats.map(move => `${PIECE_NAMES[move.piece.toLowerCase()]} on ${squareName(move.from)}`).join(" and the ");
  if (!quiet && named && named !== game.threats) {
    say(`${el("said").textContent} Your ${mountName()} is threatened by the ${named}.`.trim(), "peril");
    sound.play("peril");
    const first = world.pieces.get(threats[0].from);
    if (first) seat.glance(first.position);
  }
  game.threats = named;
}

function place() {
  const height = el("table").getBoundingClientRect().bottom;
  el("score").style.top = `${height + 10}px`;
}

function choose(sq) {
  game.chosen = sq;
  const moves = sq >= 0 ? legalMoves(game.position).filter(move => move.from === sq) : [];
  // A pawn's four promotions are one place to go.
  world.offer(moves.filter((move, at) => moves.findIndex(other => other.to === move.to) === at));
  world.show("chosen", sq >= 0 ? sq : null);
  world.wake(sq);
}

const ENDINGS = {
  checkmate: colour => [`Checkmate`, `${sideName(colour === "w" ? "b" : "w")} wins.`],
  stalemate: () => ["Stalemate", "The side to move has no move and is not in check: a draw."],
  fifty: () => ["A draw", "Fifty moves each with no pawn moved and nothing taken."],
  threefold: () => ["A draw", "The same position has come three times."],
  dead: () => ["A draw", "Neither side has the pieces left to give mate."],
};

async function ended(how) {
  const [title, words] = ENDINGS[how](game.position.turn);
  game.over = how;
  if (how === "checkmate") {
    voice.say("Checkmate.");
    const epoch = game.epoch;
    await world.fall(game.position.turn, hooks);
    if (epoch !== game.epoch) return;
    world.blaze();
  } else voice.say(title);
  el("ended-title").textContent = title;
  el("ended-how").textContent = how !== "checkmate" || game.versus === "person" ? words
    : game.position.turn === game.side ? `${words} The opponent was playing at Elo ${game.elo}.` : `${words} You beat an opponent playing at Elo ${game.elo}.`;
  el("ended").hidden = false;
  draw();
}

// A move, whoever made it: played out on the board, written down, and the
// game looked at again.
async function make(move, by) {
  game.busy = true;
  world.stir();
  const epoch = game.epoch;
  choose(-1);
  const before = game.position, text = san(before, move), aloud = spoken(before, move);
  world.show("from", move.from);
  world.show("to", null);
  hint();
  if (by !== game.side) voice.say(aloud);
  game.thrown = false;
  await world.play(move, hooks);
  // A new game begun while it was played out: this move was the old game's.
  if (epoch !== game.epoch) return;
  game.position = play(before, move);
  game.played.push({move, san: text, by, before});
  const key = positionKey(game.position);
  game.seen.set(key, (game.seen.get(key) || 0) + 1);
  game.busy = false;
  keep();
  const how = standing(game.position, game.seen);
  draw();
  if (game.thrown) {
    const lost = PIECE_NAMES[game.thrown] || "knight";
    say(ownKnights().length ? `Your ${lost} is broken under you, and you are thrown clear. Command from the side of the board, or ride again.`
      : `Your ${lost} is broken under you, and you are thrown clear. Command from the side of the board.`, "peril");
    voice.say("You are thrown.");
  }
  peril(by === game.side);
  if (how === "check") sound.play("check");
  if (how !== "play" && how !== "check") { await ended(how); return; }
  if (game.position.turn !== game.side) (game.versus === "person" ? pass() : answer());
}

// The opponent's move. It is asked the moment the board stands still, and
// is not played sooner than a breath after: a move that comes at once does
// not look thought about.
async function answer() {
  const turn = ++game.turn;
  game.thinking = true;
  hint();
  if (!opponent.awake) say("The opponent is waking…");
  // While it thinks, its pieces shift their weight.
  world.muse(game.position.turn);
  const began = performance.now();
  let found = null;
  try {
    found = await opponent.move(toFen(game.position), game.elo, 900);
  } catch (error) {
    game.thinking = false;
    world.muse(null);
    say(`The opponent could not be woken: ${error.message || error}`, "wrong");
    hint();
    return;
  }
  // Taken back, or a new game begun, while it thought: that game's thinking
  // is over, and this one's is not this answer's to end.
  if (turn !== game.turn || found === null) return;
  world.muse(null);
  const move = moveFromUci(game.position, found);
  if (!move) { game.thinking = false; say(`The opponent answered ${found}, which cannot be played.`, "wrong"); hint(); return; }
  // The breath before it is played: the piece chosen stirs, what it will take
  // braces, and a rider's eye is drawn to it. Never less than a breath, and
  // not played sooner than 650 ms after it was asked for, so that a move that
  // comes at once does not look unconsidered.
  game.foreseen = move.from;
  world.stir([move.from], move.captured && move.flag !== "ep" ? move.to : -1);
  const chosen = world.pieces.get(move.from);
  if (chosen && seat.mode !== "table") seat.glance(chosen.position, 1);
  const pause = Math.max(BREATH, 650 - (performance.now() - began));
  await new Promise(done => setTimeout(done, pause));
  if (turn !== game.turn) { world.stir(); return; }
  game.thinking = false;
  say(`${sideName(game.position.turn)}: ${spoken(game.position, move)}.`);
  await make(move, game.position.turn);
}
const BREATH = 550;

// A move after which the knight ridden can be taken, by its own going or by
// what it uncovers, is asked about first: the rider goes with their knight.
// What has been asked once in a position is not asked again.
function risks(move) {
  const sq = world.ridden ? world.ridden.userData.square : -1;
  if (sq < 0) return false;
  const after = play(game.position, move), at = move.from === sq ? move.to : sq;
  return legalMoves(after).some(answer => answer.to === at);
}
function act(move) {
  const key = `${positionKey(game.position)} ${uci(move)}`;
  if (risks(move) && game.asked !== key) {
    game.asked = key;
    game.risky = move;
    const own = world.ridden && move.from === world.ridden.userData.square;
    el("risk-said").textContent = own
      ? `Your ${mountName()} can be taken on ${squareName(move.to)}, and you with it. Ride on?`
      : `After ${spoken(game.position, move)}, your ${mountName()} can be taken, and you with it. Play it?`;
    el("risk").hidden = false;
    el("risk-go").focus();
    voice.say(own ? "You will be taken there. Ride on?" : `Your ${mountName()} will be taken. Play it?`);
    sound.play("peril");
    return;
  }
  game.risky = null;
  el("risk").hidden = true;
  say(`You: ${spoken(game.position, move)}.`);
  make(move, game.side);
}
function settle(go) {
  const move = game.risky;
  el("risk").hidden = true;
  game.risky = null;
  if (go && move) act(move);
  else say("You hold your ground.");
}

// A command, however it was given.
function command(text, quiet = false) {
  if (game.over || game.busy) return false;
  if (game.position.turn !== game.side) { if (!quiet) say("It is not your move yet.", "wrong"); return false; }
  const read = readCommand(game.position, text, {mount: world.ridden ? world.ridden.userData.square : -1});
  if (!read.move) {
    if (!quiet) { say(read.error, "wrong"); sound.play("no"); }
    return false;
  }
  act(read.move);
  return true;
}

// What the board hears while a command is still being given, typed or said:
// the pieces it could be meant for stir, when that is a few of them and not
// the whole side, and the piece it would take turns to face them.
function stir(text) {
  const yours = !game.over && !game.busy && game.position.turn === game.side;
  const fits = yours && text ? candidates(game.position, text, {mount: world.ridden ? world.ridden.userData.square : -1}) : [];
  const movers = [...new Set(fits.map(move => move.from))];
  const targets = [...new Set(fits.map(move => move.to))];
  const prey = targets.length === 1 && fits.every(move => move.captured && move.flag !== "ep") ? targets[0] : -1;
  world.stir(movers.length <= 6 ? movers : [], prey);
}

function heard(options, final) {
  if (!options.length) return;
  if (!final) { say(options[0], "heard"); stir(options[0]); return; }
  // Asked whether to ride on, a yes or a no is the answer.
  if (game.risky) {
    const said = options.join(" ").toLowerCase();
    if (/\b(yes|yeah|ride|go|charge|onward|play)\b/.test(said)) { settle(true); return; }
    if (/\b(no|stay|hold|wait|stop)\b/.test(said)) { settle(false); return; }
  }
  // What was heard comes as several guesses: the first that is a move is it.
  for (const option of options) if (command(option, true)) return;
  command(options[0]);
}
function listening(on, fault) {
  el("mic").classList.toggle("listening", on);
  if (on) say("Listening…", "heard");
  else if (fault === "not-allowed" || fault === "service-not-allowed") say("The microphone is not allowed here. Type a move, or click a piece.", "wrong");
  else if (fault === "nothing" || fault === "no-speech") say("Nothing was heard. Hold Space while you speak.", "wrong");
  else if (fault) say(`The microphone gave up (${fault}). Type a move, or click a piece.`, "wrong");
}

function begin(side = game.side, moves = []) {
  game.turn += 1;
  game.epoch += 1;
  if (side !== game.side && game.versus !== "person") game.mount = null;
  opponent.stop();
  opponent.newGame();
  Object.assign(game, {position: fromFen(START), played: [], seen: new Map(), side, busy: false, over: null, chosen: -1, thinking: false});
  // A game come back to is set down as it stood.
  for (const text of moves) {
    const move = moveFromUci(game.position, text);
    if (!move) break;
    game.played.push({move, san: san(game.position, move), by: game.position.turn, before: game.position});
    game.position = play(game.position, move);
    const key = positionKey(game.position);
    game.seen.set(key, (game.seen.get(key) || 0) + 1);
  }
  // Two players: whoever is to move sits at the board.
  if (game.versus === "person") {
    if (!moves.length) game.mounts = {w: null, b: null};
    if (game.position.turn !== game.side) { game.mounts[game.side] = game.mount; game.mount = game.mounts[game.position.turn]; }
    game.side = side = game.position.turn;
  }
  el("ended").hidden = true;
  el("promote").hidden = true;
  el("risk").hidden = true;
  game.risky = null;
  world.muse(null);
  world.set(game.position);
  world.letter(side);
  world.offer([]);
  world.show("chosen", null);
  world.show("hover", null);
  seat.sit(side);
  seat.look(null);
  if (!moves.length) game.mount = null;
  remount();
  redress();
  keep();
  draw();
  const how = standing(game.position, game.seen);
  if (how !== "play" && how !== "check") { game.over = how; draw(); if (how === "checkmate") world.blaze(); return; }
  say(game.played.length ? "The game is as you left it." : game.versus === "person" ? "Two players. White to move." : side === "w" ? "White to move. Command your pieces." : "Black. The opponent moves first.");
  if (game.position.turn !== game.side) answer();
}

function takeBack() {
  if (game.busy) return;
  game.turn += 1;
  opponent.stop();
  game.thinking = false;
  // Back to the player's last move: their own, and the answer if it came.
  // With two players, the last move, whoever made it, and the board goes back
  // to them.
  while (game.played.length) {
    const entry = game.played.pop();
    const key = positionKey(game.position);
    game.seen.set(key, (game.seen.get(key) || 1) - 1);
    game.position = entry.before;
    if (entry.by === game.side || game.versus === "person") break;
  }
  if (game.versus === "person" && game.position.turn !== game.side) {
    game.mounts[game.side] = game.mount;
    game.side = game.position.turn;
    game.mount = game.mounts[game.side];
    seat.sit(game.side);
    world.letter(game.side);
  }
  game.over = null;
  el("ended").hidden = true;
  world.muse(null);
  world.set(game.position);
  remount();
  redress();
  choose(-1);
  keep();
  draw();
  say(world.ridden && seat.mode === "ride" ? "Taken back. You are in the saddle." : "Taken back.");
  if (game.position.turn !== game.side) answer();
}

// Two players at one laptop: the board is handed to the side to move. The
// seat goes round to their side (or to the piece they ride), the names on
// the rim turn to be read from there, and the rider who has just moved stays
// on their piece, seen from the other side.
function pass() {
  const was = game.side, now = game.position.turn, left = world.ridden;
  game.mounts[was] = game.mount;
  game.side = now;
  game.mount = game.mounts[now];
  seat.sit(now);
  world.letter(now);
  remount();
  if (left && left.parent && left !== world.ridden) world.dress(left);
  keep();
  draw();
  say(`${sideName(now)} to move.`);
  voice.say(`${sideName(now)} to move.`);
  // What now threatens the piece this player rides is said to them.
  game.threats = "";
  peril();
}
// The other player's rider, set back on their piece after the board has
// been set down afresh.
function redress() {
  if (game.versus !== "person" || !game.ride) return;
  const other = game.side === "w" ? "b" : "w", origin = game.mounts[other];
  if (origin === null || origin === undefined) return;
  const began = origins(), sq = game.position.board.findIndex((piece, at) => piece && colourOf(piece) === other && began[at] === origin);
  const held = sq >= 0 ? world.pieces.get(sq) : null;
  if (held && held !== world.ridden) world.dress(held);
}

// A pawn pointed to the last rank is asked what it becomes.
function askPromotion(moves) {
  const box = el("promote-choices");
  box.innerHTML = "";
  for (const move of moves) {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = GLYPH[move.promo];
    button.title = PIECE_NAMES[move.promo.toLowerCase()];
    button.addEventListener("click", () => { el("promote").hidden = true; act(move); });
    box.appendChild(button);
  }
  el("promote").hidden = false;
  box.firstChild.focus();
}

// What a press on the board means, given what is under it (`Seat.under`:
// the pieces the line of sight passes through, nearest first, and the square
// of the board it lands on). In order: a piece the chosen one may take; the
// chosen piece itself, which is let go; a piece of the player's own that has
// a move; and only then the square of the board behind them all, when the
// chosen piece may go there. A piece that cannot act does not hide the one
// behind it, and a press on a piece is never a press on the floor behind it.
function meant({pieces, floor, mounted}) {
  if (game.over || game.busy || game.position.turn !== game.side) return {};
  const legal = legalMoves(game.position), going = legal.filter(move => move.from === game.chosen);
  const own = sq => { const piece = game.position.board[sq]; return !!piece && colourOf(piece) === game.side; };
  for (const sq of pieces) {
    const moves = going.filter(move => move.to === sq);
    if (moves.length) return {moves, square: sq};
    if (sq === game.chosen) return {letGo: true, square: sq};
    if (own(sq) && legal.some(move => move.from === sq)) return {choose: sq, square: sq};
  }
  const moves = going.filter(move => move.to === floor);
  if (moves.length) return {moves, square: floor};
  // A rider's press on their own knight's head, with nothing beyond it to
  // mean, is the knight: chosen, or let go.
  if (mounted !== undefined) {
    if (mounted === game.chosen) return {letGo: true, square: mounted};
    if (legal.some(move => move.from === mounted)) return {choose: mounted, square: mounted};
  }
  return pieces.length && own(pieces[0]) ? {stuck: pieces[0]} : {};
}

function point(under) {
  const what = meant(under);
  if (what.moves) {
    if (what.moves.length > 1) { askPromotion(what.moves); return; }
    act(what.moves[0]);
  } else if (what.choose !== undefined) {
    choose(what.choose);
    sound.play("pick");
  } else {
    choose(-1);
    // A piece with nowhere to go says so.
    if (what.stuck !== undefined) {
      say(`That ${PIECE_NAMES[game.position.board[what.stuck].toLowerCase()]} has no move.`, "wrong");
      sound.play("no");
    }
  }
}

// ---- what the laptop's own controls do
const stage = el("stage");
let pressed = null;
stage.addEventListener("pointerdown", event => {
  sound.start();
  pressed = {x: event.clientX, y: event.clientY, moved: false, button: event.button};
  stage.setPointerCapture(event.pointerId);
});
stage.addEventListener("pointermove", event => {
  if (pressed && (event.buttons & 3)) {
    const dx = event.clientX - pressed.x, dy = event.clientY - pressed.y;
    if (pressed.moved || Math.hypot(dx, dy) > 6) {
      // A drag turns the view: round the board, and up over it.
      pressed.moved = true;
      seat.turn(-dx * 0.006, dy * 0.005);
      pressed.x = event.clientX; pressed.y = event.clientY;
    }
    return;
  }
  const what = meant(seat.under(event));
  stage.classList.toggle("pointing", what.square !== undefined);
  world.show("hover", what.square !== undefined ? what.square : null);
});
stage.addEventListener("pointerup", event => {
  const was = pressed;
  pressed = null;
  if (!was || was.moved || was.button !== 0) return;
  point(seat.under(event));
});
// A double click on one of the player's own pieces rides it; anywhere else
// it puts the view back.
stage.addEventListener("dblclick", event => {
  const under = seat.under(event), own = sq => { const piece = game.position.board[sq]; return !!piece && colourOf(piece) === game.side; };
  const sq = under.pieces.find(own);
  if (sq !== undefined && !game.busy && !game.over) rideSquare(sq);
  else seat.straighten();
});
stage.addEventListener("contextmenu", event => event.preventDefault());
// Two fingers on the trackpad turn the view, and a pinch (which a browser
// sends as a wheel with Ctrl held) draws it near.
stage.addEventListener("wheel", event => {
  event.preventDefault();
  if (event.ctrlKey || event.metaKey) seat.zoom(Math.exp(event.deltaY * 0.01));
  else seat.turn(event.deltaX * 0.0035, event.deltaY * 0.003);
}, {passive: false});

const typed = el("typed");
typed.addEventListener("keydown", event => {
  event.stopPropagation();
  if (event.key === "Enter") { if (command(typed.value)) typed.value = ""; else typed.select(); }
  if (event.key === "Escape") { typed.value = ""; typed.blur(); stir(""); }
});
typed.addEventListener("input", () => stir(typed.value));
window.addEventListener("keydown", event => {
  if (event.target instanceof HTMLElement && /^(INPUT|SELECT|TEXTAREA|BUTTON|SUMMARY|A)$/.test(event.target.tagName) && event.target !== stage) {
    if (event.code !== "Space" || event.target.tagName === "INPUT") return;
  }
  sound.start();
  if (event.code === "Space") { event.preventDefault(); if (!event.repeat) voice.listen(); return; }
  if (event.key === "Escape") { choose(-1); el("promote").hidden = true; if (game.risky) settle(false); return; }
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "z") { event.preventDefault(); takeBack(); return; }
  if (event.metaKey || event.ctrlKey || event.altKey) return;
  if (event.key === "ArrowLeft") seat.turn(-0.12, 0);
  else if (event.key === "ArrowRight") seat.turn(0.12, 0);
  else if (event.key === "ArrowUp") seat.turn(0, 0.08);
  else if (event.key === "ArrowDown") seat.turn(0, -0.08);
  else if (event.key === "0") seat.straighten();
  // Anything else that is a letter or a number is the start of a typed move.
  else if (event.key.length === 1 && /[a-z0-9]/i.test(event.key)) typed.focus();
});
window.addEventListener("keyup", event => { if (event.code === "Space") voice.done(); });
const mic = el("mic");
mic.hidden = !voice.can;
mic.addEventListener("pointerdown", event => { event.preventDefault(); sound.start(); voice.listen(); });
for (const kind of ["pointerup", "pointerleave", "pointercancel"]) mic.addEventListener(kind, () => voice.done());

el("new-game").addEventListener("click", () => begin(game.versus === "person" ? "w" : game.side));
// Who plays the other side. Changing it begins a new game.
function versus(who) {
  if (game.versus === who) return;
  game.versus = who;
  game.mounts = {w: null, b: null};
  begin("w");
}
el("vs-engine").addEventListener("click", () => versus("engine"));
el("vs-person").addEventListener("click", () => versus("person"));
el("seat-board").addEventListener("click", () => { if (game.ride) ride(false); });
el("seat-knight").addEventListener("click", () => { if (!game.ride || !world.ridden) ride(true); });
el("seat-other").addEventListener("click", () => ride(true, true));
el("take-back").addEventListener("click", takeBack);
el("play-white").addEventListener("click", () => { if (game.side !== "w" || game.played.length) begin("w"); });
el("play-black").addEventListener("click", () => { if (game.side !== "b" || game.played.length) begin("b"); });
el("ended-again").addEventListener("click", () => begin(game.side));
el("risk-go").addEventListener("click", () => settle(true));
el("risk-stay").addEventListener("click", () => settle(false));
el("ended-look").addEventListener("click", () => { el("ended").hidden = true; });
el("opt-sound").checked = sound.on;
el("opt-speak").checked = voice.speaks;
el("opt-follow").checked = seat.follow;
el("opt-sound").addEventListener("change", event => { sound.on = event.target.checked; if (sound.out) sound.out.gain.value = sound.on ? 0.9 : 0; keep(); });
el("opt-speak").addEventListener("change", event => { voice.speaks = event.target.checked; if (!voice.speaks && window.speechSynthesis) window.speechSynthesis.cancel(); keep(); });
el("opt-follow").addEventListener("change", event => { seat.follow = event.target.checked; keep(); });

// The opponent's strength: the engine's own range, said by the engine.
const slider = el("elo");
function strength() {
  const {min, max} = opponent.range, full = game.elo >= max;
  el("elo-said").innerHTML = `${game.elo}<small>${full ? "Elo · full strength" : "Elo"}</small>`;
  el("elo-about").textContent = full
    ? `${opponent.name}, with nothing held back.`
    : `${opponent.name}, held to this strength. The scale is its own: a computer rating list's Elo, approximately.`;
  place();
}
slider.addEventListener("input", () => { game.elo = +slider.value; strength(); });
slider.addEventListener("change", keep);
opponent.ready.then(() => {
  const {min, max} = opponent.range;
  slider.min = min; slider.max = max;
  el("elo-min").textContent = min; el("elo-max").textContent = max;
  game.elo = Math.max(min, Math.min(max, game.elo));
  slider.value = game.elo;
  strength();
  if (el("said").textContent === "The opponent is waking…") say("");
}).catch(error => {
  el("elo-about").textContent = `The opponent could not be started (${error.message || error}), so there is nobody to play against here.`;
});

window.addEventListener("resize", place);
document.addEventListener("click", event => { const sites = el("sites"); if (sites.open && !sites.contains(event.target)) sites.open = false; });

// ---- begin
slider.value = game.elo;
strength();
begin(game.side, Array.isArray(kept.moves) ? kept.moves : []);
seat.sit(game.side, true);
place();
// The world's time against the clock's: slowed for a moment when a blow lands
// beside a rider (`hooks.impact`). The seat keeps the clock's time, so a rider
// can still turn their head while the stone hangs in the air.
const pace = {scale: 1, until: 0, count: 0};
let then = performance.now();
function frame(now) {
  const dt = Math.min(0.05, (now - then) / 1000);
  then = now;
  const slow = performance.now() < pace.until ? pace.scale : 1;
  advance(dt * slow);
  world.tick(dt * slow);
  seat.frame(dt);
  sound.hear(seat.camera, seat.mode === "table" ? 14 : 2.2);
  requestAnimationFrame(frame);
}
seat.blend = null;
requestAnimationFrame(now => { then = now; el("loading").classList.add("gone"); frame(now); });

// What a frame costs on this machine, in milliseconds: the world's own work
// and the drawing, over `count` frames, each one made to finish on the
// graphics card before the next is begun (a pixel is read back). With the
// name of what drew it, which says whether that was the machine's card or a
// browser drawing in software.
function frameCost(count = 120) {
  const gl = seat.renderer.getContext(), pixel = new Uint8Array(4), told = gl.getExtension("WEBGL_debug_renderer_info");
  const finish = () => gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, pixel);
  for (let at = 0; at < Math.min(10, count); at++) { world.tick(1 / 60); seat.frame(1 / 60); finish(); }
  const started = performance.now();
  for (let at = 0; at < count; at++) { world.tick(1 / 60); seat.frame(1 / 60); finish(); }
  return {ms: (performance.now() - started) / count, drawnBy: told ? gl.getParameter(told.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER),
    pixels: [seat.renderer.domElement.width, seat.renderer.domElement.height], seat: seat.mode};
}

// For the checks (beta/wizard_chess_test.py), and for anyone at the console.
window.wizardChess = {game, world, seat, sound, opponent, command, begin, takeBack, running, meant, point, choose, make, ride, origins,
  mountSquare, moveFromUci, peril, settle, frameCost, stir, pace, rideSquare, versus};
