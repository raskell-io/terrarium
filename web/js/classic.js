// Classic renderer: CP437-style glyphs from an 8x8 bitmap font, the 16-colour VGA palette,
// people blinking in turn when they share a tile, as on the old curses screens.

export const VGA = {
  black: "#000000", blue: "#000080", green: "#008000", cyan: "#008080",
  red: "#800000", magenta: "#800080", brown: "#808000", lgray: "#c0c0c0",
  dgray: "#808080", lblue: "#5555ff", lgreen: "#55ff55", lcyan: "#55ffff",
  lred: "#ff5555", lmagenta: "#ff55ff", yellow: "#ffff55", white: "#ffffff",
};

// 8x8 glyphs, one byte per row, most significant bit on the left (IBM CGA font shapes)
const FONT = {
  "☺": [0x7e, 0x81, 0xa5, 0x81, 0xbd, 0x99, 0x81, 0x7e],
  "☻": [0x7e, 0xff, 0xdb, 0xff, 0xc3, 0xe7, 0xff, 0x7e],
  "♣": [0x38, 0x7c, 0x38, 0xfe, 0xfe, 0xd6, 0x10, 0x38],
  "♠": [0x10, 0x10, 0x38, 0x7c, 0xfe, 0x7c, 0x10, 0x38],
  "τ": [0x00, 0x76, 0xdc, 0x18, 0x18, 0x18, 0x18, 0x00],
  "≈": [0x00, 0x76, 0xdc, 0x00, 0x76, 0xdc, 0x00, 0x00],
  "~": [0x76, 0xdc, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
  ".": [0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x00],
  ",": [0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x60],
  "'": [0x30, 0x30, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00],
  "`": [0x30, 0x30, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00],
  '"': [0x6c, 0x6c, 0x6c, 0x00, 0x00, 0x00, 0x00, 0x00],
  "·": [0x00, 0x00, 0x00, 0x18, 0x18, 0x00, 0x00, 0x00],
  "▲": [0x10, 0x10, 0x38, 0x38, 0x7c, 0x7c, 0xfe, 0x00],
  "^": [0x10, 0x38, 0x6c, 0xc6, 0x00, 0x00, 0x00, 0x00],
  "∩": [0x78, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0x00],
  "⌂": [0x10, 0x38, 0x6c, 0xc6, 0xc6, 0xfe, 0x00, 0x00],
  "■": [0x00, 0x00, 0x3c, 0x3c, 0x3c, 0x3c, 0x00, 0x00],
  "#": [0x6c, 0x6c, 0xfe, 0x6c, 0xfe, 0x6c, 0x6c, 0x00],
  "░": [0x22, 0x88, 0x22, 0x88, 0x22, 0x88, 0x22, 0x88],
  "d": [0x1c, 0x0c, 0x0c, 0x7c, 0xcc, 0xcc, 0x76, 0x00],
  "Ω": [0x38, 0x6c, 0xc6, 0xc6, 0x6c, 0x6c, 0xee, 0x00],
};

// People in the bright half of the palette; land claims in the dark half
const PERSON_COLORS = ["lcyan", "lgreen", "yellow", "lmagenta", "lred", "white", "lblue"];
const LAND_COLORS = ["blue", "green", "cyan", "red", "magenta", "brown"];

export function personColor(id) {
  return VGA[PERSON_COLORS[id % PERSON_COLORS.length]];
}

const glyphCache = new Map();
function glyph(ch, color) {
  const key = ch + color;
  let canvas = glyphCache.get(key);
  if (!canvas) {
    canvas = document.createElement("canvas");
    canvas.width = canvas.height = 8;
    const c = canvas.getContext("2d");
    c.fillStyle = color;
    const rows = FONT[ch] || FONT["·"];
    rows.forEach((bits, y) => {
      for (let x = 0; x < 8; x++) if (bits & (0x80 >> x)) c.fillRect(x, y, 1, 1);
    });
    glyphCache.set(key, canvas);
  }
  return canvas;
}

function hash(i, k = 0) {
  let h = (i * 374761393 + k * 668265263) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

function dim(hex, f) {
  const n = parseInt(hex.slice(1), 16);
  const r = ((n >> 16) & 255) * f, g = ((n >> 8) & 255) * f, b = (n & 255) * f;
  return `rgb(${r | 0},${g | 0},${b | 0})`;
}

/** What the land itself looks like on one tile: [glyph, colour] */
function landGlyph(world, cell, i, phase) {
  const t = world.terrain[i];
  const [food, forest, game] = cell;
  const e = world.elevation[i];
  if (t === "W") {
    const wave = (phase + Math.floor(hash(i) * 4)) % 4;
    return [wave < 2 ? "≈" : "~", wave % 2 ? VGA.blue : VGA.lblue];
  }
  // Game shows itself now and then where herds are thick
  if (game >= 15 && (phase + Math.floor(hash(i, 3) * 11)) % 11 === 0) return ["d", VGA.brown];
  if (t === "F") {
    if (forest >= 3) return ["♣", hash(i, 1) < 0.5 ? VGA.green : VGA.lgreen];
    if (forest === 2) return ["♠", VGA.green];
    if (forest === 1) return ["τ", VGA.green];
    if (food >= 10) return ['"', VGA.lgreen];
    const grass = [",", ".", "`", "'"][Math.floor(hash(i, 2) * 4)];
    return [grass, food >= 4 ? VGA.green : VGA.brown];
  }
  if (e >= 85) return ["▲", VGA.lgray];
  if (e >= 70) return ["^", VGA.dgray];
  return [[".", ",", "·"][Math.floor(hash(i, 2) * 3)], hash(i, 4) < 0.5 ? VGA.brown : VGA.dgray];
}

function structureGlyph(s) {
  if (!s.complete) return ["░", VGA.dgray];
  switch (s.kind) {
    case "LeanTo": return ["∩", VGA.brown];
    case "Shelter": return ["⌂", VGA.yellow];
    case "Storage": return ["■", VGA.brown];
    case "Farm": return ['"', VGA.yellow];
    default: return ["#", VGA.lgray];
  }
}

/**
 * Draw the world. `view` = { tile, phase, selected, hover }; tile is the on-screen tile size
 * in pixels (a multiple of 8).
 */
export function drawClassic(ctx, world, frame, view) {
  const { tile, phase } = view;
  const W = world.width, H = world.height;
  ctx.imageSmoothingEnabled = false;
  ctx.fillStyle = VGA.black;
  ctx.fillRect(0, 0, W * tile, H * tile);

  const land = new Map(frame.land.map(([x, y, o]) => [y * W + x, o]));
  const structures = new Map(frame.structures.map((s) => [s.y * W + s.x, s]));
  const people = new Map();
  for (const p of frame.people) {
    if (!p.alive) continue;
    const k = p.y * W + p.x;
    if (!people.has(k)) people.set(k, []);
    people.get(k).push(p);
  }

  for (let i = 0; i < W * H; i++) {
    const x = (i % W) * tile, y = Math.floor(i / W) * tile;
    const owner = land.get(i);
    let bg = owner == null ? null : dim(VGA[LAND_COLORS[owner % LAND_COLORS.length]], 0.55);
    let [ch, fg] = landGlyph(world, frame.cells[i], i, phase);

    const s = structures.get(i);
    if (s) [ch, fg] = structureGlyph(s);

    const here = people.get(i);
    if (here) {
      // Several people on one tile take turns being shown
      const p = here[phase % here.length];
      ch = p.stage === "youth" ? "☻" : "☺";
      fg = p.sick && phase % 2 ? VGA.magenta : personColor(p.id);
      if (p.sheltered && s && phase % 3 === 0) [ch, fg] = structureGlyph(s);
      if (view.selected === p.id && phase % 2 === 0) { bg = fg; fg = VGA.black; }
    }
    if (bg) {
      ctx.fillStyle = bg;
      ctx.fillRect(x, y, tile, tile);
    }
    ctx.drawImage(glyph(ch, fg), x, y, tile, tile);
  }

  if (view.hover) {
    const [hx, hy] = view.hover;
    ctx.strokeStyle = VGA.yellow;
    ctx.lineWidth = Math.max(1, tile / 16);
    ctx.strokeRect(hx * tile + 0.5, hy * tile + 0.5, tile - 1, tile - 1);
  }
}
