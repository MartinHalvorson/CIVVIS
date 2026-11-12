// The software globe's ground does not move when only an army animates.
// Keep one full-resolution backdrop, never a downsampled terrain view.
// Ordinary draw() calls (publications, input and artwork loads) invalidate it;
// only animation paints may read it. Camera and canvas identity are exact.
const PLANET_BACKDROP_MAX_PIXELS = 16_777_216;
const PLANET_BACKDROP_PAINT_FIELDS = [
  "fillStyle", "strokeStyle", "font", "textAlign", "textBaseline", "direction",
  "globalAlpha", "globalCompositeOperation", "lineCap", "lineJoin", "lineWidth",
  "lineDashOffset", "miterLimit", "shadowBlur", "shadowColor", "shadowOffsetX",
  "shadowOffsetY", "filter", "imageSmoothingEnabled", "imageSmoothingQuality",
  "fontKerning", "fontStretch", "fontVariantCaps", "letterSpacing", "wordSpacing",
  "textRendering",
];
let planetBackdrop = null, planetBackdropCanvas = null;
function planetBackdropView(centerX, centerY) {
  return [cam.x, cam.y, cam.scale, cam.rot, cam.tilt, centerX, centerY,
          MAPW, MAPH, DPR, cv.width, cv.height];
}
function reusablePlanetBackdrop(view) {
  const held = planetBackdrop;
  return held && held.state === state && held.planet === PLANET &&
    view.every((value, index) => Object.is(value, held.view[index])) ? held : null;
}
function savePlanetBackdrop(view, cells) {
  const pixels = cv.width * cv.height;
  if (!(pixels > 0 && pixels <= PLANET_BACKDROP_MAX_PIXELS)) {
    planetBackdropCanvas = null;
    return;
  }
  const canvas = planetBackdropCanvas || document.createElement("canvas");
  if (canvas.width !== cv.width) canvas.width = cv.width;
  if (canvas.height !== cv.height) canvas.height = cv.height;
  planetBackdropCanvas = canvas;
  const brush = canvas.getContext("2d", {alpha:true});
  if (!brush) return;
  // Source and destination have identical physical dimensions and colour
  // spaces. Preserve alpha too: fractional pixel ratios can leave a partly
  // covered edge pixel even though the scene starts with a black field.
  brush.globalCompositeOperation = "copy";
  brush.drawImage(cv, 0, 0);
  planetBackdrop = {canvas, state, planet:PLANET, view, cells,
    hitFrame:PLANET_HIT_FRAME,
    paint:Object.fromEntries(PLANET_BACKDROP_PAINT_FIELDS
      .filter(field => field in cx).map(field => [field, cx[field]])),
    dash:cx.getLineDash(), hits:0};
}
function paintPlanetBackdrop(held, centerY) {
  cx.setTransform(1, 0, 0, 1, 0, 0);
  cx.save();
  cx.globalAlpha = 1; cx.globalCompositeOperation = "copy";
  cx.shadowBlur = 0; cx.shadowOffsetX = 0; cx.shadowOffsetY = 0;
  cx.filter = "none";
  cx.drawImage(held.canvas, 0, 0);
  cx.restore();
  // Match the save/ground-tilt boundary of the ordinary background path.
  // Cities, units, live effects and the sky continue through that same path.
  cx.setTransform(DPR, 0, 0, DPR, 0, 0);
  cx.save();
  applyPlanetGroundTilt(centerY);
  // Leave matching Canvas attributes untouched. Reassigning their existing
  // values changed close-up foreground pixels in the native Chrome fixture,
  // even though those values read back unchanged.
  for (const [field, value] of Object.entries(held.paint)) {
    if (cx[field] !== value) cx[field] = value;
  }
  cx.setLineDash(held.dash);
  PLANET_HIT_FRAME = {...held.hitFrame};
  held.hits++;
}

