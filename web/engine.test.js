import assert from "node:assert/strict";
import test from "node:test";
import {
  Grid,
  parseCaRule,
  parseRule,
  seedPattern,
  Rng,
  loadRle,
  toRle,
  hashGrid,
  Rule,
} from "./static/engine.js";

test("conway blinker has period 2", () => {
  const grid = new Grid(5, 5, true, 2);
  seedPattern(grid, "blinker", new Rng(1));
  const a = hashGrid(grid);
  grid.step(parseCaRule("conway"));
  const b = hashGrid(grid);
  grid.step(parseCaRule("conway"));
  assert.notEqual(a, b);
  assert.equal(hashGrid(grid), a);
});

test("block is a still life", () => {
  const grid = new Grid(8, 8, true, 2);
  seedPattern(grid, "block", new Rng(1));
  const before = hashGrid(grid);
  grid.step(parseCaRule("conway"));
  assert.equal(hashGrid(grid), before);
  assert.equal(grid.population(), 4);
});

test("conway births only on 3", () => {
  const r = parseRule("conway");
  assert.equal(r.births(3), true);
  assert.equal(r.births(2), false);
  assert.equal(r.survives(2), true);
  assert.equal(r.nextAlive(true, 1), false);
});

test("highlife births on 6", () => {
  assert.equal(parseRule("highlife").births(6), true);
  assert.equal(Rule.CONWAY.births(6), false);
});

test("brian parse is multistate", () => {
  const rule = parseCaRule("brian");
  assert.equal(rule.isMultistate(), true);
  assert.equal(rule.states(), 3);
});

test("rle round trip preserves a glider", () => {
  const grid = new Grid(12, 12, true, 2);
  seedPattern(grid, "glider", new Rng(1));
  const text = toRle(grid, "glider", "B3/S23");
  const next = new Grid(12, 12, true, 2);
  loadRle(next, text);
  const again = new Grid(12, 12, true, 2);
  loadRle(again, toRle(next, "glider", "B3/S23"));
  assert.equal(next.population(), 5);
  assert.equal(hashGrid(again), hashGrid(next));
  assert.match(text, /rule = B3\/S23/);
});

test("rng is deterministic", () => {
  const a = new Rng(42);
  const b = new Rng(42);
  for (let i = 0; i < 32; i += 1) assert.equal(a.nextU64(), b.nextU64());
});
