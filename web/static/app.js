import {
  CaRule,
  Grid,
  PATTERN_NAMES,
  RULE_PRESETS,
  Rng,
  THEMES,
  ageRgb,
  hashGrid,
  loadRle,
  parseCaRule,
  seedPattern,
  toRle,
} from "./engine.js";

const els = {
  stage: document.querySelector("#stage"),
  spark: document.querySelector("#spark"),
  play: document.querySelector("#play"),
  step: document.querySelector("#step"),
  reseed: document.querySelector("#reseed"),
  pattern: document.querySelector("#pattern"),
  rule: document.querySelector("#rule"),
  theme: document.querySelector("#theme"),
  speed: document.querySelector("#speed"),
  speedOut: document.querySelector("#speedOut"),
  width: document.querySelector("#width"),
  height: document.querySelector("#height"),
  widthOut: document.querySelector("#widthOut"),
  heightOut: document.querySelector("#heightOut"),
  wrap: document.querySelector("#wrap"),
  age: document.querySelector("#age"),
  status: document.querySelector("#status"),
  stats: document.querySelector("#statsChip"),
  ruleChip: document.querySelector("#ruleChip"),
  rle: document.querySelector("#rle"),
  loadRle: document.querySelector("#loadRle"),
  copyRle: document.querySelector("#copyRle"),
  share: document.querySelector("#share"),
};

const state = {
  grid: new Grid(120, 70, true, 2),
  rule: parseCaRule("conway"),
  pattern: "gosper",
  theme: "classic",
  delay: 80,
  running: true,
  drawing: false,
  erase: false,
  gen: 0,
  popHistory: [],
  hashes: [],
  seed: 1,
};

function fillSelect(select, values) {
  select.replaceChildren(
    ...values.map((value) => {
      const option = document.createElement("option");
      option.value = typeof value === "string" ? value : value.id;
      option.textContent = typeof value === "string" ? value : value.label;
      return option;
    }),
  );
}

function applyTheme() {
  const theme = THEMES[state.theme];
  document.documentElement.style.setProperty("--accent", theme.accent);
}

function cellAtEvent(event) {
  const rect = els.stage.getBoundingClientRect();
  const x = Math.floor(((event.clientX - rect.left) / rect.width) * state.grid.w);
  const y = Math.floor(((event.clientY - rect.top) / rect.height) * state.grid.h);
  return { x, y };
}

function paint(event) {
  const { x, y } = cellAtEvent(event);
  if (state.rule.isMultistate() && !state.erase) state.grid.setState(x, y, 1);
  else state.grid.set(x, y, !state.erase);
  render();
}

function reseed() {
  state.grid = new Grid(
    Number(els.width.value),
    Number(els.height.value),
    els.wrap.checked,
    state.rule.states(),
  );
  state.seed = (state.seed * 1103515245 + 12345) >>> 0 || 1;
  try {
    seedPattern(state.grid, state.pattern, new Rng(state.seed), 0.22);
    els.status.textContent = "";
  } catch (error) {
    seedPattern(state.grid, "glider", new Rng(state.seed), 0.22);
    els.status.textContent = error.message;
  }
  state.gen = 0;
  state.popHistory = [state.grid.population()];
  state.hashes = [];
  render();
}

function tick() {
  const stats = state.grid.step(state.rule);
  state.gen += 1;
  state.popHistory = [...state.popHistory, state.grid.population()].slice(-120);
  const hash = hashGrid(state.grid);
  const prev = state.hashes.indexOf(hash);
  state.hashes = [...state.hashes, hash].slice(-64);
  if (prev >= 0) {
    const period = state.hashes.length - 1 - prev;
    els.status.textContent = period === 1 ? "Still life / stable." : `Cycle detected, period ${period}.`;
  }
  els.stats.textContent = `gen ${state.gen} · pop ${state.grid.population()} · +${stats.births}/-${stats.deaths}`;
  render();
}

function render() {
  const ctx = els.stage.getContext("2d");
  const { w, h } = state.grid;
  const cw = Math.floor(els.stage.width / w);
  const ch = Math.floor(els.stage.height / h);
  const theme = THEMES[state.theme];
  ctx.fillStyle = `rgb(${theme.dead.join(",")})`;
  ctx.fillRect(0, 0, els.stage.width, els.stage.height);
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const i = state.grid.idx(x, y);
      const cell = state.grid.cells[i];
      if (!cell) continue;
      const rgb = els.age.checked ? ageRgb(state.theme, state.grid.ages[i], cell) : cell === 2
        ? [theme.live[0] * 0.35, theme.live[1] * 0.35, theme.live[2] * 0.45]
        : theme.live;
      ctx.fillStyle = `rgb(${rgb.map((v) => Math.round(v)).join(",")})`;
      ctx.fillRect(x * cw, y * ch, Math.max(1, cw - 1), Math.max(1, ch - 1));
    }
  }
  const spark = els.spark.getContext("2d");
  spark.fillStyle = "#050608";
  spark.fillRect(0, 0, els.spark.width, els.spark.height);
  if (state.popHistory.length > 1) {
    const max = Math.max(...state.popHistory, 1);
    spark.strokeStyle = theme.accent;
    spark.beginPath();
    state.popHistory.forEach((pop, i) => {
      const x = (i / (state.popHistory.length - 1)) * (els.spark.width - 8) + 4;
      const y = els.spark.height - 6 - (pop / max) * (els.spark.height - 12);
      if (i === 0) spark.moveTo(x, y);
      else spark.lineTo(x, y);
    });
    spark.stroke();
  }
  els.ruleChip.textContent = state.rule.displayName();
  els.stats.textContent = `gen ${state.gen} · pop ${state.grid.population()}`;
}

function syncHash() {
  const url = new URL(window.location.href);
  url.hash = new URLSearchParams({
    p: state.pattern,
    r: els.rule.value,
    t: state.theme,
    w: String(state.grid.w),
    h: String(state.grid.h),
  }).toString();
  window.history.replaceState(null, "", url);
}

function loadHash() {
  const params = new URLSearchParams(window.location.hash.replace(/^#/, ""));
  if (params.get("p")) state.pattern = params.get("p");
  if (params.get("r")) {
    els.rule.value = params.get("r");
    state.rule = parseCaRule(params.get("r"));
  }
  if (params.get("t")) {
    state.theme = params.get("t");
    els.theme.value = state.theme;
  }
  if (params.get("w")) els.width.value = params.get("w");
  if (params.get("h")) els.height.value = params.get("h");
}

fillSelect(els.pattern, PATTERN_NAMES);
fillSelect(els.rule, RULE_PRESETS);
els.pattern.value = "gosper";
els.rule.value = "conway";
loadHash();
els.widthOut.textContent = els.width.value;
els.heightOut.textContent = els.height.value;
applyTheme();
reseed();

els.play.addEventListener("click", () => {
  state.running = !state.running;
  els.play.textContent = state.running ? "Pause" : "Play";
});
els.step.addEventListener("click", () => {
  state.running = false;
  els.play.textContent = "Play";
  tick();
});
els.reseed.addEventListener("click", reseed);
els.pattern.addEventListener("change", () => {
  state.pattern = els.pattern.value;
  reseed();
  syncHash();
});
els.rule.addEventListener("change", () => {
  state.rule = parseCaRule(els.rule.value);
  reseed();
  syncHash();
});
els.theme.addEventListener("change", () => {
  state.theme = els.theme.value;
  applyTheme();
  render();
  syncHash();
});
els.speed.addEventListener("input", () => {
  state.delay = Number(els.speed.value);
  els.speedOut.textContent = `${state.delay} ms`;
});
els.width.addEventListener("input", () => {
  els.widthOut.textContent = els.width.value;
});
els.height.addEventListener("input", () => {
  els.heightOut.textContent = els.height.value;
});
els.width.addEventListener("change", reseed);
els.height.addEventListener("change", reseed);
els.wrap.addEventListener("change", () => {
  state.grid.wrap = els.wrap.checked;
});
els.age.addEventListener("change", render);
els.stage.addEventListener("pointerdown", (event) => {
  state.drawing = true;
  state.erase = event.shiftKey || event.buttons === 2;
  paint(event);
});
window.addEventListener("pointerup", () => {
  state.drawing = false;
});
els.stage.addEventListener("pointermove", (event) => {
  if (state.drawing) paint(event);
});
els.stage.addEventListener("contextmenu", (event) => event.preventDefault());
els.copyRle.addEventListener("click", async () => {
  const text = toRle(state.grid, state.pattern, state.rule.toBs());
  await navigator.clipboard.writeText(text);
  els.status.textContent = "RLE copied.";
});
els.share.addEventListener("click", async () => {
  syncHash();
  await navigator.clipboard.writeText(window.location.href);
  els.status.textContent = "Link copied.";
});
els.loadRle.addEventListener("click", () => {
  try {
    const info = loadRle(state.grid, els.rle.value);
    if (info.rule) {
      state.rule = parseCaRule(info.rule);
      const match = RULE_PRESETS.find((preset) => parseCaRule(preset.id).toBs() === state.rule.toBs());
      if (match) els.rule.value = match.id;
    }
    state.gen = 0;
    state.popHistory = [state.grid.population()];
    els.status.textContent = "RLE loaded.";
    render();
  } catch (error) {
    els.status.textContent = error.message;
  }
});

window.addEventListener("keydown", (event) => {
  if (["INPUT", "TEXTAREA", "SELECT"].includes(event.target.tagName)) return;
  if (event.code === "Space") {
    event.preventDefault();
    els.play.click();
  } else if (event.key === ".") els.step.click();
  else if (event.key === "r") els.reseed.click();
  else if (event.key === "t") {
    const names = Object.keys(THEMES);
    state.theme = names[(names.indexOf(state.theme) + 1) % names.length];
    els.theme.value = state.theme;
    applyTheme();
    render();
  } else if (event.key === "a") {
    els.age.checked = !els.age.checked;
    render();
  } else if (event.key === "w") {
    els.wrap.checked = !els.wrap.checked;
    state.grid.wrap = els.wrap.checked;
  } else if (event.key === "s") els.copyRle.click();
  else if (event.key === "+" || event.key === "=") {
    els.speed.value = String(Math.max(15, Number(els.speed.value) / 2));
    els.speed.dispatchEvent(new Event("input"));
  } else if (event.key === "-") {
    els.speed.value = String(Math.min(240, Number(els.speed.value) * 2));
    els.speed.dispatchEvent(new Event("input"));
  }
});

function loop() {
  if (state.running) tick();
  window.setTimeout(loop, state.delay);
}
loop();
