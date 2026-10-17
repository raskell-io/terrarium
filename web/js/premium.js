// Premium renderer: 16x16 pixel-art tiles. Ground is generated per tile from a seeded noise
// (so every meadow differs), objects are hand-drawn sprites given a dark outline automatically,
// and tunics and banners are tinted per person.

const PALETTE = {
  // trees and plants
  q: "#24451f", r: "#346a2c", s: "#5c9a40",
  t: "#5a3b22",
  u: "#8c1f22", v: "#d8443a",
  // stone
  i: "#6c6962", j: "#8d897f", k: "#4c4a45", l: "#b5b0a3",
  // animals
  D: "#7a4e2c", E: "#a6713f", F: "#efe1c2",
  // people
  w: "#e2b387", x: "#4a2f1d", K: "#1a1310", h: "#3a2a1e",
  // timber and thatch
  W: "#8c5a30", X: "#5e3a1e", Y: "#b27d46", Z: "#c9a55a",
};
const OUTLINE = "#140e0a";

const SPRITES = {
  tree: [
    "................",
    "......rrrr......",
    "....rrsssrrr....",
    "...rrssssrrrr...",
    "..rrsssrrrrrrq..",
    "..rrssrrrrrrqq..",
    ".rrrrrrrrrrqqqq.",
    ".qrrrrrrrrqqqqq.",
    ".qqrrrrrrqqqqqq.",
    "..qqqrrrqqqqqq..",
    "...qqqqqqqqqq...",
    ".....qqqqqq.....",
    ".......tt.......",
    ".......tt.......",
    "......tttt......",
    "................",
  ],
  pine: [
    "................",
    ".......q........",
    "......qrq.......",
    ".....qrsrq......",
    "......qrq.......",
    ".....qrsrq......",
    "....qrrsrrq.....",
    ".....qrrrq......",
    "....qrrsrrq.....",
    "...qrrrsrrrq....",
    "..qqrrrrrrrqq...",
    "....qqqqqqq.....",
    ".......t........",
    ".......t........",
    "................",
    "................",
  ],
  bush: [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "......rrr.......",
    "....rrssrrq.....",
    "...rrsrrrrqq....",
    "...qrrrrrqqq....",
    "....qqqqqqq.....",
    "................",
    "................",
    "................",
  ],
  rock: [
    "................",
    "................",
    "................",
    "................",
    "......kjj.......",
    ".....kjjlj......",
    "....kjjjljj.....",
    "...kkjjjjjjj....",
    "..kkjjjjijjjj...",
    "..kjjjjiijjjjj..",
    ".kkjjjiijjjjjj..",
    ".kkkjjjjjjjiij..",
    ".kkkkkkkkkkkkk..",
    "................",
    "................",
    "................",
  ],
  boulder: [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "..........kjj...",
    ".........kjjlj..",
    ".........kjjjj..",
    "..........kkk...",
    "................",
    "................",
    "................",
  ],
  deer: [
    "................",
    "................",
    "................",
    "..E..E..........",
    "...EE...........",
    "..DDDD..........",
    "..DDD...........",
    "...DDDDDDDDDF...",
    "...DDDDDDDDDD...",
    "....DEEEEEDD....",
    "....D.D...D.D...",
    "....D.D...D.D...",
    "....h.h...h.h...",
    "................",
    "................",
    "................",
  ],
  person: [
    "................",
    "................",
    ".......xxx......",
    "......xxxxx.....",
    "......wwwww.....",
    "......wKwKw.....",
    "......wwwww.....",
    ".......www......",
    ".....yyyyyyy....",
    "....wyyyyyyyw...",
    "....wyyyyyyyw...",
    "....w.zzzzz.w...",
    "......zz.zz.....",
    "......zz.zz.....",
    "......hh.hh.....",
    "................",
  ],
  child: [
    "................",
    "................",
    "................",
    "................",
    "................",
    ".......xxx......",
    "......wwwww.....",
    "......wKwKw.....",
    ".......www......",
    "......yyyyy.....",
    ".....wyyyyyw....",
    "......zzzzz.....",
    "......z...z.....",
    "......h...h.....",
    "................",
    "................",
  ],
  leanto: [
    "................",
    "................",
    "................",
    "...........X....",
    "..........XY....",
    ".........XYZ....",
    "........XYZZ....",
    ".......XYZZZ....",
    "......XYZZZZ....",
    ".....XYZZZZZ....",
    "....XYZZZZZZ....",
    "...XYZZZZZZZ....",
    "..XXXXXXXXXXX...",
    "................",
    "................",
    "................",
  ],
  shelter: [
    "................",
    "................",
    ".......ZZ.......",
    "......ZZZZ......",
    ".....ZZYZZZ.....",
    "....ZZZZZYZZ....",
    "...ZZYZZZZZZZ...",
    "..ZZZZZZZYZZZZ..",
    "..XXXXXXXXXXXX..",
    "...WWWWWWWWWW...",
    "...WWWKKKWWWW...",
    "...WYWKKKWYWW...",
    "...WWWKKKWWWW...",
    "...XXXKKKXXXX...",
    "................",
    "................",
  ],
  storage: [
    "................",
    "................",
    "................",
    "...XXXXXXXXXX...",
    "..XYYYYYYYYYYX..",
    "..XWWWWWWWWWWX..",
    "..XWXWWXWWXWWX..",
    "..XWWWWWWWWWWX..",
    "..XWXWWXWWXWWX..",
    "..XWWWWWWWWWWX..",
    "..XXXXXXXXXXXX..",
    "...X........X...",
    "................",
    "................",
    "................",
    "................",
  ],
  scaffold: [
    "................",
    "................",
    "................",
    "...X.....X......",
    "...XXXXXXXX.....",
    "...X..X..X......",
    "...X..X..X......",
    "...XXXXXXXX.....",
    "...X..X..X......",
    "...X..X..X......",
    "...X..X..X......",
    "..XXXXXXXXXX....",
    "................",
    "................",
    "................",
    "................",
  ],
  flag: [
    ".Xyyy...........",
    ".Xyyyy..........",
    ".Xyy............",
    ".X..............",
    ".X..............",
    "XXX.............",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
  ],
};

function hexToRgb(hex) {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
function shade(hex, f) {
  return "#" + hexToRgb(hex).map((v) => Math.max(0, Math.min(255, Math.round(v * f))).toString(16).padStart(2, "0")).join("");
}

export function personColor(id) {
  // Even hue spacing (golden angle) keeps neighbours apart; HSL → hex for tinting
  const h = (id * 137.508) % 360, s = 0.62, l = 0.5;
  const f = (n) => {
    const k = (n + h / 30) % 12;
    const c = l - s * Math.min(l, 1 - l) * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(c * 255).toString(16).padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

const spriteCache = new Map();
/** A sprite rendered once (with outline), optionally tinted: 'y' and 'z' take the tint */
function sprite(name, tint) {
  const key = name + (tint || "");
  let canvas = spriteCache.get(key);
  if (canvas) return canvas;
  const rows = SPRITES[name];
  canvas = document.createElement("canvas");
  canvas.width = canvas.height = 16;
  const c = canvas.getContext("2d");
  const colorOf = (ch) => (ch === "y" ? tint : ch === "z" ? shade(tint, 0.7) : PALETTE[ch]);
  const solid = (x, y) => y >= 0 && y < 16 && x >= 0 && x < 16 && rows[y][x] !== ".";
  for (let y = 0; y < 16; y++) {
    for (let x = 0; x < 16; x++) {
      const ch = rows[y][x];
      if (ch !== ".") {
        c.fillStyle = colorOf(ch);
        c.fillRect(x, y, 1, 1);
      } else if (solid(x - 1, y) || solid(x + 1, y) || solid(x, y - 1) || solid(x, y + 1)) {
        c.fillStyle = OUTLINE;
        c.fillRect(x, y, 1, 1);
      }
    }
  }
  spriteCache.set(key, canvas);
  return canvas;
}

function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// Ground colours by terrain, from low to high ground
const GROUND = {
  F: { low: "#4f7d34", high: "#7b8a43", dark: "#3c6428", light: "#76a64c", tuft: "#2f5222" },
  B: { low: "#9a8458", high: "#87827a", dark: "#7a6745", light: "#b29d70", tuft: "#6a5a3c" },
};

const groundCache = new Map();
function ground(world, i) {
  let canvas = groundCache.get(i);
  if (canvas) return canvas;
  canvas = document.createElement("canvas");
  canvas.width = canvas.height = 16;
  const c = canvas.getContext("2d");
  const t = world.terrain[i];
  const e = world.elevation[i] / 99;
  const g = GROUND[t];
  const rand = rng(i * 7919 + 17);
  const base = hexToRgb(g.low).map((v, k) => Math.round(v + (hexToRgb(g.high)[k] - v) * e));
  c.fillStyle = `rgb(${base.join(",")})`;
  c.fillRect(0, 0, 16, 16);
  for (let k = 0; k < 40; k++) {
    c.fillStyle = rand() < 0.5 ? g.dark : g.light;
    c.fillRect(Math.floor(rand() * 16), Math.floor(rand() * 16), 1, 1);
  }
  if (t === "F") {
    for (let k = 0; k < 5; k++) {
      const x = Math.floor(rand() * 15), y = 2 + Math.floor(rand() * 13);
      c.fillStyle = g.tuft;
      c.fillRect(x, y, 1, 2);
      c.fillRect(x + 1, y - 1, 1, 2);
    }
  } else {
    for (let k = 0; k < 4; k++) {
      c.fillStyle = shade(g.low, 0.75);
      c.fillRect(Math.floor(rand() * 15), Math.floor(rand() * 15), 2, 1);
    }
  }
  // A shadow along the edge where the land falls away to the south
  const below = i + world.width;
  if (below < world.terrain.length && world.terrain[below] !== "W" && world.elevation[i] - world.elevation[below] > 15) {
    c.fillStyle = "rgba(0,0,0,0.22)";
    c.fillRect(0, 14, 16, 2);
  }
  groundCache.set(i, canvas);
  return canvas;
}

export function resetCaches() {
  groundCache.clear();
}

function drawWater(ctx, x, y, tile, i, fish, phase) {
  const px = tile / 16;
  ctx.fillStyle = fish >= 8 ? "#3a6f9e" : "#356590";
  ctx.fillRect(x, y, tile, tile);
  for (let row = 0; row < 16; row += 4) {
    for (let col = 0; col < 16; col++) {
      const v = Math.sin((col + phase * 1.5 + (i % 7) * 3) / 2.3 + row * 1.7);
      if (v > 0.75) {
        ctx.fillStyle = "#5c93c4";
        ctx.fillRect(x + col * px, y + (row + 1) * px, px, px);
      } else if (v < -0.92) {
        ctx.fillStyle = "#a9d2ee";
        ctx.fillRect(x + col * px, y + (row + 2) * px, px, px);
      }
    }
  }
}

function hash(i, k = 0) {
  let h = (i * 374761393 + k * 668265263) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Draw the world; `view` = { tile, phase, selected, hover }, tile a multiple of 16 */
export function drawPremium(ctx, world, frame, view) {
  const { tile, phase } = view;
  const W = world.width, H = world.height;
  const px = tile / 16;
  ctx.imageSmoothingEnabled = false;

  const land = new Map(frame.land.map(([x, y, o]) => [y * W + x, o]));
  const structures = new Map(frame.structures.map((s) => [s.y * W + s.x, s]));

  // Ground and water
  for (let i = 0; i < W * H; i++) {
    const x = (i % W) * tile, y = Math.floor(i / W) * tile;
    if (world.terrain[i] === "W") drawWater(ctx, x, y, tile, i, frame.cells[i][3], phase);
    else ctx.drawImage(ground(world, i), x, y, tile, tile);
  }

  // Claimed land: a wash of the owner's colour, edged where the claim ends
  for (const [k, owner] of land) {
    const x = (k % W) * tile, y = Math.floor(k / W) * tile;
    const color = personColor(owner);
    ctx.fillStyle = color + "2a";
    ctx.fillRect(x, y, tile, tile);
    ctx.fillStyle = color;
    const edge = (dx, dy) => land.get(k + dy * W + dx) !== owner || (dx && Math.floor((k + dx) / W) !== Math.floor(k / W));
    if (edge(0, -1)) ctx.fillRect(x, y, tile, px);
    if (edge(0, 1)) ctx.fillRect(x, y + tile - px, tile, px);
    if (edge(-1, 0)) ctx.fillRect(x, y, px, tile);
    if (edge(1, 0)) ctx.fillRect(x + tile - px, y, px, tile);
  }

  // Things on the land, drawn top to bottom so nearer objects overlap farther ones
  for (let i = 0; i < W * H; i++) {
    if (world.terrain[i] === "W") continue;
    const x = (i % W) * tile, y = Math.floor(i / W) * tile;
    const [food, forest, game] = frame.cells[i];
    const e = world.elevation[i];
    const s = structures.get(i);
    if (world.terrain[i] === "B" && !s) {
      if (e >= 85) ctx.drawImage(sprite("rock"), x, y, tile, tile);
      else if (e >= 70 && hash(i) < 0.6) ctx.drawImage(sprite("boulder"), x, y, tile, tile);
    }
    if (!s) {
      if (forest >= 3) ctx.drawImage(sprite(hash(i, 1) < 0.5 ? "tree" : "pine"), x, y, tile, tile);
      else if (forest === 2) ctx.drawImage(sprite("pine"), x, y, tile, tile);
      else if (forest === 1) ctx.drawImage(sprite("bush"), x, y, tile, tile);
    }
    // Berries: one cluster for every few units of food
    const clusters = Math.min(4, Math.floor(food / 5));
    for (let k = 0; k < clusters; k++) {
      const bx = x + (2 + 3 * k) * px, by = y + (13 - (k % 2)) * px;
      ctx.fillStyle = PALETTE.u;
      ctx.fillRect(bx, by, 2 * px, 2 * px);
      ctx.fillStyle = PALETTE.v;
      ctx.fillRect(bx, by, px, px);
    }
    if (game >= 15 && !s && hash(i, 5) < 0.5) {
      ctx.save();
      const flip = (phase + Math.floor(hash(i, 6) * 8)) % 16 < 8;
      if (flip) {
        ctx.translate(x + tile, y);
        ctx.scale(-1, 1);
        ctx.drawImage(sprite("deer"), 0, 0, tile, tile);
      } else {
        ctx.drawImage(sprite("deer"), x, y, tile, tile);
      }
      ctx.restore();
    }
    if (s) {
      const name = !s.complete ? "scaffold" : s.kind === "LeanTo" ? "leanto" : s.kind === "Shelter" ? "shelter" : s.kind === "Storage" ? "storage" : "scaffold";
      ctx.drawImage(sprite(name), x, y, tile, tile);
    }
    const owner = land.get(i);
    if (owner != null && !s) ctx.drawImage(sprite("flag", personColor(owner)), x, y, tile, tile);
  }

  // People: up to four per tile, set around its centre
  const people = new Map();
  for (const p of frame.people) {
    if (!p.alive) continue;
    const k = p.y * W + p.x;
    if (!people.has(k)) people.set(k, []);
    people.get(k).push(p);
  }
  const spots = [[0, 0], [-4, -2], [4, -2], [0, 3]];
  for (const [k, group] of people) {
    const x = (k % W) * tile, y = Math.floor(k / W) * tile;
    group.slice(0, 4).forEach((p, j) => {
      const [ox, oy] = group.length > 1 ? spots[j] : [0, 0];
      const bob = view.selected === p.id && phase % 2 ? -px : 0;
      const sx = x + ox * px, sy = y + oy * px + bob;
      const scale = group.length > 1 ? 0.8 : 1;
      const size = tile * scale;
      const off = (tile - size) / 2;
      if (view.selected === p.id) {
        ctx.fillStyle = "rgba(240, 207, 134, 0.85)";
        ctx.beginPath();
        ctx.ellipse(sx + tile / 2, sy + tile - 2 * px, 5 * px, 2 * px, 0, 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = p.sheltered ? 0.75 : 1;
      ctx.drawImage(sprite(p.stage === "youth" ? "child" : "person", personColor(p.id)), sx + off, sy + off, size, size);
      ctx.globalAlpha = 1;
      if (p.sick) {
        ctx.fillStyle = "#8fd14f";
        ctx.fillRect(sx + 11 * px, sy + 1 * px, 3 * px, 3 * px);
        ctx.fillStyle = "#2c4a12";
        ctx.fillRect(sx + 12 * px, sy + 2 * px, px, px);
      }
    });
    if (group.length > 4) {
      ctx.fillStyle = "#f0cf86";
      ctx.font = `${Math.max(8, 5 * px)}px "Pixelify Sans", sans-serif`;
      ctx.fillText(`+${group.length - 4}`, x + 10 * px, y + 15 * px);
    }
  }

  // Weather over everything
  if (frame.weather === "storm") {
    ctx.fillStyle = "rgba(20, 30, 50, 0.28)";
    ctx.fillRect(0, 0, W * tile, H * tile);
    ctx.fillStyle = "rgba(190, 215, 240, 0.55)";
    for (let k = 0; k < W * H * 0.8; k++) {
      const rx = hash(k, phase) * W * tile, ry = hash(k + 99, phase) * H * tile;
      ctx.fillRect(rx, ry, px, 4 * px);
    }
  } else if (frame.weather === "drought") {
    ctx.fillStyle = "rgba(230, 170, 60, 0.13)";
    ctx.fillRect(0, 0, W * tile, H * tile);
  }
  if (frame.season === "Winter") {
    ctx.fillStyle = "rgba(220, 235, 255, 0.10)";
    ctx.fillRect(0, 0, W * tile, H * tile);
  }

  if (view.hover) {
    const [hx, hy] = view.hover;
    ctx.strokeStyle = "#f0cf86";
    ctx.lineWidth = px;
    ctx.strokeRect(hx * tile + px / 2, hy * tile + px / 2, tile - px, tile - px);
  }
}
