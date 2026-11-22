import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import ts from "typescript";

async function sourceModule(path) {
  const source = await readFile(new URL(path, import.meta.url), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
}
const { GamepadNavigationInput } = await sourceModule("../src/components/navigationInput.ts");
const { defaultDashboard, validateDashboard, reorderPanel } = await sourceModule("../src/components/dashboardState.ts");
function pad(buttons = [], axes = [0, 0]) { return { buttons: Array.from({ length: 16 }, (_, index) => ({ pressed: buttons.includes(index) })), axes }; }

test("held confirmation cannot activate a control on entry or after focus recovery", () => {
  const input = new GamepadNavigationInput();
  assert.equal(input.read(pad([0]), 0), null);
  assert.equal(input.read(pad([0]), 1000), null);
  input.read(pad(), 1100);
  assert.equal(input.read(pad([0]), 1200), "confirm");
  assert.equal(input.read(pad([0]), 5000), null);
  input.reset();
  assert.equal(input.read(pad([0]), 6000), null);
  input.read(pad(), 6100);
  assert.equal(input.read(pad([0]), 6200), "confirm");
});
test("directional repeat has an initial delay and back never repeats", () => {
  const input = new GamepadNavigationInput();
  input.read(pad(), 0);
  assert.equal(input.read(pad([15]), 10), "right");
  assert.equal(input.read(pad([15]), 409), null);
  assert.equal(input.read(pad([15]), 410), "right");
  assert.equal(input.read(pad([15]), 569), null);
  assert.equal(input.read(pad([15]), 570), "right");
  input.read(pad(), 600);
  assert.equal(input.read(pad([1]), 700), "back");
  assert.equal(input.read(pad([1]), 1700), null);
});
test("stick dead zone filters drift and supports all directions", () => {
  const input = new GamepadNavigationInput();
  assert.equal(input.read(pad([], [0.4, -0.4]), 0), null);
  assert.equal(input.read(pad([], [-0.8, 0]), 1), "left");
  input.read(pad(), 2);
  assert.equal(input.read(pad([], [0, -0.8]), 3), "up");
  input.read(pad(), 4);
  assert.equal(input.read(pad([], [0, 0.8]), 5), "down");
});
test("invalid saved dashboard layouts fall back instead of losing panels", () => {
  const valid = defaultDashboard();
  assert.deepEqual(validateDashboard(valid), valid);
  for (const value of [null, {}, [], valid.slice(1), [...valid.slice(0, 4), valid[0]], valid.map((p) => ({ ...p, height: Infinity })), valid.map((p) => ({ ...p, height: 900 })), valid.map((p) => ({ ...p, wide: "yes" })), valid.map((p) => ({ ...p, id: "unknown" }))]) assert.equal(validateDashboard(value), null);
});
test("moving a dashboard panel preserves its size and the remaining panel order", () => {
  const before = defaultDashboard();
  const after = reorderPanel(before, "performance", 3);
  assert.deepEqual(after.map((p) => p.id), ["games", "community", "ranking", "performance", "events"]);
  assert.deepEqual(after[3], before[0]);
  assert.equal(before[0].id, "performance");
  assert.equal(reorderPanel(before, "games", -1), before);
  assert.equal(reorderPanel(before, "games", 5), before);
});

const { defaultCollection, validateCollection, selectCollection } = await sourceModule("../src/components/collectionState.ts");
const collectionRooms = [
  { id: "alpha", title: "Álpha", playersOnline: 1, steamAppId: 1 },
  { id: "beta", title: "Beta", playersOnline: 2, steamAppId: 2 },
  { id: "gamma", title: "Gamma", playersOnline: 3, steamAppId: 3 },
];
test("collection search ignores accents and favorite filters do not mutate selection data", () => {
  const preferences = { ...defaultCollection(), favorites: ["beta"] };
  assert.deepEqual(selectCollection(collectionRooms, [], preferences, " alpha ", "all").map(r => r.id), ["alpha"]);
  assert.deepEqual(selectCollection(collectionRooms, [], preferences, "", "favorites").map(r => r.id), ["beta"]);
  assert.deepEqual(collectionRooms.map(r => r.id), ["alpha", "beta", "gamma"]);
});
test("developer order puts games without developer last and name order is deterministic", () => {
  const rooms = [{ ...collectionRooms[0], developer: "Toaplan" }, { ...collectionRooms[1] }, { ...collectionRooms[2], developer: "Cave" }];
  assert.deepEqual(selectCollection(rooms, [], { ...defaultCollection(), sort: "developer" }, "", "all").map(r => r.id), ["gamma", "alpha", "beta"]);
  const runs = [{ gameId: "beta", startedAt: 200 }, { gameId: "alpha", startedAt: 100 }];
  assert.deepEqual(selectCollection(collectionRooms, runs, defaultCollection(), "", "recorded").map(r => r.id), ["alpha", "beta"]);
  assert.deepEqual(selectCollection(collectionRooms, [], {...defaultCollection(), descending: true}, "", "all").map(r => r.id), ["gamma", "beta", "alpha"]);
});
test("saved collection preferences reject malformed values and deduplicate favorite ids", () => {
  assert.deepEqual(validateCollection(null), defaultCollection());
  assert.deepEqual(validateCollection({view:"invalid",sort:"invalid",descending:"yes",favorites:["beta",7,null,"beta",""]}), {...defaultCollection(),favorites:["beta"]});
});

const { defaultPreferences, validatePreferences } = await sourceModule("../src/components/settingsState.ts");
test("interface preferences migrate existing theme and density without inventing a startup mode", () => {
  assert.deepEqual(validatePreferences({appearance:"dark",compact:true}), {appearance:"dark",compact:true,startup:"collection",startupBigPicture:false,language:"pt-BR"});
  assert.deepEqual(validatePreferences({appearance:"light",compact:false,startup:"collection",startupBigPicture:true,language:"pt-BR"}), {appearance:"light",compact:false,startup:"collection",startupBigPicture:true,language:"pt-BR"});
});
test("invalid startup, theme and boolean settings fall back to safe defaults", () => {
  assert.deepEqual(validatePreferences({appearance:"unknown",compact:"yes",startup:"profile",startupBigPicture:"true"}), defaultPreferences());
  assert.deepEqual(validatePreferences(null), defaultPreferences());
});

const { translate, ENGLISH } = await sourceModule("../src/i18nMessages.ts");
test("language switching preserves source data and interpolates names as plain text", () => {
  assert.equal(translate("en", "Coleção"), "Collection");
  assert.equal(translate("pt-BR", "Coleção"), "Coleção");
  assert.equal(translate("en", " de "), " of ");
  assert.equal(translate("en", "Selecionar {game}", {game:"DoDonPachi"}), "Select DoDonPachi");
  assert.equal(translate("en", "Perfil de {name}", {name:"<Piloto>"}), "Profile for <Piloto>");
  assert.equal(translate("en", "Mushihimesama"), "Mushihimesama");
});
test("saved language preferences are validated and older preferences retain Portuguese", () => {
  assert.equal(validatePreferences({language:"en"}).language,"en");
  assert.equal(validatePreferences({language:"unknown"}).language,"pt-BR");
  assert.equal(validatePreferences({appearance:"dark"}).language,"pt-BR");
});
test("all static translation references have an English entry", async () => {
  const files = [new URL("../src/App.tsx", import.meta.url), ...(await (await import("node:fs/promises")).readdir(new URL("../src/components", import.meta.url))).filter(name=>name.endsWith(".tsx")).map(name=>new URL(`../src/components/${name}`,import.meta.url))];
  const missing=new Set();
  for(const file of files){
    const ast=ts.createSourceFile(file.pathname,await readFile(file,"utf8"),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
    function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(ast)==="t"&&node.arguments[0]&&ts.isStringLiteral(node.arguments[0])){const key=node.arguments[0].text.trim();if(/[a-zA-ZÀ-ÿ]/.test(key)&&!Object.hasOwn(ENGLISH,key))missing.add(key);}ts.forEachChild(node,visit);}
    visit(ast);
  }
  assert.deepEqual([...missing],[]);
});

const { validateMapping, mappedPad, bindingIdentity } = await sourceModule("../src/components/controllerMappings.ts");
const controllerProfile = {deviceId:"Arcade test",bindings:{up:{kind:"axis",index:1,direction:-1},down:{kind:"axis",index:1,direction:1},left:{kind:"axis",index:0,direction:-1},right:{kind:"axis",index:0,direction:1},confirm:{kind:"button",index:2},back:{kind:"button",index:3}}};
test("controller profiles require all six unique inputs and bounded indexes",()=>{
  assert.deepEqual(validateMapping(controllerProfile),controllerProfile);
  assert.equal(validateMapping({...controllerProfile,bindings:{...controllerProfile.bindings,back:{kind:"button",index:2}}}),null);
  assert.equal(validateMapping({...controllerProfile,bindings:{...controllerProfile.bindings,up:{kind:"axis",index:1,direction:0}}}),null);
  assert.equal(validateMapping({...controllerProfile,bindings:{...controllerProfile.bindings,back:{kind:"button",index:128}}}),null);
  assert.equal(validateMapping({deviceId:"Arcade",bindings:{}}),null);
  assert.notEqual(bindingIdentity(controllerProfile.bindings.up),bindingIdentity(controllerProfile.bindings.down));
});
test("custom axes and buttons feed navigation without assuming a standard gamepad layout",()=>{
  const raw={buttons:Array.from({length:8},(_,index)=>({pressed:index===2})),axes:[0,-0.9]};
  const mapped=mappedPad(raw,controllerProfile);
  assert.equal(mapped.buttons[0].pressed,true);
  assert.equal(mapped.buttons[12].pressed,true);
  assert.equal(mapped.buttons[13].pressed,false);
  assert.equal(mapped.buttons[1].pressed,false);
  assert.deepEqual(mapped.axes,[0,0]);
  const input=new GamepadNavigationInput();
  assert.equal(input.read(mapped,0),null);
  input.read(mappedPad({buttons:[],axes:[0,0]},controllerProfile),1);
  assert.equal(input.read(mapped,2),"confirm");
});

const { applyRunAnalysis } = await sourceModule("../src/types/submission.ts");
test("analysis events identify the captured run and preserve the capture end time", () => {
  const runs = [
    { id: "old", nativePid: 10, gameId: "earthion", startedAt: 0, status: "analyzing", endedAt: 20 },
    { id: "new", nativePid: 11, gameId: "earthion", startedAt: 21, status: "recording" },
  ];
  const result = applyRunAnalysis(runs, { pid: 10, jogo_id: "earthion", status: "failed", message: "HUD" }, 30);
  assert.equal(result[0].status, "failed");
  assert.equal(result[0].endedAt, 20);
  assert.equal(result[0].message, "HUD");
  assert.strictEqual(result[1], runs[1]);
  assert.deepEqual(applyRunAnalysis(result, { pid: 10, jogo_id: "earthion", status: "analyzing" }, 40), result);
  assert.deepEqual(applyRunAnalysis(runs, { pid: 11, jogo_id: "other", status: "submitted" }, 40), runs);
});
test("recording is not marked submitted until native submission confirmation", () => {
  const runs = [{ id: "run", nativePid: 10, gameId: "earthion", startedAt: 0, status: "recording" }];
  const analyzing = applyRunAnalysis(runs, { pid: 10, jogo_id: "earthion", status: "analyzing" }, 20);
  assert.equal(analyzing[0].status, "analyzing");
  assert.equal(analyzing[0].endedAt, 20);
  const submitted = applyRunAnalysis(analyzing, { pid: 10, jogo_id: "earthion", status: "submitted" }, 50);
  assert.equal(submitted[0].status, "submitted");
  assert.equal(submitted[0].endedAt, 20);
});

const { answer: robotxAnswer, robotxFromGame } = await sourceModule("../src/data/robotx.ts");
test("RobotX responde pela pergunta curada mais parecida e admite quando não sabe", () => {
  const items = [
    { question: "Quais são os controles?", answer: "Z atira, X bomba." },
    { question: "Qual a nave mais rápida?", answer: "A Type-B é a mais rápida." },
    { question: "Como vencer o chefão da fase 3?", answer: "Fique embaixo e mire no núcleo." },
  ];
  assert.equal(robotxAnswer("@RobotX Oi qual a nave mais rápida desse jogo?", items).text, "A Type-B é a mais rápida.");
  assert.equal(robotxAnswer("@robotx controles", items).text, "Z atira, X bomba.");
  assert.equal(robotxAnswer("como passo do chefao da fase 3", items).text, "Fique embaixo e mire no núcleo.");
  const unknown = robotxAnswer("@RobotX qual a previsão do tempo amanhã?", items);
  assert.equal(unknown.matched, undefined);
  assert.match(unknown.text, /Ainda não sei/);
  assert.deepEqual(robotxFromGame([{ q: "a", a: "b" }, { q: "" }, "x"]), [{ question: "a", answer: "b" }]);
});

const { moveTab } = await sourceModule("../src/components/tabOrder.ts");
test("abas reordenam ao arrastar", () => {
  assert.deepEqual(moveTab(["a", "b", "c"], "a", 2), ["b", "c", "a"]);
  assert.deepEqual(moveTab(["a", "b", "c"], "c", 0), ["c", "a", "b"]);
  assert.deepEqual(moveTab(["a", "b", "c"], "b", -1), ["b", "a", "c"]);
  assert.deepEqual(moveTab(["a", "b", "c"], "b", 9), ["a", "c", "b"]);
});
