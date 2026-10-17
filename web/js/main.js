// Terrarium in the browser: boots the WebAssembly engine, runs the day loop, and draws the
// world in either look. All simulation state lives in the engine; this file only reads frames.

import init, { Simulation, scenarios } from "../pkg/terrarium_web.js";
import { drawClassic, personColor as classicColor } from "./classic.js";
import { drawPremium, personColor as premiumColor, resetCaches } from "./premium.js";

const $ = (id) => document.getElementById(id);

const LOOKS = {
  classic: { draw: drawClassic, base: 8, color: classicColor },
  premium: { draw: drawPremium, base: 16, color: premiumColor },
};

// Announcement categories, from the engine's event kinds
const KIND = {
  Death: "death", Birth: "birth", Conception: "birth",
  Attack: "conflict", AllyIntervened: "conflict", TerritoryFight: "conflict", RivalryFormed: "conflict",
  TerritoryMarked: "land", TerritoryChallenged: "land", TerritorySubmitted: "land", TerritoryLost: "land",
  StructureCompleted: "build", StructureDestroyed: "build", Crafting: "build", ToolBroke: "build", FarmProduced: "build",
  Gift: "share", Hunting: "share", Fishing: "share",
  Courtship: "social", Gossip: "social", GroupFormed: "social", GroupDissolved: "social", SkillTaught: "social",
  Illness: "illness", Weather: "weather",
};
// Everyday doings that would drown out the news
const QUIET = new Set(["Movement", "Gathering", "Eating", "Resting", "MaterialGathering", "Chopping", "Meta", "Courtship"]);

const state = {
  sim: null,
  world: null,
  frame: null,
  scenario: "first_winter",
  look: "classic",
  zoom: 2,
  speed: 4, // days per second
  paused: false,
  selected: null,
  hover: null,
  phase: 0,
  log: [],
};

function store(key, value) {
  try { localStorage.setItem("terrarium." + key, value); } catch { /* private mode: fine */ }
}
function recall(key) {
  try { return localStorage.getItem("terrarium." + key); } catch { return null; }
}

// ---------------- Title screen ----------------

function setLook(look) {
  state.look = look;
  document.body.dataset.look = look;
  document.querySelectorAll(".look-option").forEach((b) => b.setAttribute("aria-checked", String(b.dataset.look === look)));
  store("look", look);
  if (state.world) {
    resetCaches();
    fitZoom();
    draw();
    renderSide();
  }
}

function renderScenarios(list) {
  const ul = $("scenarios");
  ul.innerHTML = "";
  for (const s of list) {
    const li = document.createElement("li");
    li.className = "scenario";
    li.setAttribute("role", "option");
    li.tabIndex = 0;
    li.dataset.id = s.id;
    li.setAttribute("aria-selected", String(s.id === state.scenario));
    li.innerHTML = `<span class="name"></span><span class="meta"></span><span class="desc"></span>`;
    li.querySelector(".name").textContent = s.name;
    li.querySelector(".meta").textContent = `${s.agents} people · ${s.width}×${s.height}`;
    li.querySelector(".desc").textContent = s.description.replace(/\s+/g, " ");
    const pick = () => {
      state.scenario = s.id;
      store("scenario", s.id);
      ul.querySelectorAll(".scenario").forEach((n) => n.setAttribute("aria-selected", String(n.dataset.id === s.id)));
    };
    li.addEventListener("click", pick);
    li.addEventListener("dblclick", embark);
    li.addEventListener("keydown", (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); pick(); } });
    ul.appendChild(li);
  }
}

// ---------------- Game ----------------

function embark() {
  try {
    state.sim?.free();
    state.sim = new Simulation(state.scenario);
  } catch (e) {
    $("boot-status").textContent = `This world could not be created: ${e.message ?? e}`;
    $("boot-status").classList.add("error");
    return;
  }
  state.world = JSON.parse(state.sim.world());
  state.frame = JSON.parse(state.sim.frame());
  state.selected = null;
  state.log = [];
  state.paused = false;
  resetCaches();
  $("title").hidden = true;
  $("game").hidden = false;
  $("world-name").textContent = state.world.name;
  addLog(0, "other", `${state.frame.people.length} people arrive in ${state.world.name}. It is ${state.frame.season.toLowerCase()}.`);
  fitZoom();
  renderAll();
  updateSpeedButtons();
}

function toTitle() {
  state.paused = true;
  $("game").hidden = true;
  $("title").hidden = false;
}

function stepDay() {
  if (!state.sim || !state.sim.populated()) return;
  state.sim.step();
  state.frame = JSON.parse(state.sim.frame());
  for (const e of state.frame.events) {
    if (QUIET.has(e.kind)) continue;
    addLog(state.frame.day, KIND[e.kind] || "other", e.text);
  }
  if (!state.sim.populated()) {
    addLog(state.frame.day, "death", "No one is left. The world goes on without them.");
    state.paused = true;
    updateSpeedButtons();
  }
  renderAll();
}

function addLog(day, kind, text) {
  state.log.push({ day, kind, text });
  if (state.log.length > 400) state.log.splice(0, state.log.length - 400);
}

// ---------------- Rendering ----------------

const canvas = $("map");
const ctx = canvas.getContext("2d");

function tileSize() {
  return LOOKS[state.look].base * state.zoom;
}

function fitZoom() {
  const box = $("map-scroll").getBoundingClientRect();
  const base = LOOKS[state.look].base;
  // Stacked (narrow) layouts size the map pane to its content, so fit to the screen instead
  const stacked = matchMedia("(max-width: 820px)").matches;
  const height = stacked ? window.innerHeight * 0.6 : box.height;
  const fit = Math.floor(Math.min(box.width / (state.world.width * base), height / (state.world.height * base)));
  state.zoom = Math.max(1, Math.min(6, fit || 1));
  sizeCanvas();
}

function sizeCanvas() {
  const t = tileSize();
  canvas.width = state.world.width * t;
  canvas.height = state.world.height * t;
}

function draw() {
  if (!state.world) return;
  LOOKS[state.look].draw(ctx, state.world, state.frame, {
    tile: tileSize(),
    phase: state.phase,
    selected: state.selected,
    hover: state.hover,
  });
}

function renderAll() {
  draw();
  renderTop();
  renderSide();
  renderLog();
}

function renderTop() {
  const f = state.frame;
  $("date").textContent = `${f.season}, day ${f.dayOfYear} · year ${f.year}`;
  const w = $("weather");
  w.className = "weather " + f.weather;
  w.textContent = f.weather === "fair" ? "" : f.weather === "storm" ? "Storm" : "Drought";
}

function barRow(label, value, cls = "") {
  const pct = Math.round(Math.max(0, Math.min(1, value)) * 100);
  return `<span>${label}</span><div class="bar ${cls}"><i style="width:${pct}%"></i></div><span>${pct}%</span>`;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

function renderSide() {
  const f = state.frame;
  const look = LOOKS[state.look];
  const alive = f.people.filter((p) => p.alive);
  $("pop-count").textContent = `(${alive.length})`;

  // People list: the living first, then the dead
  const ul = $("units");
  const sorted = [...f.people].sort((a, b) => (b.alive - a.alive) || a.name.localeCompare(b.name));
  ul.innerHTML = sorted.map((p) => {
    const glyph = state.look === "classic"
      ? `<span class="glyph" style="color:${look.color(p.id)}">${p.stage === "youth" ? "☻" : "☺"}</span>`
      : `<span class="glyph" style="background:${look.color(p.id)}"></span>`;
    const doing = !p.alive ? "dead" : p.sick ? "sick" : p.doing || (p.sheltered ? "sheltering" : "idle");
    return `<li data-id="${p.id}" class="${p.alive ? "" : "dead"} ${state.selected === p.id ? "sel" : ""}">${glyph}<span class="who">${escapeHtml(p.name)}</span><span class="doing">${escapeHtml(doing)}</span></li>`;
  }).join("");

  // Selected person
  const box = $("selected");
  const p = f.people.find((q) => q.id === state.selected);
  if (!p) {
    box.innerHTML = `<span class="meta">Click someone on the map or in the list to follow them.</span>`;
    return;
  }
  const notes = [
    p.alive ? null : "has died",
    p.sick ? "sick with a fever" : null,
    p.sheltered ? "in shelter" : null,
    p.expecting ? "expecting a child" : null,
    p.generation > 0 ? `born here (generation ${p.generation})` : "one of the founders",
  ].filter(Boolean).join(" · ");
  box.innerHTML = `
    <h3 style="color:${look.color(p.id)}">${escapeHtml(p.name)}</h3>
    <div class="meta">${p.stage}, ${p.age} days · at (${p.x}, ${p.y}) · carrying ${p.food} food</div>
    <div class="bars">
      ${barRow("health", p.health)}
      ${barRow("hunger", p.hunger, "hunger")}
      ${barRow("energy", p.energy, "energy")}
    </div>
    <div>${escapeHtml(p.doing ? `Today: ${p.doing}.` : "")} ${escapeHtml(notes)}</div>
    <div class="meta">${escapeHtml(p.personality)}. Wants ${escapeHtml(p.aspiration)}.</div>`;
}

function renderLog() {
  const ol = $("log");
  ol.innerHTML = state.log.slice(-150).reverse()
    .map((e) => `<li class="k-${e.kind}"><span class="day">${e.day}</span>${escapeHtml(e.text)}</li>`)
    .join("");
}

function describeCell(x, y) {
  const w = state.world, f = state.frame, i = y * w.width + x;
  const [food, forest, game, fish] = f.cells[i];
  const t = w.terrain[i];
  const e = w.elevation[i];
  const parts = [];
  if (t === "W") parts.push(fish >= 8 ? "A lake with plenty of fish" : fish >= 1 ? "A lake with a few fish" : "A fished-out lake");
  else {
    const ground = t === "F" ? (forest >= 3 ? "Thick forest" : forest === 2 ? "Woodland" : forest === 1 ? "Brush" : "Meadow") : e >= 85 ? "Rocky high ground" : e >= 70 ? "Upland" : "Barren ground";
    parts.push(ground);
    parts.push(food >= 11 ? "plenty of food" : food >= 6 ? "some food" : food > 0 ? "a little food" : "no food");
    if (game >= 15) parts.push("tracks of game");
  }
  const s = f.structures.find((q) => q.x === x && q.y === y);
  if (s) parts.push(`${s.complete ? "a" : "an unfinished"} ${s.name}${s.owner != null ? ` (${f.people[s.owner]?.name})` : ""}`);
  const claim = f.land.find(([lx, ly]) => lx === x && ly === y);
  if (claim) parts.push(`claimed by ${f.people[claim[2]]?.name}`);
  const here = f.people.filter((p) => p.alive && p.x === x && p.y === y).map((p) => p.name + (p.doing ? ` (${p.doing})` : ""));
  return `(${x}, ${y}) ${parts.join(", ")}.${here.length ? " Here: " + here.join(", ") + "." : ""}`;
}

function cellFromEvent(e) {
  const r = canvas.getBoundingClientRect();
  const t = r.width / state.world.width;
  const x = Math.floor((e.clientX - r.left) / t), y = Math.floor((e.clientY - r.top) / t);
  return x >= 0 && y >= 0 && x < state.world.width && y < state.world.height ? [x, y] : null;
}

// ---------------- Controls ----------------

function updateSpeedButtons() {
  $("pause").setAttribute("aria-pressed", String(state.paused));
  document.querySelectorAll(".speed-btn").forEach((b) => b.setAttribute("aria-pressed", String(!state.paused && +b.dataset.speed === state.speed)));
}
function setSpeed(speed) {
  state.speed = speed;
  state.paused = false;
  updateSpeedButtons();
}
function togglePause() {
  state.paused = !state.paused;
  updateSpeedButtons();
}
function zoom(delta) {
  state.zoom = Math.max(1, Math.min(8, state.zoom + delta));
  sizeCanvas();
  draw();
}

function wire() {
  document.querySelectorAll(".look-option").forEach((b) => b.addEventListener("click", () => setLook(b.dataset.look)));
  $("embark").addEventListener("click", embark);
  $("to-title").addEventListener("click", toTitle);
  $("pause").addEventListener("click", togglePause);
  $("step").addEventListener("click", () => { state.paused = true; updateSpeedButtons(); stepDay(); });
  document.querySelectorAll(".speed-btn").forEach((b) => b.addEventListener("click", () => setSpeed(+b.dataset.speed)));
  $("zoom-in").addEventListener("click", () => zoom(1));
  $("zoom-out").addEventListener("click", () => zoom(-1));
  $("look-toggle").addEventListener("click", () => setLook(state.look === "classic" ? "premium" : "classic"));

  canvas.addEventListener("mousemove", (e) => {
    const c = cellFromEvent(e);
    if (!c) return;
    if (!state.hover || c[0] !== state.hover[0] || c[1] !== state.hover[1]) {
      state.hover = c;
      $("hover-info").textContent = describeCell(...c);
      draw();
    }
  });
  canvas.addEventListener("mouseleave", () => { state.hover = null; draw(); });
  canvas.addEventListener("click", (e) => {
    const c = cellFromEvent(e);
    if (!c) return;
    const here = state.frame.people.filter((p) => p.alive && p.x === c[0] && p.y === c[1]);
    if (here.length) {
      const k = here.findIndex((p) => p.id === state.selected);
      state.selected = here[(k + 1) % here.length].id;
    } else {
      state.selected = null;
    }
    renderSide();
    draw();
  });
  $("units").addEventListener("click", (e) => {
    const li = e.target.closest("li[data-id]");
    if (!li) return;
    state.selected = +li.dataset.id;
    renderSide();
    draw();
  });

  document.addEventListener("keydown", (e) => {
    if (e.target.closest("input, select, textarea")) return;
    if (!$("title").hidden) {
      if (e.key === "e" || (e.key === "Enter" && e.target === document.body)) embark();
      if (e.key === "l" || e.key === "L") setLook(state.look === "classic" ? "premium" : "classic");
      return;
    }
    if (e.key === " ") { e.preventDefault(); togglePause(); }
    else if (e.key === ".") { state.paused = true; updateSpeedButtons(); stepDay(); }
    else if (e.key === "1") setSpeed(1);
    else if (e.key === "2") setSpeed(4);
    else if (e.key === "3") setSpeed(15);
    else if (e.key === "l" || e.key === "L") setLook(state.look === "classic" ? "premium" : "classic");
    else if (e.key === "+" || e.key === "=") zoom(1);
    else if (e.key === "-" || e.key === "_") zoom(-1);
    else if (e.key === "Escape") { state.selected = null; renderSide(); draw(); }
  });
  window.addEventListener("resize", () => { if (state.world && !$("game").hidden) { fitZoom(); draw(); } });
}

// ---------------- Loop ----------------

let last = performance.now();
let owed = 0;
let lastPhase = 0;
function loop(now) {
  const dt = Math.min(1000, now - last);
  last = now;
  if (state.sim && !$("game").hidden) {
    if (!state.paused) {
      owed += (dt / 1000) * state.speed;
      let steps = 0;
      while (owed >= 1 && steps < 5) {
        stepDay();
        owed -= 1;
        steps++;
      }
      if (steps === 5) owed = 0;
    }
    // Blinking and water move a few times a second, whatever the speed
    if (now - lastPhase > 350) {
      state.phase = (state.phase + 1) % 1000;
      lastPhase = now;
      draw();
    }
  }
  requestAnimationFrame(loop);
}

// ---------------- Boot ----------------

async function boot() {
  setLook(recall("look") === "premium" ? "premium" : "classic");
  state.scenario = recall("scenario") || state.scenario;
  wire();
  $("embark").disabled = true;
  try {
    await init();
    const list = JSON.parse(scenarios());
    if (!list.some((s) => s.id === state.scenario)) state.scenario = list[0].id;
    renderScenarios(list);
    $("embark").disabled = false;
    $("boot-status").textContent = "Ready. Pick a world and embark.";
  } catch (e) {
    $("boot-status").textContent = `The world engine failed to load: ${e.message ?? e}`;
    $("boot-status").classList.add("error");
    return;
  }
  requestAnimationFrame(loop);
}

boot();
