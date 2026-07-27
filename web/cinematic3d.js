/* CIVVIS cinematic 3D models.
 *
 * This is deliberately dependency-free: the strategic map remains one
 * Canvas2D scene, while close cinematic shots can ask this module to project
 * small real 3D meshes into that canvas. Units use a compact perspective
 * camera; terrain features use the map's orthographic yaw and tilt. Geometry
 * is depth-sorted per face and lit from a fixed world key, so models remain
 * solid while the board rotates and zooms. The ordinary vector figures and
 * painted sprites remain compatibility fallbacks.
 */
(function installCinematic3D(global) {
  "use strict";

  const FAMILIES = Object.freeze([
    "embarked", "naval", "air", "rotor", "balloon", "drone", "robot",
    "armor", "gun", "siege", "mounted", "religious", "civilian", "infantry",
  ]);
  const FAMILY_SET = new Set(FAMILIES);
  const MELEE = new Set(["warrior", "eagle_warrior", "swordsman", "legion", "man_at_arms"]);
  const FIREARM = new Set(["musketman", "line_infantry", "infantry", "pike_and_shot",
    "ranger", "spec_ops", "machine_gun"]);
  const SPEAR = new Set(["spearman", "pikeman", "hoplite", "pike_and_shot", "at_crew", "modern_at"]);
  const BOW = new Set(["slinger", "archer", "crossbowman", "pitati_archer",
    "crouching_tiger", "skirmisher", "saka_horse_archer", "maryannu_chariot_archer"]);
  const CHARIOTS = new Set(["heavy_chariot", "war_cart", "maryannu_chariot_archer"]);
  const MODERN_SHIPS = new Set(["ironclad", "battleship", "destroyer", "aircraft_carrier",
    "missile_cruiser"]);
  const SUBMARINES = new Set(["submarine", "nuclear_submarine"]);
  // Ordinary terrain features, then the full Natural Wonder roster. Every one
  // of the thirty-four is modelled: a wonder the cinematic view cannot draw is
  // a landmark the player walks past as bare ground.
  const ENVIRONMENTS = Object.freeze([
    "hills", "mountain", "forest", "jungle", "burning_forest", "burnt_forest",
    "burning_jungle", "burnt_jungle", "marsh", "oasis", "floodplains",
    "grassland_floodplains", "plains_floodplains", "reef", "geothermal_fissure",
    "ice", "volcano", "volcanic_soil", "impact_zone", "great_barrier_reef",
    "crater_lake", "pantanal", "uluru", "yosemite", "dead_sea",
    "mount_everest", "pamukkale", "torres_del_paine", "eye_of_the_sahara",
    "zhangye_danxia", "ha_long_bay", "cliffs_of_dover", "giants_causeway",
    "galapagos_islands", "matterhorn", "kilimanjaro", "piopiotahi", "ik_kil",
    "gobustan", "ubsunur_hollow", "mato_tipila", "delicate_arch",
    "chocolate_hills", "vesuvius", "lake_retba", "bermuda_triangle",
    "eyjafjallajokull", "fountain_of_youth", "lysefjord", "paititi",
    "mount_roraima", "tsingy_de_bemaraha", "sahara_el_beyda",
  ]);
  const ENVIRONMENT_SET = new Set(ENVIRONMENTS);

  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
  const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const mul = (a, k) => [a[0] * k, a[1] * k, a[2] * k];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const norm = a => {
    const n = Math.hypot(a[0], a[1], a[2]) || 1;
    return [a[0] / n, a[1] / n, a[2] / n];
  };
  const hex = color => {
    const text = String(color || "#82909a");
    const rgb = text.match(/^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/i);
    if (rgb) return rgb.slice(1, 4).map(Number);
    const raw = text.replace("#", "");
    const h = raw.length === 3 ? raw.split("").map(c => c + c).join("") : raw.slice(0, 6);
    const value = Number.parseInt(h, 16);
    return Number.isFinite(value)
      ? [(value >> 16) & 255, (value >> 8) & 255, value & 255] : [130, 144, 154];
  };
  const tint = (color, amount, alpha = 1) => {
    const [r, g, b] = hex(color);
    const lift = amount > 1 ? 255 * (amount - 1) * .16 : 0;
    const scale = amount > 1 ? 1 : amount;
    return `rgba(${clamp(Math.round(r * scale + lift), 0, 255)},` +
      `${clamp(Math.round(g * scale + lift), 0, 255)},` +
      `${clamp(Math.round(b * scale + lift), 0, 255)},${alpha})`;
  };

  function rotatePoint(p, rx = 0, ry = 0, rz = 0) {
    let [x, y, z] = p;
    let c = Math.cos(rx), s = Math.sin(rx);
    [y, z] = [y * c - z * s, y * s + z * c];
    c = Math.cos(ry); s = Math.sin(ry);
    [x, z] = [x * c + z * s, -x * s + z * c];
    c = Math.cos(rz); s = Math.sin(rz);
    [x, y] = [x * c - y * s, x * s + y * c];
    return [x, y, z];
  }

  class Scene {
    constructor(ctx, options) {
      this.ctx = ctx;
      this.scale = options.scale || 1;
      this.facing = options.facing < 0 ? -1 : 1;
      this.bank = options.bank || 0;
      this.yaw = options.yaw || 0;
      this.tilt = options.tilt || .38;
      this.orthographic = !!options.orthographic;
      this.items = [];
      this.light = Number.isFinite(options.sunAngle)
        ? norm([Math.cos(options.sunAngle + this.yaw),
          Math.sin(options.sunAngle + this.yaw), 1.12])
        : norm([-0.55 * this.facing, -0.72, 1.1]);
      this.stroke = options.stroke === undefined ? "rgba(8,12,16,.78)" : options.stroke;
    }

    world(p) {
      let q = [p[0] * this.facing, p[1], p[2]];
      if (this.bank) q = rotatePoint(q, 0, this.bank * .22, this.bank * .12);
      if (this.yaw) q = rotatePoint(q, 0, 0, this.yaw);
      return q;
    }

    project(p) {
      const q = this.world(p);
      const perspective = this.orthographic ? 1 : 88 / (88 + q[1]);
      return {
        x: q[0] * this.scale * perspective,
        y: (q[1] * (this.orthographic ? this.tilt : .38) - q[2]) *
          this.scale * perspective,
        depth: q[1] + q[2] * .012,
        perspective,
      };
    }

    // `depthBias` exists for a face that is the *ground* of its own model —
    // one broad plane the rest of the model stands on. Depth is dominated by
    // world y, so without it anything placed toward the back of that plane
    // sorts in front of it and the plane paints over its own furniture.
    mesh(points, faces, color, alpha = 1, depthBias = 0) {
      const world = points.map(p => this.world(p));
      const projected = points.map(p => this.project(p));
      for (const face of faces) {
        const vertices = face.map(i => projected[i]);
        if (vertices.length < 3) continue;
        const a = world[face[0]], b = world[face[1]], c = world[face[2]];
        const normal = norm(cross(sub(b, a), sub(c, a)));
        const direct = Math.max(0, dot(normal, this.light));
        const rim = Math.max(0, normal[2]) * .08;
        this.items.push({
          kind: "face", vertices,
          depth: depthBias +
            vertices.reduce((sum, p) => sum + p.depth, 0) / vertices.length,
          fill: tint(color, .42 + direct * .72 + rim, alpha),
          shine: direct > .72 ? direct : 0,
        });
      }
    }

    box(center, size, color, rotation = [0, 0, 0]) {
      const [w, d, h] = size, [cx, cy, cz] = center;
      const points = [];
      for (const z of [-h / 2, h / 2]) for (const y of [-d / 2, d / 2])
        for (const x of [-w / 2, w / 2]) {
          const p = rotatePoint([x, y, z], rotation[0], rotation[1], rotation[2]);
          points.push([p[0] + cx, p[1] + cy, p[2] + cz]);
        }
      this.mesh(points, [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1],
        [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]], color);
    }

    wedge(center, size, color, nose = 1) {
      const [w, d, h] = size, [x, y, z] = center;
      const n = nose >= 0 ? d / 2 : -d / 2, t = -n;
      const points = [[-w / 2, t, -h / 2], [w / 2, t, -h / 2],
        [-w / 2, t, h / 2], [w / 2, t, h / 2], [0, n, -h / 3], [0, n, h / 3]]
        .map(p => add(p, [x, y, z]));
      this.mesh(points, [[0, 1, 3, 2], [0, 4, 1], [2, 3, 5],
        [0, 2, 5, 4], [1, 4, 5, 3]], color);
    }

    tube(a, b, radius, color, sides = 7) {
      const axis = norm(sub(b, a));
      const reference = Math.abs(axis[2]) > .88 ? [0, 1, 0] : [0, 0, 1];
      const u = norm(cross(axis, reference)), v = norm(cross(axis, u));
      const points = [];
      for (const end of [a, b]) for (let i = 0; i < sides; i++) {
        const angle = i * Math.PI * 2 / sides;
        points.push(add(end, add(mul(u, Math.cos(angle) * radius),
          mul(v, Math.sin(angle) * radius))));
      }
      const faces = [];
      for (let i = 0; i < sides; i++) faces.push([i, (i + 1) % sides,
        sides + (i + 1) % sides, sides + i]);
      faces.push([...Array(sides).keys()].reverse());
      faces.push([...Array(sides).keys()].map(i => sides + i));
      this.mesh(points, faces, color);
    }

    ellipsoid(center, radii, color, segments = 8, rings = 4, alpha = 1) {
      const points = [];
      for (let r = 0; r <= rings; r++) {
        const lat = -Math.PI / 2 + r * Math.PI / rings;
        for (let i = 0; i < segments; i++) {
          const lon = i * Math.PI * 2 / segments;
          points.push([center[0] + Math.cos(lat) * Math.cos(lon) * radii[0],
            center[1] + Math.cos(lat) * Math.sin(lon) * radii[1],
            center[2] + Math.sin(lat) * radii[2]]);
        }
      }
      const faces = [];
      for (let r = 0; r < rings; r++) for (let i = 0; i < segments; i++) {
        const next = (i + 1) % segments;
        faces.push([r * segments + i, r * segments + next,
          (r + 1) * segments + next, (r + 1) * segments + i]);
      }
      this.mesh(points, faces, color, alpha);
    }

    polygon(points, color, thickness = .45, alpha = 1) {
      const top = points.map(p => [p[0], p[1], p[2] + thickness / 2]);
      const bottom = points.map(p => [p[0], p[1], p[2] - thickness / 2]);
      const vertices = [...bottom, ...top], n = points.length;
      const faces = [[...Array(n).keys()].reverse(), [...Array(n).keys()].map(i => n + i)];
      for (let i = 0; i < n; i++) faces.push([i, (i + 1) % n, n + (i + 1) % n, n + i]);
      this.mesh(vertices, faces, color, alpha);
    }

    cone(center, radius, height, color, sides = 8, topRadius = 0,
         rotation = 0, alpha = 1) {
      const [cx, cy, cz] = center;
      const points = [];
      for (const [z, r] of [[cz, radius], [cz + height, topRadius]]) {
        for (let i = 0; i < sides; i++) {
          const angle = rotation + i * Math.PI * 2 / sides;
          points.push([cx + Math.cos(angle) * r, cy + Math.sin(angle) * r, z]);
        }
      }
      const faces = [];
      for (let i = 0; i < sides; i++) faces.push([
        i, (i + 1) % sides, sides + (i + 1) % sides, sides + i,
      ]);
      faces.push([...Array(sides).keys()].reverse());
      if (topRadius > 0) faces.push([...Array(sides).keys()].map(i => sides + i));
      this.mesh(points, faces, color, alpha);
    }

    glow(point, radius, color, alpha = .7) {
      const p = this.project(point);
      this.items.push({kind: "glow", ...p, radius: radius * this.scale * p.perspective,
        color, alpha});
    }

    shadow(rx, ry, alpha = .28) {
      const p = this.project([0, 0, .1]);
      this.items.push({kind: "shadow", ...p, depth:-1e6, rx: rx * this.scale,
        ry: ry * this.scale, alpha});
    }

    flush() {
      const ctx = this.ctx;
      this.items.sort((a, b) => a.depth - b.depth);
      ctx.save();
      ctx.lineJoin = "round"; ctx.lineCap = "round";
      for (const item of this.items) {
        if (item.kind === "face") {
          ctx.beginPath();
          ctx.moveTo(item.vertices[0].x, item.vertices[0].y);
          for (let i = 1; i < item.vertices.length; i++)
            ctx.lineTo(item.vertices[i].x, item.vertices[i].y);
          ctx.closePath(); ctx.fillStyle = item.fill; ctx.fill();
          if (this.stroke) {
            ctx.strokeStyle = this.stroke; ctx.lineWidth = .72; ctx.stroke();
          }
          if (item.shine && this.stroke) {
            ctx.globalAlpha = (item.shine - .7) * .34;
            ctx.strokeStyle = "#fff"; ctx.lineWidth = .45; ctx.stroke();
            ctx.globalAlpha = 1;
          }
        } else if (item.kind === "shadow") {
          ctx.fillStyle = `rgba(0,0,0,${item.alpha})`;
          ctx.beginPath(); ctx.ellipse(item.x, item.y + 1.4, item.rx, item.ry, 0, 0, 7); ctx.fill();
        } else {
          const gradient = ctx.createRadialGradient(item.x, item.y, 0,
            item.x, item.y, Math.max(1, item.radius));
          gradient.addColorStop(0, tint(item.color, 1.55, item.alpha));
          gradient.addColorStop(.35, tint(item.color, 1.1, item.alpha * .68));
          gradient.addColorStop(1, tint(item.color, 1, 0));
          ctx.fillStyle = gradient; ctx.beginPath();
          ctx.arc(item.x, item.y, item.radius, 0, 7); ctx.fill();
        }
      }
      ctx.restore();
    }
  }

  const mapPoint = (p, origin, scale) => [origin[0] + p[0] * scale,
    origin[1] + p[1] * scale, origin[2] + p[2] * scale];

  function human(scene, options, origin = [0, 0, 0], size = 1) {
    const type = options.type;
    const walk = options.moving ? Math.sin(options.time * 8.5 + options.seed) : 0;
    const action = options.action;
    const p = point => mapPoint(point, origin, size);
    const armor = tint(options.color, .78);
    const cloth = options.family === "religious" ? "#eee4cc" : options.color;
    const skin = options.skin || "#e8c49b";
    const leftFoot = p([-2.1 - walk * 1.7, -.3, .8]);
    const rightFoot = p([2.1 + walk * 1.7, .3, .8]);
    const leftKnee = p([-1.6, walk * .7, 4.5]);
    const rightKnee = p([1.6, -walk * .7, 4.5]);
    scene.tube(p([-1.7, 0, 8]), leftKnee, 1.05 * size, "#273039", 6);
    scene.tube(leftKnee, leftFoot, .9 * size, "#20272e", 6);
    scene.tube(p([1.7, 0, 8]), rightKnee, 1.05 * size, "#273039", 6);
    scene.tube(rightKnee, rightFoot, .9 * size, "#20272e", 6);
    scene.box(p([0, 0, 11.5]), [7.4 * size, 3.8 * size, 8 * size], cloth, [0, 0, -.04]);
    if (!options.civilian && options.family !== "religious")
      scene.box(p([0, -.15, 14.6]), [7.7 * size, 4 * size, 2.1 * size], armor);
    scene.ellipsoid(p([0, 0, 18]), [3.05 * size, 2.7 * size, 3.25 * size], skin, 7, 3);
    if (!options.civilian && options.family !== "religious")
      scene.ellipsoid(p([0, 0, 19.1]), [3.25 * size, 2.9 * size, 1.55 * size], armor, 7, 2);

    const shoulderL = p([-3.4, 0, 14]), shoulderR = p([3.4, 0, 14]);
    const leftHand = p([-5.2, -.4, 9.6]);
    let rightHand = p([5.1 + action * 2, -1.2, 10.4 + action * 3.5]);
    if (FIREARM.has(type) || BOW.has(type)) rightHand = p([5.4, -2.6 - action, 13.1]);
    scene.tube(shoulderL, leftHand, .9 * size, skin, 6);
    scene.tube(shoulderR, rightHand, .9 * size, skin, 6);

    if (MELEE.has(type) || type === "warrior_monk") {
      scene.tube(p([4.6, -1.2, 10.7]), p([10 + action * 3, -3,
        20 - action * 7]), .42 * size, "#edf3f5", 5);
      scene.box(p([-5.1, -.2, 11.2]), [4.8 * size, 1 * size, 6.2 * size], armor,
        [0, 0, -.1]);
    } else if (SPEAR.has(type)) {
      scene.tube(p([4.2, -.7, 5]), p([7 + action * 5, -3,
        23 - action * 5]), .34 * size, "#7c5d3b", 6);
      scene.wedge(p([7 + action * 5, -3, 23.8 - action * 5]),
        [1.5 * size, 3 * size, 3.5 * size], "#d8e1e6");
    } else if (FIREARM.has(type)) {
      const recoil = action * 2.2;
      scene.tube(p([-1.5, -1.3, 12]), p([11 - recoil, -4, 15]),
        .65 * size, "#4d3b2c", 7);
      scene.tube(p([3, -2.5, 14]), p([14 - recoil, -4.5, 15.3]),
        .42 * size, "#303940", 7);
      if (action > .58) scene.glow(p([15 - recoil, -4.7, 15.4]), 4.2 * size,
        "#ffcc6b", (action - .58) * 2);
    } else if (BOW.has(type)) {
      if (type === "slinger") {
        const cast = action * 5.5;
        scene.tube(p([4, -1, 13]), p([8 + cast, -3, 18 - action * 3]),
          .14 * size, "#94704b", 5);
        scene.tube(p([8 + cast, -3, 18 - action * 3]),
          p([11 + cast, -3.5, 16 - action * 3]), .14 * size, "#94704b", 5);
        scene.ellipsoid(p([11.5 + cast, -3.5, 16 - action * 3]),
          [1.3 * size, 1.1 * size, 1.2 * size], "#697076", 6, 2);
      } else {
        const pull = action * 2.6;
        const top = p([8, -2, 19]), mid = p([5 - pull, -3.5, 13.5]), bot = p([8, -2, 8]);
        scene.tube(top, p([10, -2, 16]), .28 * size, "#8c653d", 5);
        scene.tube(p([10, -2, 16]), bot, .28 * size, "#8c653d", 5);
        scene.tube(top, mid, .12 * size, "#eadfbe", 4);
        scene.tube(mid, bot, .12 * size, "#eadfbe", 4);
        scene.tube(mid, p([15 - pull, -4, 13.5]), .17 * size, "#d9e1df", 5);
      }
    } else if (options.family === "religious") {
      scene.tube(p([5, 0, 5]), p([7 + action * 2, -1, 22 + action * 2]),
        .45 * size, type === "inquisitor" ? "#d95b3e" : "#c7a84b", 7);
      scene.glow(p([7 + action * 2, -1, 23 + action * 2]),
        (2.2 + action * 3.5) * size, type === "inquisitor" ? "#ff7648" : "#ffe58c",
        .35 + action * .45);
    } else if (type === "builder" || type === "military_engineer" || type === "archaeologist") {
      scene.tube(p([4, 0, 7]), p([9 + action * 2, -1, 19]), .38 * size,
        "#795634", 6);
      scene.box(p([9.5 + action * 2, -1, 19.5]), [5 * size, 2 * size, 2.8 * size],
        "#929da3", [0, 0, .25]);
    } else if (type === "rock_band") {
      scene.ellipsoid(p([6, -1, 11.5]), [3.8 * size, 1.4 * size, 5.2 * size],
        "#c85c92", 7, 3);
      scene.tube(p([4, -1, 13]), p([10, -2, 20]), .35 * size, "#d8c59e", 6);
    } else if (type === "medic") {
      scene.box(p([-4.2, .2, 11]), [3.6 * size, 2 * size, 6 * size], "#ece9dc");
      scene.box(p([-4.2, -1, 11]), [2.6 * size, .4 * size, .7 * size], "#d94b48");
      scene.box(p([-4.2, -1, 11]), [.7 * size, .4 * size, 2.6 * size], "#d94b48");
    } else if (type === "spy") {
      scene.polygon([p([-5, 0, 7]), p([0, 1, 17]), p([5, 0, 7])], "#202631", .7);
      scene.glow(p([1.2, -2.5, 18]), .8 * size, "#d9f4ff", .8);
    } else if (type === "naturalist") {
      scene.tube(p([4, 0, 7]), p([8, -1, 20]), .36 * size, "#355b38", 6);
      scene.ellipsoid(p([8, -1, 21]), [2 * size, 1.4 * size, 1 * size], "#c8dcc1", 6, 2);
    } else if (type === "scout") {
      scene.tube(p([5, .5, 5]), p([7, -.5, 22]), .34 * size, "#6e4d31", 6);
      scene.polygon([p([-4.4, 1.8, 15]), p([4.4, 1.8, 15]), p([2.8, 2, 7]),
        p([-3.5, 2, 8])], tint(options.color, .58), .45 * size);
      scene.box(p([-4.7, 1.6, 11.5]), [3.4 * size, 2.2 * size, 5.6 * size],
        "#71543b");
    } else if (type === "trader" || type === "settler") {
      scene.box(p([-6, .8, 7]), [6 * size, 4 * size, 5 * size], "#a77e4d");
      scene.tube(p([-8, 1, 4]), p([-8, 1, 1]), 1.2 * size, "#41362a", 7);
      scene.tube(p([-4, 1, 4]), p([-4, 1, 1]), 1.2 * size, "#41362a", 7);
    }
  }

  function drawChariot(scene, o) {
    const stride = o.moving ? Math.sin(o.time * 10 + o.seed) * 2.2 : 0;
    scene.shadow(15, 4.8);
    for (const y of [-4.8, 4.8])
      scene.tube([-4, y, 3.5], [-4, y + .6, 3.5], 3.8, "#4b3727", 9);
    scene.box([-2, 0, 6], [11, 8, 5], tint(o.color, .7));
    scene.polygon([[-8, -4, 8], [3, -4, 8], [4, -4, 13], [-7, -4, 13]],
      o.color, .7);
    for (const side of [-1, 1]) {
      const y = side * 4.4;
      scene.ellipsoid([8, y, 7.5], [7.5, 2.8, 4.2], "#765333", 8, 3);
      scene.tube([12, y, 8], [15, y, 13], 1.7, "#765333", 7);
      scene.ellipsoid([15.5, y, 14], [2.8, 2.2, 2.3], "#765333", 7, 3);
      for (const x of [4, 11]) {
        scene.tube([x, y, 5.5], [x + stride * .35, y, 2.5], .62, "#553a27", 6);
        scene.tube([x + stride * .35, y, 2.5], [x + stride, y, .4], .5,
          "#443024", 6);
      }
      scene.tube([1, side * 2.7, 8], [13, y, 9], .24, "#9b7b52", 5);
    }
    human(scene, {...o, civilian:false}, [-3, 0, 8], .64);
  }

  function drawMounted(scene, o) {
    if (CHARIOTS.has(o.type)) return drawChariot(scene, o);
    const stride = o.moving ? Math.sin(o.time * 9 + o.seed) : 0;
    scene.shadow(12, 4.2);
    scene.ellipsoid([0, 0, 8], [10.5, 4.2, 5.4], "#765333", 9, 4);
    scene.tube([7, 0, 9], [10, 0, 15], 2.2, "#765333", 7);
    scene.ellipsoid([10.5, 0, 16], [3.4, 2.8, 2.7], "#765333", 7, 3);
    for (const [x, phase] of [[-6, 1], [-2, -1], [3, -1], [7, 1]]) {
      const swing = stride * phase * 2.4;
      scene.tube([x, phase * 1.2, 6], [x + swing * .4, phase * 1.4, 2.7], .75,
        "#553a27", 6);
      scene.tube([x + swing * .4, phase * 1.4, 2.7], [x + swing, phase * 1.1, .4],
        .62, "#443024", 6);
    }
    scene.tube([-9, 0, 9], [-14, 1.4 + stride, 12], .5, "#3c2b22", 6);
    human(scene, {...o, civilian:false}, [-1, 0, 9], .7);
  }

  function drawArmor(scene, o) {
    const recoil = o.action * 3.2;
    scene.shadow(13, 4.4);
    scene.box([0, 0, 4.5], [23, 9, 6.5], "#273239");
    for (const y of [-5, 5]) for (const x of [-8, -3, 3, 8])
      scene.tube([x, y - .5, 2.4], [x, y + .5, 2.4], 2.4, "#566168", 7);
    scene.wedge([0, 0, 8.5], [18, 8, 6], tint(o.color, .76), 1);
    scene.ellipsoid([2, 0, 12], [6.4, 4.4, 3.2], o.color, 8, 3);
    if (o.type === "mobile_sam" || o.type === "anti_air_gun") {
      for (const y of [-2, 2]) scene.tube([0, y, 13], [11, y, 20], .75, "#c2cdd0", 7);
      scene.box([0, 0, 14], [8, 6, 2.5], tint(o.color, .9), [0, -.28, 0]);
    } else {
      scene.tube([3, 0, 13], [17 - recoil, 0, 16], 1.05, "#303b42", 8);
      if (o.action > .6) scene.glow([18 - recoil, 0, 16], 5, "#ffc76a", o.action);
    }
  }

  function drawRobot(scene, o) {
    const stride = o.moving ? Math.sin(o.time * 7 + o.seed) * 2 : 0;
    scene.shadow(10, 4.5, .34);
    for (const side of [-1, 1]) {
      scene.tube([side * 4, 0, 10], [side * (5 + stride), 0, 5.5], 1.6, "#34424b", 7);
      scene.tube([side * (5 + stride), 0, 5.5], [side * (5.5 + stride * 1.5), 0, .8],
        1.25, "#27333b", 7);
      scene.tube([side * 6, 0, 18], [side * (12 + o.action * 2), -1, 13 + o.action * 4],
        1.5, "#34424b", 7);
      scene.glow([side * (12 + o.action * 2), -1, 13 + o.action * 4], 2.8,
        "#77edff", .75);
    }
    scene.box([0, 0, 15], [13, 7, 12], o.color, [0, 0, -.03]);
    scene.box([0, -1, 22], [9, 6, 4.5], tint(o.color, .72));
    scene.glow([0, -4.2, 22.5], 4, "#7ff6ff", .9);
  }

  function drawGun(scene, o) {
    const recoil = o.action * 3.4;
    scene.shadow(12, 4);
    scene.box([-1, 0, 4], [15, 7, 3.5], tint(o.color, .72));
    for (const y of [-4, 4]) scene.tube([-5, y, 3.2], [-5, y + .5, 3.2], 3.2,
      "#303940", 8);
    if (o.type === "rocket_artillery") {
      scene.box([2, 0, 10], [12, 7, 4.5], "#424d52", [0, -.32, 0]);
      for (const y of [-2.2, 0, 2.2]) scene.tube([3, y, 10], [15 - recoil, y, 16],
        .72, "#a7b1b5", 7);
    } else {
      scene.tube([-1, 0, 8], [17 - recoil, 0, 15], o.type === "machine_gun" ? .65 : 1.05,
        "#343f45", 8);
      scene.box([-1, 0, 8], [7, 6, 4], o.color, [0, -.28, 0]);
    }
    human(scene, {...o, type:"infantry", action:o.action * .4}, [-7, 5, 0], .55);
    if (o.action > .62) scene.glow([18 - recoil, 0, 15], 5, "#ffc76a", o.action);
  }

  function drawConvoy(scene, o) {
    const bounce = o.moving ? Math.abs(Math.sin(o.time * 9 + o.seed)) * .55 : 0;
    scene.shadow(13, 4.4);
    for (const x of [-7, 6]) for (const y of [-4.6, 4.6])
      scene.tube([x, y - .5, 2.8], [x, y + .5, 2.8], 2.5, "#303940", 8);
    scene.box([-2, 0, 6 + bounce], [21, 8.5, 5], "#4f5e55");
    scene.box([-6, 0, 10 + bounce], [10, 8, 6], "#7c765b");
    scene.wedge([7, 0, 9 + bounce], [7, 8, 7], o.color, 1);
    scene.glow([10, -4.2, 9 + bounce], 1.8, "#fff1aa", .5);
    scene.box([-6, -4.4, 10 + bounce], [5, .7, 3.4], "#d9dccb");
  }

  function drawSiege(scene, o) {
    const throwAngle = -.45 - o.action * 1.15;
    scene.shadow(12, 4);
    for (const y of [-4, 4]) scene.tube([-5, y, 3], [-5, y + .5, 3], 3, "#4b3626", 8);
    scene.box([0, 0, 5], [18, 7, 3], "#755435");
    if (o.type === "battering_ram") {
      scene.tube([-10 - o.action * 3, 0, 8], [10 + o.action * 3, 0, 8], 1.7,
        "#4d3524", 8);
      for (const x of [-7, 7]) scene.tube([x, -3, 4], [x, -3, 13], .65, "#725235", 6);
    } else if (o.type === "siege_tower") {
      scene.box([0, 0, 13], [11, 8, 20], "#725235");
      for (const z of [7, 13, 19]) scene.box([0, -4.2, z], [12, 1, 1], "#4b3626");
    } else {
      for (const y of [-3, 3]) {
        scene.tube([-5, y, 5], [0, y, 17], .65, "#6d4c30", 6);
        scene.tube([5, y, 5], [0, y, 17], .65, "#6d4c30", 6);
      }
      const tip = [Math.cos(throwAngle) * 17, 0, 8 - Math.sin(throwAngle) * 17];
      scene.tube([-4, 0, 7], tip, .7, "#553b28", 7);
      scene.ellipsoid(tip, [2.6, 2.4, 2.6], "#64666a", 7, 3);
    }
  }

  function drawNaval(scene, o, embarked = false) {
    scene.shadow(15, 4.2, .2);
    const modern = MODERN_SHIPS.has(o.type), submarine = SUBMARINES.has(o.type);
    const hull = embarked ? "#714d30" : (modern || submarine ? "#42515b" : "#68482f");
    scene.wedge([0, 0, 5], [25, 9, 7], hull, 1);
    if (submarine) {
      scene.ellipsoid([0, 0, 6.5], [12, 4.4, 4], hull, 10, 4);
      scene.box([-1, 0, 11], [5, 4, 4], "#35434b");
      scene.tube([0, 0, 12], [0, 0, 17], .38, "#809096", 6);
    } else if (modern) {
      const deck = o.type === "aircraft_carrier" ? [25, 8, 1.4] : [13, 7, 1.8];
      scene.box([0, 0, 9], deck, tint(o.color, .82));
      if (o.type === "aircraft_carrier") {
        scene.polygon([[-8, -2, 10], [5, -2, 10], [0, -2, 11.2]], "#d8e3e5", .45);
        scene.box([7, 2, 12], [5, 4, 6], "#657078");
      } else {
        scene.ellipsoid([4, 0, 12], [3.4, 3, 2.2], o.color, 7, 3);
        scene.tube([5, 0, 13], [15 - o.action * 3, 0, 15], .65, "#2e3940", 7);
      }
    } else {
      scene.tube([0, 0, 7], [0, 0, 24], .45, "#533a28", 7);
      scene.polygon([[0, 0, 23], [0, 0, 9], [10, 0, 14]], o.color, .5);
      if (embarked) human(scene, {...o, type:"settler", civilian:true}, [-4, 0, 7], .45);
    }
    const wake = Math.sin(o.time * 4 + o.seed) * 1.2;
    scene.glow([-12, 4 + wake, 2], 4.5, "#bfeef4", .22);
  }

  function drawAir(scene, o) {
    const jet = o.type !== "biplane";
    scene.shadow(13, 3.5, .13);
    scene.tube([-11, 0, 12], [13, 0, 12], 2.1, tint(o.color, .82), 9);
    scene.wedge([12, 0, 12], [5, 5, 4], tint(o.color, 1.15), 1);
    scene.polygon([[-5, -2, 12], [5, -15, 12], [8, -2, 12],
      [8, 2, 12], [5, 15, 12], [-5, 2, 12]], o.color, .8);
    scene.polygon([[-9, -1, 12], [-13, -7, 14], [-7, -1, 14],
      [-7, 1, 14], [-13, 7, 14], [-9, 1, 12]], tint(o.color, .75), .65);
    if (jet) {
      scene.glow([-12, 0, 12], 4 + o.action * 3, "#6de7ff", .5 + o.action * .35);
    } else {
      const spin = o.time * 20;
      scene.tube([14, Math.cos(spin) * 7, 12 + Math.sin(spin) * 7],
        [14, -Math.cos(spin) * 7, 12 - Math.sin(spin) * 7], .2, "#eee5cf", 5);
      scene.polygon([[-4, -14, 16], [5, -14, 16], [6, 14, 16], [-4, 14, 16]],
        tint(o.color, 1.08), .5);
    }
  }

  function drawRotor(scene, o) {
    const spin = o.time * 22;
    scene.shadow(12, 4, .14);
    scene.ellipsoid([2, 0, 11], [9, 5, 5.5], o.color, 9, 4);
    scene.tube([-3, 0, 12], [-17, 0, 15], 1.2, "#344148", 8);
    scene.box([-17, 0, 15], [2, 7, 6], tint(o.color, .72));
    scene.tube([0, 0, 17], [Math.cos(spin) * 18, Math.sin(spin) * 18, 17.3], .22,
      "#d8e5e5", 5);
    scene.tube([0, 0, 17], [-Math.cos(spin) * 18, -Math.sin(spin) * 18, 17.3], .22,
      "#d8e5e5", 5);
    scene.glow([7, -4.2, 11], 2.2, "#9fe5f1", .45);
  }

  function drawBalloon(scene, o) {
    const sway = Math.sin(o.time * 1.7 + o.seed) * .8;
    scene.shadow(7, 2.4, .12);
    scene.ellipsoid([sway, 0, 23], [8, 7, 11], tint(o.color, .74), 10, 5);
    scene.ellipsoid([sway - 2, -2, 25], [3, 2.3, 7], tint(o.color, 1.22), 7, 4);
    for (const x of [-3, 3]) scene.tube([x + sway, 0, 13], [x * .55, 0, 7], .18,
      "#7c6344", 5);
    scene.box([0, 0, 5.5], [6, 5, 5], "#795936");
  }

  function drawDrone(scene, o) {
    const spin = o.time * 28;
    scene.shadow(9, 3, .12);
    scene.box([0, 0, 12], [9, 6, 4], "#56666f");
    for (const sx of [-1, 1]) for (const sy of [-1, 1]) {
      const hub = [sx * 9, sy * 7, 13];
      scene.tube([sx * 3, sy * 2, 12], hub, .5, "#9aa8ad", 6);
      scene.tube([hub[0] + Math.cos(spin * sx) * 5, hub[1] + Math.sin(spin * sx) * 5, 13],
        [hub[0] - Math.cos(spin * sx) * 5, hub[1] - Math.sin(spin * sx) * 5, 13],
        .15, "#d5dfdf", 5);
    }
    scene.glow([2, -3.2, 11], 2.3, "#64ecff", .7);
  }

  // ---------------------------------------------------------------- map environment
  // These are intentionally low-poly models rather than camera-facing image
  // cutouts. Every vertex is expressed in the same world plane as a hex, then
  // yawed and tilted by Scene.project. Deterministic seeds keep a forest or
  // ridge stable between frames while still avoiding a repeated tile stamp.
  function seeded(seed) {
    let value = (Math.floor(Number(seed) || 1) ^ 0x9e3779b9) >>> 0;
    return () => {
      value ^= value << 13; value ^= value >>> 17; value ^= value << 5;
      return (value >>> 0) / 4294967296;
    };
  }

  // A shadow is a shadow, not a wafer. Modelling it as a flat black ellipsoid
  // meant the scene stroked all twenty of its faces, so what landed on the map
  // was a wheel of black spokes over ground that never actually darkened — the
  // fill is a fifth of an alpha, the outlines are not. It only stayed hidden
  // because the model standing on it happened to cover it.
  function groundShadow(scene, radius, alpha = .2) {
    scene.shadow(radius, radius * .68 * scene.tilt, alpha);
  }

  function crag(scene, x, y, radius, height, color, snow, random, fine = true) {
    const sides = (fine ? 7 : 5) + Math.floor(random() * 3);
    const turn = random() * Math.PI * 2;
    scene.cone([x, y, 0], radius, height, color, sides, .3, turn);
    if (snow && height > 16) {
      const snowLine = height * (.66 + random() * .08);
      scene.cone([x, y, snowLine], radius * .37, height - snowLine + .2,
        "#e8edf0", sides, .2, turn, .96);
    }
  }

  // The direction a range actually runs, from the edges this tile shares with
  // other mountains. Two opposite neighbours are one line, not two vectors, so
  // the axis comes from the orientation tensor rather than a vector sum, which
  // would cancel them to nothing. Returns null for a mountain standing alone.
  function ridgeAxis(ridge) {
    if (!ridge.length) return null;
    let xx = 0, xy = 0, yy = 0;
    for (const [dx, dy] of ridge) {
      const length = Math.hypot(dx, dy) || 1;
      const ux = dx / length, uy = dy / length;
      xx += ux * ux; xy += ux * uy; yy += uy * uy;
    }
    const angle = .5 * Math.atan2(2 * xy, xx - yy);
    return [Math.cos(angle), Math.sin(angle)];
  }

  // A range is one landform that happens to occupy several tiles, so a mountain
  // builds two things. Its own massif is turned to lie along the direction the
  // range runs, which gives a run of tiles a single crest line instead of the
  // same three peaks stamped over and over. Then every edge it shares with
  // another mountain carries a saddle standing exactly on the seam: the
  // neighbour builds the identical saddle from the identical edge seed, so the
  // tile boundary falls inside solid rock rather than into the valley of open
  // ground that used to separate two scoops.
  function mountains(scene, o, monument = false) {
    const random = seeded(o.seed + 71);
    const span = o.span || 1;
    const snowy = monument || o.terrain === "snow" || o.polar;
    const sandstone = o.terrain === "desert" && !snowy;
    const base = sandstone ? "#a66e45" : "#746f66";
    // Ridge offsets are tile-to-tile distances in the map's own world units, so
    // unlike the model's own coordinates they are never scaled by `span`.
    const ridge = Array.isArray(o.ridge) ? o.ridge : [];
    groundShadow(scene, (24 + ridge.length * 2.4) * span, .25);
    const axis = ridgeAxis(ridge) || [1, 0];
    // Zoomed out the shoulders go and the cones lose two sides each. The col
    // is what joins one tile to the next and stays at every distance; the
    // shoulder only smooths the run between a peak and its col, which is
    // nothing anybody can see once a hex is forty pixels across.
    const fine = o.detail;
    for (const [dx, dy, edgeSeed, owns] of ridge) {
      const edge = seeded(Math.abs(Math.floor(edgeSeed) || 1) + 13);
      const shoulder = .68 + edge() * .06;
      // The col belongs to the edge and is raised once, by whichever tile owns
      // it: rock is opaque, so building it from both sides would be invisible
      // except for the outline, which would be laid down twice and come out
      // darker than every other ridge on the map. The shoulder carrying this
      // tile's own crest down into the col is per-tile.
      if (owns) {
        footing(scene, dx * .5, dy * .5, 21, base, fine);
        crag(scene, dx * .5, dy * .5, 15.5 + edge() * 2.5, 24 + edge() * 7,
          base, snowy, edge, fine);
      }
      if (fine)
        crag(scene, dx * .5 * shoulder, dy * .5 * shoulder,
          13 + random() * 3, 28 + random() * 8, base, snowy, random, fine);
    }
    // Along the ridge, across it, radius, height. Rotating these onto the axis
    // is what turns a row of identical stamps into a crest.
    const peaks = monument
      ? [[0, 0, 20, 55], [-18, 3, 13, 36], [17, 4, 14, 40]]
      : [[-15, 1, 15, 32], [1, -2, 20, 44], [16, 4, 13, 28]];
    footing(scene, 0, 0, 26, base, fine);
    for (const [along, across, radius, height] of peaks) {
      const x = (along * axis[0] - across * axis[1]) * span;
      const y = (along * axis[1] + across * axis[0]) * span;
      crag(scene, x, y, radius * span * (.9 + random() * .18),
        height * span * (.9 + random() * .2), base, snowy, random, fine);
    }
  }

  // The rock the crags stand on. Without it a massif is a handful of cones with
  // the flat tile top showing between them, and a range reads as beads on a
  // string; overlapping footings merge into one continuous apron.
  function footing(scene, x, y, radius, color, fine = true) {
    scene.cone([x, y, 0], radius, 5.5, color, fine ? 9 : 6, radius * .74,
      radius * .13);
  }

  function hills(scene, o) {
    groundShadow(scene, 18, .12);
    const color = o.terrain === "desert" ? "#b78955"
      : o.terrain === "snow" ? "#cbd3cf" : "#738153";
    // A seam mound was tried here and taken out again: a mountain saddle joins
    // two masses that are already solid, but these mounds have air between
    // them by design, and bridging every shared edge turned hill country into
    // a carpet of overlapping coins rather than into rolling ground. Hills
    // flow in the painted view, where the relief is light and shade laid over
    // the ground rather than a body standing on it.
    // Centred, and the same three mounds on every hill tile: the ground under
    // a hill is what the rest of the tile is built on, so it belongs in the
    // middle of the face. Anything the tile also carries rides on top of it —
    // see the lift `woodland` takes when `hills` is set.
    // The flat views drop their hill symbol to two fifths of its height; this
    // one deliberately does not follow. An ellipsoid draws its whole body, so
    // squashing it does not make a lower dome — it makes a wafer, and three
    // wafers with visible side walls read as coins laid on the tile. Height is
    // what gives these mounds a lit flank to be read by at all.
    for (const [x, y, rx, ry, rz] of [[-10, 2, 10.5, 7.5, 4.6],
      [4, -5, 11, 7, 4.4], [11, 6, 9.5, 6.5, 4]])
      scene.ellipsoid([x, y, 1.4], [rx, ry, rz], color,
        o.detail ? 9 : 6, o.detail ? 3 : 2, .82);
  }

  // Woods and rainforest are stamped, not scattered. Scattering every hex from
  // its own seed gave each tile a different arrangement of trees at different
  // heights in a different green, so a wood read as a run of unrelated clumps
  // with a visible seam at every boundary rather than as one forest. One woods
  // and one rainforest, repeated: what tells them apart is species, not noise.
  // Firs are squat triangles; rainforest is a tall bare trunk under a broad
  // crown, and neighbouring crowns overlap into the closed canopy that a fir
  // stand deliberately does not have.
  const CONIFERS = [                       // x, y, height, tone
    [-13, -10, 13.5, 0], [1, -13, 15.5, 1], [12, -8, 12.5, 2],
    [-7, -2, 16.5, 1], [8, 2, 14, 0], [-14, 7, 13, 2],
    [0, 10, 15, 1], [13, 10, 12, 0],
  ];
  const CONIFER_GREENS = ["#24562d", "#2d6636", "#1b4825"];
  // Ordered so that dropping every other tree still leaves one on each side of
  // the face rather than a bare half.
  const RAINFOREST = [
    [-14, -6, 21, 0], [2, -12, 22, 1], [13, 2, 19, 2],
    [-8, 8, 20, 1], [1, 13, 18, 0],
  ];
  const RAINFOREST_GREENS = ["#17613b", "#237447", "#145433"];

  // Where a wood's trees actually stand. The stand tables above fixed the
  // arrangement so that a wood stopped being a fresh arrangement of trees at
  // every hex — but a *fixed* arrangement inset into every face is the same
  // grid stated the other way round: identical clumps in the middle of each
  // hex with a bare lane along every boundary, which is what a forest looked
  // like from any distance. Trees are placed on a lattice in WORLD space
  // instead and kept by whichever tile contains them, so a canopy runs across
  // a tile boundary without repeating and without a seam. A tree may lean out
  // over an edge the wood continues across and is pulled well back from one it
  // does not, which is what gives a wood a soft edge against open ground and a
  // closed middle. See `ice`, which fills a sheet the same way.
  function standPoints(o, step, random) {
    const frozen = frozenEdges(o.ridge);
    const limit = [];
    for (let k = 0; k < 6; k++) limit.push(frozen.has(k) ? INRADIUS + 4 : INRADIUS - 10);
    const ox = o.origin ? o.origin[0] : 0, oy = o.origin ? o.origin[1] : 0;
    const out = [];
    for (let i = Math.floor((ox - 42) / step); i <= Math.ceil((ox + 42) / step); i++)
      for (let j = Math.floor((oy - 42) / step); j <= Math.ceil((oy + 42) / step); j++) {
        const cell = seeded(Math.imul(i + 8192, 374761393) ^
          Math.imul(j + 8192, 668265263));
        const px = i * step + (cell() - .5) * step * .9 - ox;
        const py = j * step + (cell() - .5) * step * .9 - oy;
        let keep = true;
        for (let k = 0; k < 6 && keep; k++) {
          const a = k * Math.PI / 3;
          if (px * Math.cos(a) + py * Math.sin(a) > limit[k]) keep = false;
        }
        if (keep) out.push([px, py, cell(), cell()]);
      }
    // Nothing at all is worse than a lattice: a tile whose lattice cells all
    // fell outside it would be a hole in the wood.
    if (!out.length) out.push([0, 0, random(), random()]);
    return out;
  }

  function woodland(scene, o, jungle) {
    const burnt = o.kind.startsWith("burnt_");
    const burning = o.kind.startsWith("burning_");
    const greens = jungle ? RAINFOREST_GREENS : CONIFER_GREENS;
    const random = seeded(o.seed + 131);
    const step = (jungle ? 21 : 18) * (o.detail ? 1 : 1.45);
    const spread = jungle ? RAINFOREST : CONIFERS;   // heights, kept for scale
    const low = Math.min(...spread.map(s => s[2])), high = Math.max(...spread.map(s => s[2]));
    const trees = standPoints(o, step, random);
    const sides = o.detail ? 8 : 6;
    // A wood on a hill grows on the hill, not through it. Standing the trunks
    // on the mound and setting the stand further back puts the trees in the
    // upper half of the face, where the crest is, instead of half-buried in
    // relief that is drawn after them.
    const crest = o.hills ? 3.8 : 0;
    const back = o.hills ? -6 : 0;
    // The shade a wood casts is the wood's, so it answers to how much of the
    // face this one actually covers. A fixed disc under a corner tile holding
    // three trees is a grey plate with a copse standing on it.
    groundShadow(scene, Math.min(20, 7 + trees.length * 1.5), burnt ? .28 : .2);
    trees.forEach(([x, ty, sizeRoll, toneRoll], i) => {
      const height = low + sizeRoll * (high - low);
      const tone = Math.min(2, Math.floor(toneRoll * 3));
      const y = ty + back;
      const trunkTop = crest + height * (jungle ? .74 : .3);
      scene.tube([x, y, crest], [x, y, trunkTop], jungle ? .85 : .7,
        burnt ? "#332b25" : "#5b3d24", 6);
      if (burnt) {
        scene.tube([x, y, crest + (trunkTop - crest) * .72],
          [x - 3, y + 1, crest + height * .83],
          .32, "#282521", 5);
        return;
      }
      const green = greens[tone];
      if (jungle) {
        // A dome, not a parasol. A cone wide at the bottom shows the map its
        // underside — six of them per hex read as a row of beach umbrellas on
        // sticks. A flattened ellipsoid hides its own floor behind its top and
        // merges with its neighbours into one canopy.
        const crown = o.detail ? 10 : 9;
        scene.ellipsoid([x, y, trunkTop], [crown, crown * .86, 3.6], green,
          sides, o.detail ? 3 : 2);
        scene.ellipsoid([x - crown * .3, y + .8, trunkTop + 1.5],
          [crown * .54, crown * .46, 2.6], tint(green, 1.12), sides,
          o.detail ? 3 : 2);
      } else {
        scene.cone([x, y, crest + height * .22], 6.2, height * .62,
          green, 7, .15, tone);
        scene.cone([x, y, crest + height * .5], 4.8, height * .56,
          tint(green, 1.08), 7, .08, tone + .25);
      }
      if (burning && i % 3 !== 2) {
        scene.cone([x + 1, y - .5, crest + 1], 2.5, 9, "#e34c1c", 6, .1, tone, .88);
        scene.cone([x + 1, y - .5, crest + 2], 1.25, 6, "#ffd34d", 6, .05, tone, .92);
        scene.glow([x + 1, y, crest + 5], 7, "#ff6a24", .34);
      }
    });
  }

  function waterPatch(scene, color, scale = 1, alpha = .88) {
    const points = [];
    for (let i = 0; i < 10; i++) {
      const a = i * Math.PI * 2 / 10;
      const r = (i & 1 ? 13 : 16) * scale;
      points.push([Math.cos(a) * r, Math.sin(a) * r, .3]);
    }
    scene.polygon(points, color, .45, alpha);
  }

  function wetlands(scene, o, dense = false) {
    const random = seeded(o.seed + 211);
    waterPatch(scene, dense ? "#47775f" : "#467968", 1, .58);
    const count = dense ? (o.detail ? 14 : 8) : (o.detail ? 10 : 5);
    for (let i = 0; i < count; i++) {
      const x = (random() - .5) * 29, y = (random() - .5) * 21;
      const h = 4 + random() * 5;
      scene.tube([x, y, .5], [x + (random() - .5), y, h], .22,
        i % 3 ? "#51743d" : "#9a7a43", 5);
    }
  }

  function palm(scene, x, y, size, turn) {
    const bend = [x + Math.cos(turn) * 2.2 * size, y + Math.sin(turn) * 2.2 * size,
      8.5 * size];
    const crown = [bend[0] + Math.cos(turn) * 1.2 * size,
      bend[1] + Math.sin(turn) * 1.2 * size, 14 * size];
    scene.tube([x, y, 0], bend, .72 * size, "#76502d", 7);
    scene.tube(bend, crown, .55 * size, "#8a6035", 7);
    for (let i = 0; i < 7; i++) {
      const a = turn + i * Math.PI * 2 / 7;
      const reach = (6 + (i & 1) * 1.5) * size;
      const side = [-Math.sin(a) * 1.25 * size, Math.cos(a) * 1.25 * size, 0];
      const tip = [crown[0] + Math.cos(a) * reach, crown[1] + Math.sin(a) * reach,
        crown[2] - 1.7 * size];
      scene.polygon([crown, add(tip, side), tip, sub(tip, side)],
        i & 1 ? "#28713c" : "#1d5e32", .18);
    }
  }

  function oasis(scene, o) {
    waterPatch(scene, "#358bab", .72, .9);
    groundShadow(scene, 17, .1);
    palm(scene, -7, 2, .92, .3);
    palm(scene, 8, 5, .75, 2.2);
  }

  function floodplain(scene, o) {
    const random = seeded(o.seed + 251);
    for (let row = -1; row <= 1; row++) {
      const points = [];
      for (let i = -3; i <= 3; i++)
        points.push([i * 5, row * 5 + Math.sin(i * 1.7 + row) * 1.4, .28]);
      for (let i = 0; i < points.length - 1; i++)
        scene.tube(points[i], points[i + 1], .42, "#8e7344", 5);
    }
    for (let i = 0; i < 5; i++) {
      const x = (random() - .5) * 28, y = (random() - .5) * 17;
      scene.tube([x, y, .3], [x, y, 3.5 + random() * 2], .18, "#68783b", 5);
    }
  }

  function reef(scene, o, grand = false) {
    const random = seeded(o.seed + 307);
    waterPatch(scene, "#2c91a2", grand ? 1.25 : .9, .42);
    const count = grand ? (o.detail ? 14 : 8) : (o.detail ? 9 : 4);
    const colors = ["#dc765d", "#e7b45c", "#6dc2a6", "#9b6ab3"];
    for (let i = 0; i < count; i++) {
      const a = random() * Math.PI * 2, r = Math.sqrt(random()) * (grand ? 20 : 14);
      const x = Math.cos(a) * r, y = Math.sin(a) * r, h = 2 + random() * 5;
      const color = colors[i % colors.length];
      scene.tube([x, y, .5], [x, y, h], .45 + random() * .4, color, 6);
      if (i & 1) {
        scene.tube([x, y, h * .58], [x + (random() - .5) * 4,
          y + (random() - .5) * 4, h + 1], .32, color, 5);
      }
    }
  }

  function geothermal(scene, o) {
    const random = seeded(o.seed + 331);
    for (let i = 0; i < 8; i++) {
      const a = i * Math.PI / 4, r = 8 + random() * 4;
      crag(scene, Math.cos(a) * r, Math.sin(a) * r, 2.6 + random() * 2,
        2 + random() * 4, "#625c50", false, random);
    }
    waterPatch(scene, "#58aaa5", .5, .78);
    for (const [x, y, h] of [[-4, 0, 12], [3, -3, 16], [7, 3, 10]]) {
      scene.tube([x, y, 2], [x + 1.5, y, h], .5, "#dcebea", 6);
      scene.glow([x + 1.5, y, h], 3.2, "#edf8f6", .22);
    }
  }

  // Which of the six edges this tile shares with its own kind, as an index
  // 0..5 where edge k faces k * 60 degrees in the map's world plane. Segment k
  // of a hex outline runs from corner k to corner k+1 and spans edge k, so a
  // corner is flanked by edges k-1 and k.
  function frozenEdges(ridge) {
    const set = new Set();
    for (const [dx, dy] of (Array.isArray(ridge) ? ridge : []))
      set.add((Math.round(Math.atan2(dy, dx) * 3 / Math.PI) % 6 + 6) % 6);
    return set;
  }

  // A polar cap is one sheet, not a raft of identical floes on a hex lattice.
  // How far the slab reaches at a corner is a property of the CORNER rather
  // than of the tile: all three hexes meeting there weigh the same two edges
  // and so agree on the same answer, which makes neighbouring slabs share a
  // boundary exactly instead of leaving a lane of open water between them.
  // Walls are raised only on the segments facing the sea, so the inside of a
  // sheet has no cliffs running through it and only its outer edge reads as
  // floe. For the same reason the ice scene is drawn without an outline: a
  // stroke along a shared boundary would be laid down by both tiles and put
  // the hex grid straight back.
  function ice(scene, o) {
    const random = seeded(o.seed + 367);
    const frozen = frozenEdges(o.ridge);
    const TOP = 1.9, FLOOR = -.4;
    const reach = [];
    for (let i = 0; i < 6; i++) {
      const joined = (frozen.has((i + 5) % 6) ? 1 : 0) + (frozen.has(i) ? 1 : 0);
      // Both edges frozen: the corner is inside the sheet and takes the whole
      // hex. One: it reaches most of the way, and the neighbour sharing that
      // edge computes the same number. Neither: open water, so it is a floe
      // corner and free to be ragged.
      reach.push(joined === 2 ? 36 : joined === 1 ? 30 : 18 + random() * 6);
    }
    const outline = [], seaward = [];
    for (let i = 0; i < 6; i++) {
      const a = (60 * i - 30) * Math.PI / 180;
      outline.push([Math.cos(a) * reach[i], Math.sin(a) * reach[i]]);
      seaward.push(!frozen.has(i));
      const mid = (60 * i) * Math.PI / 180;
      if (frozen.has(i)) {
        // Just past the shared edge, so the two slabs overlap by a hair rather
        // than meeting on it. Sharing a boundary exactly is not enough: both
        // sides antialias against the water and the seam comes back as a
        // hairline. They are opaque and the same colour, so the overlap is
        // invisible and the hex grid is gone.
        outline.push([Math.cos(mid) * (INRADIUS + .9),
          Math.sin(mid) * (INRADIUS + .9)]);
        seaward.push(false);
        continue;
      }
      // A scallop halfway along every open edge, so the sea side breaks up
      // instead of running straight from corner to corner.
      const r = Math.min(reach[i], reach[(i + 1) % 6]) * (.86 + random() * .17);
      outline.push([Math.cos(mid) * r, Math.sin(mid) * r]);
      seaward.push(true);
    }
    const n = outline.length;
    const points = outline.map(p => [p[0], p[1], TOP])
      .concat(outline.map(p => [p[0], p[1], FLOOR]));
    // `mesh` reads a face's normal off its first three vertices, which is the
    // turn at the *second* one — and a floe edge is not convex everywhere, so
    // starting anywhere would sometimes hand it a reflex corner and light the
    // whole slab as if it faced away. Starting one before the vertex that
    // reaches furthest puts a guaranteed hull corner in that seat.
    let far = 0;
    for (let i = 1; i < n; i++)
      if (Math.hypot(...outline[i]) > Math.hypot(...outline[far])) far = i;
    const top = [];
    for (let i = 0; i < n; i++) top.push((far - 1 + n + i) % n);
    // The slab is the ground of this model, and everything else stands on it.
    scene.mesh(points, [top], "#cfe1e5", 1, -2e4);
    const walls = [];
    for (let i = 0; i < n; i++) {
      if (!seaward[i]) continue;
      const j = (i + 1) % n;
      walls.push([i, j, n + j, n + i]);
    }
    if (walls.length) scene.mesh(points, walls, "#cfe1e5");
    // Drifts and pressure ridges, placed on a lattice in WORLD space and kept
    // by whichever tile happens to contain them. Anything hashed from the tile
    // instead puts a feature at the same place in every hex, which is how the
    // old floes ended up wearing an identical pair of pinnacles; anything
    // hashed from an edge draws a line along the seam, which is the hex grid
    // by another name. A lattice that knows nothing about tiles is the only
    // arrangement that can cross one.
    const step = o.detail ? 38 : 52;
    const ox = o.origin ? o.origin[0] : 0, oy = o.origin ? o.origin[1] : 0;
    for (let i = Math.floor((ox - 36) / step); i <= Math.ceil((ox + 36) / step); i++)
      for (let j = Math.floor((oy - 36) / step); j <= Math.ceil((oy + 36) / step); j++) {
        const cell = seeded(Math.imul(i + 8192, 374761393) ^
          Math.imul(j + 8192, 668265263));
        const px = i * step + (cell() - .5) * step * .95 - ox;
        const py = j * step + (cell() - .5) * step * .95 - oy;
        if (!inHex(px, py) || !inOutline(outline, px, py)) continue;
        if (cell() > .88) {
          // Light: a ridge block on a white sheet spends most of its area in
          // its own shade, and at the sheet's own colour that comes out as a
          // near-black stud rather than as ice.
          crag(scene, px, py, 4 + cell() * 3, 7 + cell() * 6,
            "#e9f4f6", true, cell, o.detail);
          continue;
        }
        // Drift and bare ice are painted, not modelled. Anything with a body
        // — a low cone, a squashed ellipsoid — puts a rim and a shaded far
        // side on a surface that is flat, and a field of them reads as coins
        // laid on the sheet. These are coplanar patches lying in the slab's
        // own plane, so they take exactly the slab's light and differ only in
        // colour. They can be drawn at all because the bias orders them after
        // the slab; on real depth the ones toward the back sort behind it.
        const patch = [], sides = o.detail ? 7 : 5;
        const rx = 12 + cell() * 9;
        for (let k = 0; k < sides; k++) {
          const a = k * Math.PI * 2 / sides;
          const r = rx * (.72 + cell() * .45);
          patch.push([px + Math.cos(a) * r, py + Math.sin(a) * r * .78, TOP]);
        }
        scene.mesh(patch, [[...Array(sides).keys()]],
          cell() > .5 ? "#dff0f2" : "#bfd6dd", 1, -1e4);
      }
  }

  const INRADIUS = 36 * Math.sqrt(3) / 2;
  // Inside this tile's own hexagon: three edge normals, each tested both ways.
  function inHex(x, y) {
    for (let k = 0; k < 3; k++) {
      const a = k * Math.PI / 3;
      if (Math.abs(x * Math.cos(a) + y * Math.sin(a)) > INRADIUS - 3) return false;
    }
    return true;
  }

  function inOutline(outline, x, y) {
    let inside = false;
    for (let i = 0, j = outline.length - 1; i < outline.length; j = i++) {
      const [xi, yi] = outline[i], [xj, yj] = outline[j];
      if ((yi > y) !== (yj > y) &&
          x < (xj - xi) * (y - yi) / (yj - yi) + xi) inside = !inside;
    }
    return inside;
  }

  function crater(scene, o, lake = false) {
    const random = seeded(o.seed + 401);
    groundShadow(scene, 17, .17);
    if (lake) waterPatch(scene, "#337e9d", .7, .9);
    else scene.ellipsoid([0, 0, .35], [10, 8, .45], "#2b2927", 10, 2);
    for (let i = 0; i < 10; i++) {
      const a = i * Math.PI * 2 / 10, r = 12 + random() * 3;
      crag(scene, Math.cos(a) * r, Math.sin(a) * r, 3 + random() * 2,
        2.5 + random() * 4, lake ? "#746b5b" : "#574c42", false, random);
    }
  }

  function volcanicSoil(scene, o) {
    const random = seeded(o.seed + 419);
    for (let i = 0; i < 10; i++) {
      const a = random() * Math.PI * 2, r = Math.sqrt(random()) * 16;
      const size = 1.8 + random() * 3.2;
      scene.ellipsoid([Math.cos(a) * r, Math.sin(a) * r, size * .22],
        [size, size * (.65 + random() * .3), size * .45],
        i % 3 ? "#403934" : "#5b4538", 7, 3);
    }
  }

  function volcano(scene, o) {
    const random = seeded(o.seed + 439);
    groundShadow(scene, 20, .27);
    scene.cone([0, 0, 0], 19, 29, "#5c5148", 10, 5.8, random() * 6.28);
    scene.ellipsoid([0, 0, 29.2], [5.4, 5.4, .7], "#211d1c", 10, 2);
    if (o.active) {
      scene.ellipsoid([0, 0, 29.8], [3.8, 3.8, .5], "#ef5b25", 9, 2);
      scene.glow([0, 0, 31], 9, "#ff6a25", .52);
      const sway = Math.sin(o.time * .7 + o.seed) * 2.5;
      scene.ellipsoid([sway * .2, 0, 35], [4.3, 3.8, 3.2],
        "#696863", 8, 3, .7);
      scene.ellipsoid([sway, 1, 42], [6.2, 5.4, 4.5],
        "#858680", 8, 3, .52);
      scene.ellipsoid([sway * 1.7, 2, 50], [8, 6.5, 5.2],
        "#a1a49e", 8, 3, .34);
    }
  }

  function uluru(scene, o) {
    groundShadow(scene, 23 * (o.span || 1), .22);
    scene.cone([0, 0, 0], 23 * (o.span || 1), 13 * (o.span || 1),
      "#b85e35", 10, 13 * (o.span || 1), .25);
  }

  function yosemite(scene, o) {
    const random = seeded(o.seed + 487);
    groundShadow(scene, 24 * (o.span || 1), .24);
    for (const [x, y, r, h] of [[-10, 1, 11, 34], [8, 3, 9, 27]])
      scene.cone([x, y, 0], r * (o.span || 1), h * (o.span || 1),
        "#85817a", 8, r * .62, random() * 6.28);
  }

  function pamukkale(scene, o) {
    const span = o.span || 1;
    groundShadow(scene, 20 * span, .16);
    for (let i = 0; i < 5; i++)
      scene.cone([0, i * 1.5, i * 3 * span], (18 - i * 2.4) * span,
        2.2 * span, i & 1 ? "#dce7df" : "#edf0e7", 10,
        (16 - i * 2.3) * span, i * .17);
    waterPatch(scene, "#79c7cf", .55 * span, .72);
  }

  // --- the rest of the Natural Wonder roster.
  //
  // Twenty-six of the thirty-four had no model at all, so the cinematic view
  // drew bare ground where a landmark stood. Each is built from the same
  // primitives as the eight that did: what makes the Matterhorn a Matterhorn
  // and not a mountain is its proportion — one horn, no range — so these are
  // small, specific compositions rather than a shared "wonder" blob.

  // A single sharp peak: one summit, not a range. Used for the horn-shaped
  // mountains, which are the ones a player recognises by silhouette.
  function lonePeak(scene, o, {radius = 17, height = 52, color = "#7a746a",
    snow = .62, skirt = 0} = {}) {
    const span = o.span || 1;
    const random = seeded(o.seed + 503);
    groundShadow(scene, (radius + 6) * span, .24);
    if (skirt) {
      scene.cone([0, 0, 0], (radius + skirt) * span, height * .28 * span,
        color, 9, radius * .8 * span, .3);
    }
    const turn = random() * 6.28;
    scene.cone([0, 0, 0], radius * span, height * span, color, 9,
      radius * .08 * span, turn);
    // The cap has to sit slightly proud of the rock it covers, or the two
    // cones are coplanar and the depth sort is free to hide the snow inside
    // the mountain — which is exactly what a Matterhorn with no snow on it
    // looked like.
    if (snow > 0) {
      const line = height * snow;
      scene.cone([0, 0, line * span], (radius * (1 - snow) + 1.2) * span,
        (height - line) * span, "#eef3f6", 9, radius * .06 * span, turn);
    }
  }

  // A flat-topped tower of rock. Devils Tower, a tepui, the Torres, the
  // Tsingy needles: all the same body at different counts and proportions.
  function stoneTowers(scene, o, {towers, color = "#8a7a66", cap = null,
    fluted = false} = {}) {
    const span = o.span || 1;
    const random = seeded(o.seed + 509);
    groundShadow(scene, 22 * span, .22);
    for (const [x, y, radius, height] of towers) {
      const sides = fluted ? 12 : 8;
      scene.cone([x * span, y * span, 0], radius * span, height * span,
        color, sides, radius * .82 * span, random() * 6.28);
      if (cap) {
        scene.ellipsoid([x * span, y * span, height * span],
          [radius * .84 * span, radius * .84 * span, 1.1 * span], cap, sides, 2);
      }
    }
  }

  // Banded or domed ground: the striped hills, the petroglyph shelves, the
  // chocolate drops. The colours are what separate them.
  function banded(scene, o, {colors, count = 9, radius = 6, height = 5,
    dome = false} = {}) {
    const span = o.span || 1;
    const random = seeded(o.seed + 521);
    groundShadow(scene, 20 * span, .16);
    for (let i = 0; i < count; i++) {
      const a = random() * Math.PI * 2, r = Math.sqrt(random()) * 16 * span;
      const x = Math.cos(a) * r, y = Math.sin(a) * r;
      const size = radius * (.7 + random() * .6) * span;
      const tall = height * (.6 + random() * .9) * span;
      const color = colors[i % colors.length];
      if (dome) scene.ellipsoid([x, y, tall * .3], [size, size * .9, tall], color, 9, 3);
      else scene.cone([x, y, 0], size, tall, color, 7, size * .55, random() * 6.28);
    }
  }

  // Water held in a bowl of rock, with the rim showing. Every lake wonder is
  // this shape; only the water colour tells Retba from Ik-Kil.
  function basin(scene, o, {water = "#337e9d", rim = "#7b7263", rimCount = 9,
    depth = .7, glow: aura = null} = {}) {
    const span = o.span || 1;
    const random = seeded(o.seed + 541);
    groundShadow(scene, 17 * span, .17);
    waterPatch(scene, water, depth * span, .9);
    for (let i = 0; i < rimCount; i++) {
      const a = i * Math.PI * 2 / rimCount, r = (12 + random() * 3) * span;
      crag(scene, Math.cos(a) * r, Math.sin(a) * r, (2.4 + random() * 1.8) * span,
        (2 + random() * 3.5) * span, rim, false, random);
    }
    if (aura) scene.glow([0, 0, 3 * span], 10 * span, aura, .3);
  }

  // A wall of rock with water at its foot: the sea cliffs and the fjords.
  // `face` is the colour of the exposed rock, which is the whole read.
  function seaWall(scene, o, {face = "#efeee6", water = "#2f6d8c",
    inlet = false} = {}) {
    const span = o.span || 1;
    const random = seeded(o.seed + 547);
    groundShadow(scene, 20 * span, .2);
    // A fjord is water first: the cliffs are what a drowned valley looks like
    // from the deck of a ship in it, so the inlet gets the wider patch.
    waterPatch(scene, water, (inlet ? 1.05 : .9) * span, .88);
    const walls = inlet
      ? [[-13, 0, 6, 26], [13, 1, 6, 24], [0, -14, 5, 20]]
      : [[-9, 4, 8, 21], [7, 6, 7, 18]];
    for (const [x, y, radius, height] of walls) {
      scene.cone([x * span, y * span, 0], radius * span, height * span,
        face, 6, radius * .88 * span, random() * 6.28);
    }
  }

  function drawEnvironment(options) {
    if (!options || !options.ctx || !ENVIRONMENT_SET.has(options.kind)) return false;
    const o = {
      ...options,
      seed:Number(options.seed || 1),
      time:Number(options.time || 0) / 1000,
      detail:options.detail !== false,
      span:clamp(Number(options.span || 1), 1, 2.2),
    };
    const scene = new Scene(o.ctx, {scale:o.scale || 1, orthographic:true,
      yaw:Number(o.yaw || 0), tilt:clamp(Number(o.tilt || .64), .28, 1),
      sunAngle:Number.isFinite(o.sunAngle) ? o.sunAngle : Math.PI * 1.25,
      // Ice alone goes unoutlined: its slabs share a boundary with the next
      // tile's exactly, so a stroke there would be drawn twice and redraw the
      // hex grid over a sheet built to hide it. See `ice`.
      stroke:o.detail && o.kind !== "ice" ? "rgba(10,15,13,.38)" : null});
    if (o.kind === "hills") hills(scene, o);
    else if (o.kind === "mountain") mountains(scene, o);
    else if (o.kind === "mount_everest") mountains(scene, o, true);
    else if (["forest", "burning_forest", "burnt_forest"].includes(o.kind))
      woodland(scene, o, false);
    else if (["jungle", "burning_jungle", "burnt_jungle"].includes(o.kind))
      woodland(scene, o, true);
    else if (o.kind === "marsh") wetlands(scene, o);
    else if (o.kind === "pantanal") wetlands(scene, o, true);
    else if (o.kind === "oasis") oasis(scene, o);
    else if (["floodplains", "grassland_floodplains", "plains_floodplains"].includes(o.kind))
      floodplain(scene, o);
    else if (o.kind === "reef") reef(scene, o);
    else if (o.kind === "great_barrier_reef") reef(scene, o, true);
    else if (o.kind === "geothermal_fissure") geothermal(scene, o);
    else if (o.kind === "ice") ice(scene, o);
    else if (o.kind === "volcano") volcano(scene, o);
    else if (o.kind === "impact_zone") crater(scene, o);
    else if (o.kind === "crater_lake") crater(scene, o, true);
    else if (o.kind === "volcanic_soil") volcanicSoil(scene, o);
    else if (o.kind === "uluru") uluru(scene, o);
    else if (o.kind === "yosemite") yosemite(scene, o);
    else if (o.kind === "pamukkale") pamukkale(scene, o);
    else if (o.kind === "dead_sea") {
      waterPatch(scene, "#3f8295", 1.25 * o.span, .88);
      scene.cone([-12, 4, .3], 4, 5, "#d6d1bb", 7, .4, .2);
      scene.cone([11, 1, .3], 3.5, 4, "#e3ddc8", 7, .3, .8);
    }
    // --- the twenty-six that used to draw nothing
    else if (o.kind === "matterhorn")
      lonePeak(scene, o, {radius:15, height:58, color:"#6f6a63", snow:.58});
    else if (o.kind === "kilimanjaro")
      lonePeak(scene, o, {radius:22, height:44, color:"#6b6152", snow:.74, skirt:6});
    else if (o.kind === "mount_roraima")
      stoneTowers(scene, o, {color:"#6d6455", cap:"#5c7248",
        towers:[[-8, 2, 12, 30], [9, -3, 11, 27]]});
    else if (o.kind === "mato_tipila")
      stoneTowers(scene, o, {color:"#9c7550", fluted:true, towers:[[0, 0, 8, 34]]});
    else if (o.kind === "torres_del_paine")
      stoneTowers(scene, o, {color:"#8e8880", cap:"#e7ecef",
        towers:[[-9, 1, 5, 33], [0, -2, 5.5, 38], [9, 2, 5, 31]]});
    else if (o.kind === "tsingy_de_bemaraha")
      stoneTowers(scene, o, {color:"#a9a08c", fluted:true,
        towers:[[-9, 3, 3, 19], [-2, -4, 2.6, 24], [4, 4, 3.2, 17],
          [10, -1, 2.4, 21], [1, 8, 2.8, 15]]});
    else if (o.kind === "giants_causeway") {
      // Basalt columns stepping off a headland into the sea, which is the
      // half of it a pavement of stubs on grass could not say.
      waterPatch(scene, "#2c6d84", .8 * (o.span || 1), .84);
      stoneTowers(scene, o, {color:"#6d766f", fluted:true,
        towers:[[-12, 4, 4, 15], [-6, -1, 4, 20], [0, 3, 4, 17],
          [6, -2, 4, 22], [11, 4, 4, 13], [15, 0, 4, 9]]});
    }
    else if (o.kind === "zhangye_danxia")
      banded(scene, o, {colors:["#b4533c", "#d9924a", "#e6c06a", "#9a5f6d"],
        count:11, radius:7, height:9});
    else if (o.kind === "gobustan")
      banded(scene, o, {colors:["#8d8272", "#a2957f", "#6f6759"],
        count:10, radius:6, height:4});
    else if (o.kind === "chocolate_hills")
      banded(scene, o, {colors:["#8a6a44", "#9c7a4e", "#7a5c3b"],
        count:13, radius:5.5, height:6, dome:true});
    else if (o.kind === "sahara_el_beyda")
      banded(scene, o, {colors:["#eee7d5", "#f5f1e3", "#ded4bc"],
        count:10, radius:5, height:7, dome:true});
    else if (o.kind === "ubsunur_hollow") {
      // A steppe basin, not a swamp: shallow water ringed by dry ground.
      basin(scene, o, {water:"#4d7f7a", rim:"#8a8358", rimCount:7, depth:1.05});
      wetlands(scene, o, true);
    }
    else if (o.kind === "lake_retba")
      basin(scene, o, {water:"#d4708f", rim:"#c3b291", depth:.85});
    else if (o.kind === "ik_kil")
      basin(scene, o, {water:"#1f6f7a", rim:"#5f6c48", rimCount:11, depth:.42});
    else if (o.kind === "fountain_of_youth")
      basin(scene, o, {water:"#63c6c0", rim:"#8d8a6e", depth:.4,
        glow:"#c8fff4"});
    else if (o.kind === "eye_of_the_sahara") {
      const span = o.span || 1;
      groundShadow(scene, 22 * span, .18);
      for (let ring = 3; ring >= 1; ring--) {
        scene.cone([0, 0, 0], ring * 7 * span, (4 - ring) * 2.4 * span,
          ring % 2 ? "#a9834f" : "#c8ab74", 14, ring * 6 * span, .1);
      }
    }
    else if (o.kind === "delicate_arch") {
      const span = o.span || 1;
      groundShadow(scene, 14 * span, .2);
      scene.cone([-6 * span, 0, 0], 3.4 * span, 18 * span, "#b9713f", 8, 2.6 * span, .2);
      scene.cone([6 * span, 0, 0], 3.4 * span, 18 * span, "#a9663a", 8, 2.6 * span, .5);
      scene.tube([-6 * span, 0, 17 * span], [6 * span, 0, 17 * span], 2.4 * span,
        "#c07a45", 7);
    }
    else if (o.kind === "cliffs_of_dover")
      seaWall(scene, o, {face:"#eff0ea", water:"#2f7d9c"});
    else if (o.kind === "piopiotahi")
      seaWall(scene, o, {face:"#5d6b52", water:"#1f5468", inlet:true});
    else if (o.kind === "lysefjord")
      seaWall(scene, o, {face:"#77736a", water:"#27596c", inlet:true});
    else if (o.kind === "ha_long_bay") {
      const span = o.span || 1;
      const random = seeded(o.seed + 563);
      waterPatch(scene, "#2b8296", 1.15 * span, .9);
      for (const [x, y, radius, height] of [[-11, 3, 4.5, 17], [-1, -5, 5.5, 22],
        [7, 4, 4, 14], [13, -3, 3.4, 11]]) {
        scene.cone([x * span, y * span, 0], radius * span, height * span,
          "#5e6f52", 7, radius * .5 * span, random() * 6.28);
      }
    }
    else if (o.kind === "galapagos_islands") {
      const span = o.span || 1;
      waterPatch(scene, "#1f7f95", 1.2 * span, .9);
      for (const [x, y, radius, height] of [[-8, 2, 8, 11], [8, -3, 6.5, 8],
        [3, 8, 4.5, 5]]) {
        scene.cone([x * span, y * span, 0], radius * span, height * span,
          "#6b5f4e", 9, radius * .22 * span, .3);
      }
    }
    else if (o.kind === "vesuvius") volcano(scene, o);
    else if (o.kind === "eyjafjallajokull") {
      volcano(scene, o);
      const span = o.span || 1;
      scene.cone([0, 0, 12 * span], 13 * span, 9 * span, "#dce8ed", 10,
        7 * span, .4, .92);
    }
    else if (o.kind === "paititi") {
      const span = o.span || 1;
      const random = seeded(o.seed + 571);
      groundShadow(scene, 20 * span, .22);
      for (let step = 0; step < 4; step++) {
        scene.cone([0, 0, step * 5 * span], (16 - step * 3.4) * span,
          5 * span, step & 1 ? "#9a8a63" : "#c0b087", 6,
          (13.4 - step * 3.4) * span, .26);
      }
      for (let i = 0; i < 6; i++) {
        const a = random() * Math.PI * 2, r = (12 + random() * 6) * span;
        scene.tube([Math.cos(a) * r, Math.sin(a) * r, .4],
          [Math.cos(a) * r, Math.sin(a) * r, (5 + random() * 4) * span],
          .5 * span, "#3f6b3a", 5);
      }
    }
    else if (o.kind === "bermuda_triangle") {
      const span = o.span || 1;
      waterPatch(scene, "#123f5c", 1.3 * span, .92);
      const points = [[0, -15 * span, .6], [13 * span, 8 * span, .6],
        [-13 * span, 8 * span, .6]];
      scene.polygon(points, "#0d2c46", 1.6, .74);
      scene.glow([0, 0, 4 * span], 15 * span, "#7fd8ef", .26);
      for (let i = 0; i < 3; i++) {
        scene.ellipsoid([Math.sin(o.time * .6 + i * 2) * 5 * span,
          Math.cos(o.time * .5 + i * 2) * 5 * span, (5 + i * 4) * span],
          [7 * span, 6 * span, 2.4 * span], "#9db9c6", 8, 3, .28);
      }
    }
    scene.flush();
    return true;
  }

  function draw(options) {
    if (!options || !FAMILY_SET.has(options.family) || !options.ctx) return false;
    const o = {
      ...options,
      time: Number(options.time || 0) / 1000,
      action: clamp(Number(options.action || 0), 0, 1),
      seed: Number(options.seed || 0) * 1.713,
    };
    const scene = new Scene(o.ctx, {scale:o.scale || 1.04, facing:o.facing,
      bank:o.family === "air" ? Math.sin(o.time * 2.2 + o.seed) * .45 : 0});
    if (o.type === "supply_convoy" && o.family !== "embarked") drawConvoy(scene, o);
    else if (o.family === "mounted") drawMounted(scene, o);
    else if (o.family === "armor") drawArmor(scene, o);
    else if (o.family === "robot") drawRobot(scene, o);
    else if (o.family === "gun") drawGun(scene, o);
    else if (o.family === "siege") drawSiege(scene, o);
    else if (o.family === "naval") drawNaval(scene, o);
    else if (o.family === "embarked") drawNaval(scene, o, true);
    else if (o.family === "air") drawAir(scene, o);
    else if (o.family === "rotor") drawRotor(scene, o);
    else if (o.family === "balloon") drawBalloon(scene, o);
    else if (o.family === "drone") drawDrone(scene, o);
    else {
      scene.shadow(o.family === "civilian" ? 7 : 8.5, 3.2);
      human(scene, {...o, civilian:o.family === "civilian"});
    }
    scene.flush();
    return true;
  }

  global.Cinematic3D = Object.freeze({
    families:FAMILIES,
    environments:ENVIRONMENTS,
    supports:family => FAMILY_SET.has(family),
    supportsEnvironment:kind => ENVIRONMENT_SET.has(kind),
    draw,
    drawEnvironment,
  });
})(globalThis);
