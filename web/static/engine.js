/** Browser port of gol_rs (Grid, Rule, CaRule, patterns, RLE, FNV history). */

export const DEAD = 0;
export const LIVE = 1;
export const REFRACTORY = 2;

export class Rule {
  constructor(birth, survive) {
    this.birth = birth;
    this.survive = survive;
  }

  static CONWAY = new Rule(1 << 3, (1 << 2) | (1 << 3));
  static HIGHLIFE = new Rule((1 << 3) | (1 << 6), (1 << 2) | (1 << 3));
  static SEEDS = new Rule(1 << 2, 0);
  static DAYNIGHT = new Rule(
    (1 << 3) | (1 << 6) | (1 << 7) | (1 << 8),
    (1 << 3) | (1 << 4) | (1 << 6) | (1 << 7) | (1 << 8),
  );
  static LIFE_WITHOUT_DEATH = new Rule(1 << 3, 0b1_1111_1111);
  static MAZE = new Rule(1 << 3, (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5));
  static REPLICATOR = new Rule(
    (1 << 1) | (1 << 3) | (1 << 5) | (1 << 7),
    (1 << 1) | (1 << 3) | (1 << 5) | (1 << 7),
  );

  births(n) {
    return n <= 8 && (this.birth & (1 << n)) !== 0;
  }
  survives(n) {
    return n <= 8 && (this.survive & (1 << n)) !== 0;
  }
  nextAlive(alive, n) {
    return alive ? this.survives(n) : this.births(n);
  }
  toBs() {
    return `B${maskDigits(this.birth)}/S${maskDigits(this.survive)}`;
  }
  displayName() {
    const named = [
      [Rule.CONWAY, "conway (B3/S23)"],
      [Rule.HIGHLIFE, "highlife (B36/S23)"],
      [Rule.SEEDS, "seeds (B2/S)"],
      [Rule.DAYNIGHT, "daynight (B3678/S34678)"],
      [Rule.LIFE_WITHOUT_DEATH, "life-without-death (B3/S012345678)"],
      [Rule.MAZE, "maze (B3/S12345)"],
      [Rule.REPLICATOR, "replicator (B1357/S1357)"],
    ];
    for (const [rule, label] of named) {
      if (rule.birth === this.birth && rule.survive === this.survive) return label;
    }
    return this.toBs();
  }
}

export class CaRule {
  constructor(kind, life) {
    this.kind = kind;
    this.life = life || null;
  }
  static life(rule) {
    return new CaRule("life", rule);
  }
  static brian() {
    return new CaRule("brian", null);
  }
  isMultistate() {
    return this.kind === "brian";
  }
  states() {
    return this.kind === "brian" ? 3 : 2;
  }
  toBs() {
    return this.kind === "brian" ? "BB" : this.life.toBs();
  }
  displayName() {
    return this.kind === "brian" ? "brian (Brian's Brain)" : this.life.displayName();
  }
}

export const RULE_PRESETS = [
  { id: "conway", label: "Conway B3/S23" },
  { id: "highlife", label: "HighLife B36/S23" },
  { id: "seeds", label: "Seeds B2/S" },
  { id: "daynight", label: "Day & Night" },
  { id: "lwd", label: "Life without Death" },
  { id: "maze", label: "Maze" },
  { id: "replicator", label: "Replicator" },
  { id: "brian", label: "Brian's Brain" },
];

export function parseCaRule(s) {
  const t = String(s || "").trim().toLowerCase();
  if (["brian", "brains", "brians-brain", "brian's-brain", "briansbrain", "bb"].includes(t)) {
    return CaRule.brian();
  }
  return CaRule.life(parseRule(t || "conway"));
}

export function parseRule(s) {
  const t = String(s || "").trim().toLowerCase();
  if (!t) throw new Error("empty rule");
  const named = {
    conway: Rule.CONWAY,
    life: Rule.CONWAY,
    "b3/s23": Rule.CONWAY,
    highlife: Rule.HIGHLIFE,
    "b36/s23": Rule.HIGHLIFE,
    seeds: Rule.SEEDS,
    "b2/s": Rule.SEEDS,
    "b2/s0": Rule.SEEDS,
    daynight: Rule.DAYNIGHT,
    "day-night": Rule.DAYNIGHT,
    "day&night": Rule.DAYNIGHT,
    "b3678/s34678": Rule.DAYNIGHT,
    "life-without-death": Rule.LIFE_WITHOUT_DEATH,
    lwd: Rule.LIFE_WITHOUT_DEATH,
    "b3/s012345678": Rule.LIFE_WITHOUT_DEATH,
    maze: Rule.MAZE,
    "b3/s12345": Rule.MAZE,
    replicator: Rule.REPLICATOR,
    "b1357/s1357": Rule.REPLICATOR,
  };
  if (named[t]) return named[t];
  if (t.startsWith("b")) return parseBsForm(t.slice(1));
  const parts = t.split("/");
  if (parts.length === 2 && parts.every((p) => [...p].every((c) => c >= "0" && c <= "9"))) {
    return fromLists(parseDigits(parts[1]), parseDigits(parts[0]));
  }
  throw new Error(`unknown rule: ${s}`);
}

function parseBsForm(rest) {
  const [b, s] = rest.split("/");
  if (s == null || !s.startsWith("s")) throw new Error(`expected B#/S# form, got: b${rest}`);
  return fromLists(parseDigits(b), parseDigits(s.slice(1)));
}

function parseDigits(s) {
  const out = [];
  for (const c of s) {
    if (c < "0" || c > "9") throw new Error(`invalid character in rule counts: ${c}`);
    const d = c.charCodeAt(0) - 48;
    if (d > 8) throw new Error(`neighbor digit ${c} out of range`);
    out.push(d);
  }
  return out;
}

function fromLists(birth, survive) {
  let b = 0;
  let s = 0;
  for (const n of birth) b |= 1 << n;
  for (const n of survive) s |= 1 << n;
  return new Rule(b, s);
}

function maskDigits(mask) {
  let s = "";
  for (let i = 0; i <= 8; i += 1) if (mask & (1 << i)) s += String(i);
  return s;
}

export class Rng {
  constructor(seed) {
    this.x = seed === 0 || seed == null ? 0xdeadbeefcafebabe : BigInt(seed) & 0xffffffffffffffffn;
    if (this.x === 0n) this.x = 0xdeadbeefcafebaben;
  }
  nextU64() {
    let x = this.x;
    x ^= (x << 13n) & 0xffffffffffffffffn;
    x ^= x >> 7n;
    x ^= (x << 17n) & 0xffffffffffffffffn;
    this.x = x & 0xffffffffffffffffn;
    return this.x;
  }
  nextF64() {
    return Number(this.nextU64() >> 11n) / Number(1n << 53n);
  }
}

export class Grid {
  constructor(w, h, wrap = true, states = 2) {
    this.w = Math.max(1, w | 0);
    this.h = Math.max(1, h | 0);
    this.wrap = Boolean(wrap);
    this.states = Math.max(2, states | 0);
    const n = this.w * this.h;
    this.cells = new Uint8Array(n);
    this.ages = new Uint16Array(n);
    this.next = new Uint8Array(n);
    this.nextAges = new Uint16Array(n);
  }

  idx(x, y) {
    return y * this.w + x;
  }
  getState(x, y) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return 0;
    return this.cells[this.idx(x, y)];
  }
  setState(x, y, v) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    const i = this.idx(x, y);
    const value = Math.min(this.states - 1, Math.max(0, v));
    this.cells[i] = value;
    this.ages[i] = value ? 1 : 0;
  }
  get(x, y) {
    return this.getState(x, y) !== 0;
  }
  set(x, y, alive) {
    this.setState(x, y, alive ? LIVE : DEAD);
  }
  clear() {
    this.cells.fill(0);
    this.ages.fill(0);
  }
  population() {
    let n = 0;
    for (const c of this.cells) if (c) n += 1;
    return n;
  }
  resize(w, h) {
    const next = new Grid(w, h, this.wrap, this.states);
    const mw = Math.min(this.w, next.w);
    const mh = Math.min(this.h, next.h);
    for (let y = 0; y < mh; y += 1) {
      for (let x = 0; x < mw; x += 1) {
        const i = this.idx(x, y);
        const j = next.idx(x, y);
        next.cells[j] = this.cells[i];
        next.ages[j] = this.ages[i];
      }
    }
    return next;
  }
  countNeighbors(x, y, match = null) {
    return countNeighbors(this.cells, this.w, this.h, this.wrap, x, y, match);
  }
  syncAges() {
    for (let i = 0; i < this.cells.length; i += 1) this.ages[i] = this.cells[i] ? 1 : 0;
  }
  step(rule) {
    if (rule.kind === "brian") return this.stepBrian();
    return this.stepLife(rule.life);
  }
  stepLife(rule) {
    let births = 0;
    let deaths = 0;
    for (let y = 0; y < this.h; y += 1) {
      for (let x = 0; x < this.w; x += 1) {
        const n = this.countNeighbors(x, y, null);
        const i = this.idx(x, y);
        const alive = this.cells[i] !== 0;
        const nextAlive = rule.nextAlive(alive, n);
        if (nextAlive) {
          this.next[i] = LIVE;
          if (alive) this.nextAges[i] = this.ages[i] === 65535 ? 65535 : this.ages[i] + 1;
          else {
            this.nextAges[i] = 1;
            births += 1;
          }
        } else {
          this.next[i] = DEAD;
          this.nextAges[i] = 0;
          if (alive) deaths += 1;
        }
      }
    }
    [this.cells, this.next] = [this.next, this.cells];
    [this.ages, this.nextAges] = [this.nextAges, this.ages];
    return { births, deaths };
  }
  stepBrian() {
    let births = 0;
    let deaths = 0;
    for (let y = 0; y < this.h; y += 1) {
      for (let x = 0; x < this.w; x += 1) {
        const i = this.idx(x, y);
        const n = this.countNeighbors(x, y, LIVE);
        const cur = this.cells[i];
        let next = DEAD;
        let age = 0;
        if (cur === DEAD) {
          if (n === 2) {
            next = LIVE;
            age = 1;
            births += 1;
          }
        } else if (cur === LIVE) {
          next = REFRACTORY;
          age = this.ages[i] === 65535 ? 65535 : this.ages[i] + 1;
        } else {
          next = DEAD;
          deaths += 1;
        }
        this.next[i] = next;
        this.nextAges[i] = age;
      }
    }
    [this.cells, this.next] = [this.next, this.cells];
    [this.ages, this.nextAges] = [this.nextAges, this.ages];
    return { births, deaths };
  }
}

function countNeighbors(cells, w, h, wrap, x, y, match) {
  let n = 0;
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      if (dx === 0 && dy === 0) continue;
      let nx = x + dx;
      let ny = y + dy;
      if (wrap) {
        nx = ((nx % w) + w) % w;
        ny = ((ny % h) + h) % h;
      } else if (nx < 0 || ny < 0 || nx >= w || ny >= h) {
        continue;
      }
      const v = cells[ny * w + nx];
      if (match == null ? v !== 0 : v === match) n += 1;
    }
  }
  return n;
}

const ARTS = {
  blinker: "ooo",
  toad: ".ooo\nooo.",
  beacon: "oo..\noo..\n..oo\n..oo",
  block: "oo\noo",
  beehive: ".oo.\no..o\n.oo.",
  boat: "oo.\no.o\n.o.",
  loaf: ".oo.\no..o\n.o.o\n..o.",
  pond: ".oo.\no..o\no..o\n.oo.",
  barge: ".o..\no.o.\n.o.o\n..o.",
  bipole: "...oo\n..o.o\n.....\no.o..\noo...",
  clock: "..o.\no.o.\n.o.o\n.o..",
  lwss: ".o..o\no....\no...o\noooo.",
  mwss: "..o..\n.o...\no....\no...o\noooo.",
  hwss: "..oo..\n.o....\no.....\no....o\nooooo.",
  rpentomino: ".oo\noo.\n.o.",
  acorn: ".o.....\n...o...\noo..ooo",
  diehard: "......o.\noo......\n.o...ooo",
  pulsar:
    "..ooo...ooo..\n.............\no....o.o....o\no....o.o....o\no....o.o....o\n..ooo...ooo..\n.............\n..ooo...ooo..\no....o.o....o\no....o.o....o\no....o.o....o\n.............\n..ooo...ooo..",
  gosper:
    "........................o...........\n......................o.o...........\n............oo......oo............oo\n...........o...o....oo............oo\noo........o.....o...oo..............\noo........o...o.oo....o.o...........\n..........o.....o.......o...........\n...........o...o....................\n............oo......................",
  pentadecathlon: "..o....o..\noo.oooo.oo\n..o....o..",
  "glider-pair": ".o.....o.\n..o.....o\nooo...ooo",
  infinite1: "ooooooo.o\noo.o...oo",
  queenbee:
    ".........o.........\n.......o.o.........\n......o.o..........\noo...o..o.......oo.\noo....o.o.......oo.\n.......o.o.........\n.........o.........",
};

export const PATTERN_NAMES = [
  "random",
  "glider",
  "blinker",
  "toad",
  "beacon",
  "block",
  "beehive",
  "boat",
  "loaf",
  "pond",
  "barge",
  "bipole",
  "clock",
  "queenbee",
  "lwss",
  "mwss",
  "hwss",
  "rpentomino",
  "acorn",
  "diehard",
  "pulsar",
  "gosper",
  "pentadecathlon",
  "glider-pair",
  "infinite1",
];

function placeArt(grid, ox, oy, art) {
  for (const [dy, line] of art.split("\n").entries()) {
    for (let dx = 0; dx < line.length; dx += 1) {
      const c = line[dx];
      if (c === "o" || c === "O" || c === "#" || c === "*") grid.set(ox + dx, oy + dy, true);
    }
  }
}

function centerPlace(grid, art) {
  const lines = art.split("\n");
  const pw = Math.max(...lines.map((l) => l.length));
  const ph = lines.length;
  const ox = Math.max(0, Math.floor((grid.w - pw) / 2));
  const oy = Math.max(0, Math.floor((grid.h - ph) / 2));
  placeArt(grid, ox, oy, art);
}

export function seedPattern(grid, name, rng, density = 0.25) {
  const key = String(name || "random").toLowerCase();
  grid.clear();
  if (key === "random") {
    for (let i = 0; i < grid.cells.length; i += 1) grid.cells[i] = rng.nextF64() < density ? 1 : 0;
    grid.syncAges();
    return;
  }
  if (key === "glider" || key === "spaceship") {
    for (const [dx, dy] of [
      [1, 0],
      [2, 1],
      [0, 2],
      [1, 2],
      [2, 2],
    ])
      grid.set(1 + dx, 1 + dy, true);
    return;
  }
  if (key === "gosper") {
    if (grid.w < 40 || grid.h < 12) throw new Error("gosper needs at least a 40x12 grid");
    placeArt(grid, 1, 1, ARTS.gosper);
    return;
  }
  const art = ARTS[key] || ARTS[key.replaceAll("-", "")];
  if (!art) throw new Error(`unknown pattern: ${name}`);
  if (key === "lwss" || key === "mwss" || key === "hwss") {
    const ph = art.split("\n").length;
    placeArt(grid, 2, Math.max(0, Math.floor((grid.h - ph) / 2)), art);
    return;
  }
  centerPlace(grid, art);
}

export function loadRle(grid, text) {
  const trimmed = text.trim();
  if (!trimmed) throw new Error("empty pattern");
  let rule = null;
  let body = "";
  for (const line of trimmed.split(/\r?\n/)) {
    const t = line.trim();
    if (!t || t.startsWith("#")) continue;
    if (t.startsWith("x") || t.startsWith("X")) {
      const ruleMatch = t.match(/rule\s*=\s*([^,\s]+)/i);
      if (ruleMatch) rule = ruleMatch[1];
      continue;
    }
    body += t;
  }
  if (!body) {
    loadPlain(grid, text);
    return { rule };
  }
  const cells = [];
  let x = 0;
  let y = 0;
  let run = 0;
  let maxX = 0;
  const take = () => {
    const n = run || 1;
    run = 0;
    return n;
  };
  for (const ch of body) {
    if (ch >= "0" && ch <= "9") {
      run = run * 10 + (ch.charCodeAt(0) - 48);
      continue;
    }
    if (ch === "b" || ch === "B") {
      x += take();
    } else if (ch === "o" || ch === "O") {
      const n = take();
      for (let i = 0; i < n; i += 1) cells.push([x + i, y]);
      x += n;
    } else if (ch === "$") {
      y += take();
      maxX = Math.max(maxX, x);
      x = 0;
    } else if (ch === "!") {
      break;
    }
    maxX = Math.max(maxX, x);
  }
  const pw = Math.max(1, maxX);
  const ph = y + 1;
  const ox = Math.max(0, Math.floor((grid.w - pw) / 2));
  const oy = Math.max(0, Math.floor((grid.h - ph) / 2));
  grid.clear();
  for (const [cx, cy] of cells) grid.set(ox + cx, oy + cy, true);
  return { rule };
}

function loadPlain(grid, text) {
  const lines = text
    .split(/\r?\n/)
    .map((l) => l.trimEnd())
    .filter((l) => l && !l.startsWith("#"));
  const pw = Math.max(...lines.map((l) => l.length), 1);
  const ph = lines.length;
  const ox = Math.max(0, Math.floor((grid.w - pw) / 2));
  const oy = Math.max(0, Math.floor((grid.h - ph) / 2));
  grid.clear();
  lines.forEach((line, dy) => {
    [...line].forEach((c, dx) => {
      if ("oO*#".includes(c)) grid.set(ox + dx, oy + dy, true);
    });
  });
}

export function toRle(grid, name, rule = "B3/S23") {
  let minX = grid.w;
  let minY = grid.h;
  let maxX = 0;
  let maxY = 0;
  let any = false;
  for (let y = 0; y < grid.h; y += 1) {
    for (let x = 0; x < grid.w; x += 1) {
      if (grid.get(x, y)) {
        any = true;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
    }
  }
  let out = `#N ${name}\n#O gol-rs\n#C generated by gol-rs\n`;
  if (!any) return `${out}x = 0, y = 0, rule = ${rule}\n!\n`;
  const w = maxX - minX + 1;
  const h = maxY - minY + 1;
  out += `x = ${w}, y = ${h}, rule = ${rule}\n`;
  let body = "";
  for (let y = minY; y <= maxY; y += 1) {
    let x = minX;
    while (x <= maxX) {
      const alive = grid.get(x, y);
      let run = 1;
      while (x + run <= maxX && grid.get(x + run, y) === alive) run += 1;
      if (run > 1) body += String(run);
      body += alive ? "o" : "b";
      x += run;
    }
    body = body.replace(/(\d+)?b$/, "");
    if (y < maxY) body += "$";
  }
  body += "!";
  for (let i = 0; i < body.length; i += 1) {
    if (i > 0 && i % 70 === 0) out += "\n";
    out += body[i];
  }
  return `${out}\n`;
}

const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;

export function hashGrid(grid) {
  let h = FNV_OFFSET;
  const mix = (b) => {
    h = ((h ^ BigInt(b)) * FNV_PRIME) & 0xffffffffffffffffn;
  };
  mix(grid.w & 255);
  mix((grid.w >> 8) & 255);
  mix(grid.h & 255);
  mix((grid.h >> 8) & 255);
  mix(grid.states);
  if (grid.states <= 2) {
    let acc = 0;
    let bit = 0;
    for (const cell of grid.cells) {
      if (cell) acc |= 1 << bit;
      bit += 1;
      if (bit === 8) {
        mix(acc);
        acc = 0;
        bit = 0;
      }
    }
    if (bit) mix(acc);
  } else {
    for (const cell of grid.cells) mix(cell);
  }
  return h.toString(16);
}

export const THEMES = {
  classic: { live: [0, 220, 80], dead: [6, 8, 10], accent: "#2ee59d" },
  neon: { live: [0, 255, 220], dead: [8, 0, 20], accent: "#5cf0ff" },
  fire: { live: [255, 90, 20], dead: [12, 4, 0], accent: "#ff6a20" },
  ocean: { live: [40, 160, 255], dead: [0, 8, 24], accent: "#4aa3ff" },
  mono: { live: [230, 230, 230], dead: [8, 8, 8], accent: "#d0d0d0" },
};

export function ageRgb(theme, age, state) {
  if (state === DEAD) return THEMES[theme].dead;
  if (state === REFRACTORY) {
    const [r, g, b] = THEMES[theme].live;
    return [Math.floor(r * 0.35), Math.floor(g * 0.35), Math.floor(b * 0.45)];
  }
  if (theme === "fire") {
    if (age <= 1) return [255, 255, 100];
    if (age <= 7) return [255, 140, 0];
    if (age <= 20) return [255, 60, 0];
    return [200, 0, 0];
  }
  if (theme === "neon") {
    if (age <= 1) return [0, 255, 255];
    if (age <= 7) return [80, 120, 255];
    return [255, 0, 255];
  }
  if (theme === "ocean") {
    if (age <= 1) return [180, 240, 255];
    if (age <= 7) return [40, 160, 255];
    return [0, 60, 160];
  }
  if (theme === "mono") {
    const v = age <= 1 ? 255 : age <= 7 ? 200 : 120;
    return [v, v, v];
  }
  if (age <= 1) return [0, 255, 80];
  if (age <= 7) return [220, 255, 0];
  if (age <= 20) return [255, 200, 0];
  return [220, 0, 0];
}
