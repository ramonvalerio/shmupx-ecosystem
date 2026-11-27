import { readFile } from "node:fs/promises";
import { after, before, beforeEach, test } from "node:test";
import { initializeTestEnvironment, assertFails, assertSucceeds } from "@firebase/rules-unit-testing";
import { collection, doc, getDoc, getDocs, setDoc, updateDoc, deleteDoc, deleteField,
  query, where, orderBy, limit, serverTimestamp, Timestamp } from "firebase/firestore";

let environment;
const now = Timestamp.fromMillis(1_700_000_000_000);
const profile = () => ({ displayName: "Piloto", locale: "pt-BR",
  createdAt: serverTimestamp(), updatedAt: serverTimestamp() });
const db = (uid) => uid ? environment.authenticatedContext(uid).firestore()
  : environment.unauthenticatedContext().firestore();

before(async () => {
  environment = await initializeTestEnvironment({
    projectId: "demo-shmupx",
    firestore: { rules: await readFile(new URL("../firestore.rules", import.meta.url), "utf8") },
  });
});
after(async () => { await environment?.cleanup(); });
beforeEach(async () => {
  await environment.clearFirestore();
  await environment.withSecurityRulesDisabled(async (context) => {
    const store = context.firestore();
    const rows = {
      "users/alice": { displayName: "Alice", locale: "en", createdAt: now, updatedAt: now },
      "publicProfiles/alice": { displayName: "Alice" },
      "games/earthion": { title: "Earthion", published: true },
      "games/draft": { title: "Draft", published: false },
      "events/cup": { published: true, startsAt: now },
      "events/private": { published: false, startsAt: now },
      "plans/pro": { name: "Pro", active: true },
      "plans/old": { name: "Old", active: false },
      "gameplays/run-a": { ownerId: "alice", status: "approved", createdAt: now },
      "gameplays/run-b": { ownerId: "bob", status: "analyzing", createdAt: now },
      "playerStats/alice_earthion": { ownerId: "alice", gameId: "earthion", credits: 3, seconds: 900 },
      "wallets/alice": { balance: 3 },
      "wallets/alice/entries/debit": { amount: -1 },
      "seasons/current": { published: true },
      "seasons/draft": { published: false },
      "seasons/current/gameRankings/earthion/entries/alice": { bestScore: 123, achievedAt: now },
      "seasons/current/globalRankings/alice": { totalRating: 5, playerId: "alice" },
      "seasons/draft/globalRankings/alice": { totalRating: 9, playerId: "alice" },
      "analysisJobs/internal": { diagnostics: "private" },
    };
    await Promise.all(Object.entries(rows).map(([path, value]) => setDoc(doc(store, path), value)));
  });
});

test("anonymous access is denied, including rankings and public profiles", async () => {
  for (const path of ["users/alice", "publicProfiles/alice", "games/earthion",
    "gameplays/run-a", "seasons/current/globalRankings/alice"])
    await assertFails(getDoc(doc(db(), path)));
});

test("email/password sign-ups need a confirmed email; Google sign-ins don't", async () => {
  const as = (token) => environment.authenticatedContext("carol", token).firestore();
  const unconfirmed = as({ email: "carol@example.com", email_verified: false, firebase: { sign_in_provider: "password" } });
  await assertFails(getDoc(doc(unconfirmed, "users/carol")));
  await assertFails(setDoc(doc(unconfirmed, "users/carol"), profile()));
  const confirmed = as({ email: "carol@example.com", email_verified: true, firebase: { sign_in_provider: "password" } });
  await assertSucceeds(setDoc(doc(confirmed, "users/carol"), profile()));
  const google = as({ email: "carol@example.com", email_verified: false, firebase: { sign_in_provider: "google.com" } });
  await assertSucceeds(getDoc(doc(google, "users/carol")));
});
test("private profiles are only readable by their owner", async () => {
  await assertSucceeds(getDoc(doc(db("alice"), "users/alice")));
  await assertFails(getDoc(doc(db("bob"), "users/alice")));
  await assertSucceeds(getDoc(doc(db("bob"), "publicProfiles/alice")));
});

test("profile creation validates ownership, allowed fields and server timestamps", async () => {
  const store = db("bob");
  await assertFails(setDoc(doc(store, "users/other"), profile()));
  await assertFails(setDoc(doc(store, "users/bob"), { ...profile(), role: "admin" }));
  await assertFails(setDoc(doc(store, "users/bob"), { ...profile(), balance: 999 }));
  await assertFails(setDoc(doc(store, "users/bob"), { ...profile(), createdAt: now }));
  await assertFails(setDoc(doc(store, "users/bob"), { ...profile(), displayName: "X".repeat(33) }));
  await assertSucceeds(setDoc(doc(store, "users/bob"), profile()));
});

test("profile updates cannot remove required data or change creation time", async () => {
  const ref = doc(db("alice"), "users/alice");
  await assertFails(updateDoc(ref, { locale: deleteField(), updatedAt: serverTimestamp() }));
  await assertFails(updateDoc(ref, { createdAt: serverTimestamp(), updatedAt: serverTimestamp() }));
  await assertFails(updateDoc(ref, { role: "admin", updatedAt: serverTimestamp() }));
  await assertSucceeds(updateDoc(ref, { displayName: "New name", updatedAt: serverTimestamp() }));
});

test("catalog and events require published filters", async () => {
  const store = db("alice");
  await assertSucceeds(getDocs(query(collection(store, "games"), where("published", "==", true), limit(50))));
  await assertSucceeds(getDocs(query(collection(store, "events"), where("published", "==", true), orderBy("startsAt"), limit(50))));
  await assertFails(getDocs(collection(store, "games")));
  await assertFails(getDoc(doc(store, "games/draft")));
});

test("plans: only active plans are readable and nobody writes from the client", async () => {
  const store = db("alice");
  await assertSucceeds(getDoc(doc(store, "plans/pro")));
  await assertFails(getDoc(doc(store, "plans/old")));
  await assertSucceeds(getDocs(query(collection(store, "plans"), where("active", "==", true))));
  await assertFails(getDocs(collection(store, "plans")));
  await assertFails(setDoc(doc(store, "plans/free"), { name: "Free", active: true }));
  await assertFails(getDoc(doc(db(), "plans/pro")));
});

test("gameplay history is private and queries must constrain owner", async () => {
  const store = db("alice");
  await assertSucceeds(getDocs(query(collection(store, "gameplays"), where("ownerId", "==", "alice"), orderBy("createdAt", "desc"), limit(50))));
  await assertSucceeds(getDocs(query(collection(store, "gameplays"), where("ownerId", "==", "alice"), where("status", "==", "approved"), orderBy("createdAt", "desc"), limit(50))));
  await assertFails(getDoc(doc(store, "gameplays/run-b")));
  await assertFails(getDocs(collection(store, "gameplays")));
  await assertFails(updateDoc(doc(store, "gameplays/run-a"), { status: "approved", score: 999999 }));
  await assertFails(setDoc(doc(store, "gameplays/forged"), { ownerId: "alice", status: "approved" }));
});

test("player stats are private to the owner and written only by the server", async () => {
  await assertSucceeds(getDoc(doc(db("alice"), "playerStats/alice_earthion")));
  await assertFails(getDoc(doc(db("bob"), "playerStats/alice_earthion")));
  await assertFails(getDoc(doc(db(), "playerStats/alice_earthion")));
  await assertFails(updateDoc(doc(db("alice"), "playerStats/alice_earthion"), { credits: 0 }));
  await assertFails(setDoc(doc(db("alice"), "playerStats/alice_forged"), { ownerId: "alice", credits: 0 }));
});

test("both ranking queries work only for published seasons and reject score edits", async () => {
  const store = db("bob");
  await assertSucceeds(getDocs(query(collection(store, "seasons/current/gameRankings/earthion/entries"), orderBy("bestScore", "desc"), orderBy("achievedAt"), limit(100))));
  await assertSucceeds(getDocs(query(collection(store, "seasons/current/globalRankings"), orderBy("totalRating", "desc"), orderBy("playerId"), limit(100))));
  await assertFails(getDoc(doc(store, "seasons/draft/globalRankings/alice")));
  await assertFails(setDoc(doc(store, "seasons/current/globalRankings/bob"), { totalRating: 50 }));
  await assertFails(updateDoc(doc(store, "seasons/current/gameRankings/earthion/entries/alice"), { bestScore: 999 }));
});

test("wallets and ledger are private, server writes cannot be forged", async () => {
  await assertSucceeds(getDoc(doc(db("alice"), "wallets/alice")));
  await assertSucceeds(getDoc(doc(db("alice"), "wallets/alice/entries/debit")));
  await assertFails(getDoc(doc(db("bob"), "wallets/alice")));
  await assertFails(updateDoc(doc(db("alice"), "wallets/alice"), { balance: 999 }));
  await assertFails(setDoc(doc(db("alice"), "wallets/alice/entries/credit"), { amount: 999 }));
});

test("internal jobs, unknown paths and nested private documents default to denied", async () => {
  const store = db("alice");
  await assertFails(getDoc(doc(store, "analysisJobs/internal")));
  await assertFails(setDoc(doc(store, "unknown/document"), { data: true }));
  await assertFails(setDoc(doc(store, "users/alice/roles/admin"), { enabled: true }));
  await assertFails(setDoc(doc(store, "publicProfiles/alice"), { displayName: "Forged" }));
});

test("room chat: players read and post in published rooms only, as themselves", async () => {
  const message = (extra = {}) => ({ authorId: "alice", authorName: "Alice", text: "bora rank?",
    createdAt: serverTimestamp(), ...extra });
  const alice = db("alice");
  const recent = (store, room) => query(collection(store, `rooms/${room}/messages`), orderBy("createdAt", "desc"), limit(50));
  await assertFails(getDocs(recent(db(), "earthion")));
  await assertSucceeds(getDocs(recent(alice, "earthion")));
  await assertFails(getDocs(recent(alice, "draft")));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/messages/m1"), message()));
  await assertFails(setDoc(doc(alice, "rooms/draft/messages/m2"), message()));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m3"), message({ authorId: "bob" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m4"), message({ authorName: "Admin" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m5"), message({ text: "" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m6"), message({ text: "x".repeat(501) })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m7"), message({ createdAt: now })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/messages/m8"), message({ pinned: true })));
  await assertFails(updateDoc(doc(alice, "rooms/earthion/messages/m1"), { text: "editado" }));
  await assertFails(setDoc(doc(db("bob"), "rooms/earthion/messages/m9"), message({ authorId: "bob", authorName: "Bob" })));
});

test("moderation (Lab Users Manager): muted can't post, banned/disabled can't post or show presence", async () => {
  const message = (id) => setDoc(doc(db("alice"), `rooms/earthion/messages/${id}`), { authorId: "alice", authorName: "Alice", text: "oi", createdAt: serverTimestamp() });
  const presence = () => setDoc(doc(db("alice"), "rooms/earthion/presence/alice"), { name: "Alice", state: "online", lastSeen: serverTimestamp() });
  const moderate = (flags) => environment.withSecurityRulesDisabled((context) => setDoc(doc(context.firestore(), "moderation/alice"), flags));
  const read = () => getDocs(query(collection(db("alice"), "rooms/earthion/messages"), orderBy("createdAt", "desc"), limit(50)));
  // Documento sem bandeiras: liberado.
  await moderate({ banned: false, muted: false, disabled: false, chatHidden: false });
  await assertSucceeds(message("ok"));
  await assertSucceeds(read());
  await assertSucceeds(presence());
  await moderate({ muted: true });
  await assertFails(message("muted"));
  await assertSucceeds(read());
  await assertSucceeds(presence());
  // Sem visibilidade no chat: não lê nem posta, mas continua aparecendo na sala.
  await moderate({ chatHidden: true });
  await assertFails(message("hidden"));
  await assertFails(read());
  await assertSucceeds(presence());
  for (const flag of ["banned", "disabled"]) {
    await moderate({ [flag]: true });
    await assertFails(message(flag));
    await assertFails(read());
    await assertFails(presence());
  }
  // O jogador lê só a própria moderação; ninguém grava pelo app.
  await assertSucceeds(getDoc(doc(db("alice"), "moderation/alice")));
  await assertFails(getDoc(doc(db("bob"), "moderation/alice")));
  await assertFails(setDoc(doc(db("alice"), "moderation/alice"), { banned: false }));
  // Lista de quem some do chat: todo jogador logado lê.
  await assertSucceeds(getDoc(doc(db("bob"), "chatModeration/hidden")));
  await assertFails(getDoc(doc(db(), "chatModeration/hidden")));
  await assertFails(setDoc(doc(db("bob"), "chatModeration/hidden"), { uids: [] }));
});

test("room presence: each player writes only their own entry, with profile name and server time", async () => {
  const presence = (extra = {}) => ({ name: "Alice", state: "online", lastSeen: serverTimestamp(), ...extra });
  const alice = db("alice");
  await assertFails(getDocs(collection(db(), "rooms/earthion/presence")));
  await assertSucceeds(getDocs(collection(alice, "rooms/earthion/presence")));
  await assertFails(getDocs(collection(alice, "rooms/draft/presence")));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence()));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "playing" })));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "away" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "offline" })));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "live", liveVideoId: "dQw4w9WgXcQ", liveViewers: 3 })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "live" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "online", liveVideoId: "dQw4w9WgXcQ", liveViewers: 3 })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "live", liveVideoId: "x/../y", liveViewers: 3 })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "live", liveVideoId: "dQw4w9WgXcQ", liveViewers: -1 })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/bob"), presence()));
  await assertFails(setDoc(doc(alice, "rooms/draft/presence/alice"), presence()));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ name: "Admin" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ lastSeen: now })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ score: 1 })));
  // Jogo aberto na conta: id e título juntos, para a lista de jogadores da sala.
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "playing", playingGameId: "darius-gaiden", playingTitle: "Darius Gaiden" })));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "live", liveVideoId: "dQw4w9WgXcQ", liveViewers: 3, playingGameId: "earthion", playingTitle: "Earthion" })));
  // Modo do jogo aberto ao lado do título: só os três modos, e só junto com o jogo.
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "playing", playingGameId: "ketsui", playingTitle: "Ketsui", playingKind: "ranked" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "playing", playingGameId: "ketsui", playingTitle: "Ketsui", playingKind: "pro" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "playing", playingKind: "training" })));
  // Dupla online (com quem) e modo do Ranked; ping até os pontos de referência (sem IP).
  const k = { state: "playing", playingGameId: "ketsui", playingTitle: "Ketsui" };
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ ...k, playingKind: "training", playingWith: "Bob" })));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ ...k, playingKind: "ranked", playingMode: "caravan" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ ...k, playingKind: "training", playingMode: "highscore" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ ...k, playingKind: "ranked", playingMode: "1cc" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ playingWith: "Bob" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ ...k, playingWith: "B".repeat(65) })));
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ net: { sa: 14, use: 129, euw: 153 } })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ net: { sa: 14, ip: 1 } })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ net: { sa: "14" } })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ net: { sa: 0 } })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ net: {} })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ playingGameId: "darius-gaiden" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ playingGameId: "x/../y", playingTitle: "X" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ playingGameId: "ketsui", playingTitle: "" })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ playingGameId: "ketsui", playingTitle: "K".repeat(81) })));
  // Chegada na sala (ordem da lista): a hora de agora; depois, o mesmo valor; nunca uma hora antiga.
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ joinedAt: serverTimestamp() })));
  const joinedAt = (await getDoc(doc(alice, "rooms/earthion/presence/alice"))).get("joinedAt");
  await assertSucceeds(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ state: "away", joinedAt })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ joinedAt: now })));
  await assertFails(setDoc(doc(alice, "rooms/earthion/presence/alice"), presence({ joinedAt: "ontem" })));
  await assertFails(deleteDoc(doc(db("bob"), "rooms/earthion/presence/alice")));
  await assertSucceeds(deleteDoc(doc(alice, "rooms/earthion/presence/alice")));
});

test("remote launch: only the owner reads and writes small, known fields", async () => {
  const command = { id: "c1", action: "play", gameId: "earthion", platform: "steam", kind: "ranked", mode: "1cc", createdAt: 1 };
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { command }));
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { desktop: { seenAt: 2, owned: { earthion: ["steam"] } } }, { mergeFields: ["desktop"] }));
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { result: { id: "c1", status: "started", message: "ok", at: 3 } }, { mergeFields: ["result"] }));
  await assertSucceeds(getDoc(doc(db("alice"), "remoteLaunch/alice")));
  await assertFails(getDoc(doc(db("bob"), "remoteLaunch/alice")));
  await assertFails(setDoc(doc(db("bob"), "remoteLaunch/alice"), { command }));
  await assertFails(setDoc(doc(db("alice"), "remoteLaunch/alice"), { command: { ...command, platform: "mister" } }));
  await assertFails(setDoc(doc(db("alice"), "remoteLaunch/alice"), { command: { ...command, extra: true } }));
  await assertFails(setDoc(doc(db("alice"), "remoteLaunch/alice"), { other: 1 }));
  // Jogo aberto na conta (um por vez): o dono grava e apaga; campos e valores conferidos.
  const playing = { gameId: "ketsui", title: "Ketsui", platform: "mister", kind: "training", device: "lite", startedAt: 1, beatAt: 2 };
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { playing }, { mergeFields: ["playing"] }));
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { playing: deleteField() }, { mergeFields: ["playing"] }));
  await assertFails(setDoc(doc(db("bob"), "remoteLaunch/alice"), { playing }, { mergeFields: ["playing"] }));
  await assertFails(setDoc(doc(db("alice"), "remoteLaunch/alice"), { playing: { ...playing, device: "tv" } }, { mergeFields: ["playing"] }));
  await assertFails(setDoc(doc(db("alice"), "remoteLaunch/alice"), { playing: { ...playing, extra: true } }, { mergeFields: ["playing"] }));
  // Jogo marcado pelo agente do MiSTer (servidor): o Client continua gravando o resto do documento.
  await environment.withSecurityRulesDisabled((context) => setDoc(doc(context.firestore(), "remoteLaunch/alice"), { playing: { ...playing, device: "mister", kind: "ranked" } }));
  await assertSucceeds(setDoc(doc(db("alice"), "remoteLaunch/alice"), { desktop: { seenAt: 1, owned: {} } }, { mergeFields: ["desktop"] }));
});

test("chat safety: each player keeps their own block list", async () => {
  const blocks = (uids) => ({ uids, updatedAt: serverTimestamp() });
  await assertSucceeds(setDoc(doc(db("alice"), "chatBlocks/alice"), blocks(["bob"])));
  await assertSucceeds(getDoc(doc(db("alice"), "chatBlocks/alice")));
  await assertFails(getDoc(doc(db("bob"), "chatBlocks/alice")));
  await assertFails(setDoc(doc(db("bob"), "chatBlocks/alice"), blocks([])));
  await assertFails(setDoc(doc(db("alice"), "chatBlocks/alice"), blocks(["alice"])));
  await assertFails(setDoc(doc(db("alice"), "chatBlocks/alice"), blocks(Array.from({ length: 501 }, (_, i) => `u${i}`))));
  await assertFails(setDoc(doc(db("alice"), "chatBlocks/alice"), { uids: [], updatedAt: now }));
  await assertFails(setDoc(doc(db("alice"), "chatBlocks/alice"), { ...blocks([]), note: "x" }));
});

test("chat safety: reports point to a real message, one per player, read only by the reporter", async () => {
  await environment.withSecurityRulesDisabled(async (context) => {
    await setDoc(doc(context.firestore(), "rooms/earthion/messages/m1"), { authorId: "bob", authorName: "Bob", text: "spam spam", createdAt: now });
  });
  const report = (extra = {}) => ({ reporterId: "alice", authorId: "bob", authorName: "Bob", roomId: "earthion",
    messageId: "m1", text: "spam spam", reason: "spam", createdAt: serverTimestamp(), ...extra });
  const alice = db("alice");
  await assertSucceeds(getDoc(doc(alice, "chatReports/m1__alice")));
  await assertFails(setDoc(doc(alice, "chatReports/m1__alice"), report({ text: "outra coisa" })));
  await assertFails(setDoc(doc(alice, "chatReports/m1__alice"), report({ authorId: "carol" })));
  await assertFails(setDoc(doc(alice, "chatReports/m1__alice"), report({ reason: "chato" })));
  await assertFails(setDoc(doc(alice, "chatReports/m1__alice"), report({ reporterId: "bob" })));
  await assertFails(setDoc(doc(alice, "chatReports/m2__alice"), report({ messageId: "m2" })));
  await assertFails(setDoc(doc(alice, "chatReports/m1__carol"), report()));
  await assertSucceeds(setDoc(doc(alice, "chatReports/m1__alice"), report()));
  await assertSucceeds(getDoc(doc(alice, "chatReports/m1__alice")));
  await assertFails(setDoc(doc(alice, "chatReports/m1__alice"), report({ reason: "abuse" })));
  await assertFails(getDoc(doc(db("bob"), "chatReports/m1__alice")));
  await assertFails(getDocs(collection(alice, "chatReports")));
  await assertFails(setDoc(doc(db("bob"), "chatReports/m1__bob"), report({ reporterId: "bob" })));
});

test("netplay: invite to play, only the two players read and each side changes its own part", async () => {
  const key = "0123456789abcdef0123456789abcdef";
  const invite = (extra = {}) => ({ hostUid: "alice", hostName: "Alice", guestUid: "bob", guestName: "Bob", gameId: "earthion",
    title: "Earthion", key, status: "invited", active: true, createdAt: serverTimestamp(), ...extra });
  const alice = db("alice");
  const bob = db("bob");
  const side = { port: 50000, candidates: ["192.168.0.2:50000", "203.0.113.5:50000"] };
  // Convite: só o anfitrião, num jogo publicado, sem endereços ainda.
  await assertFails(setDoc(doc(bob, "netplaySessions/s1"), invite()));
  await assertFails(setDoc(doc(alice, "netplaySessions/s1"), invite({ guestUid: "alice" })));
  await assertFails(setDoc(doc(alice, "netplaySessions/s1"), invite({ gameId: "draft" })));
  await assertFails(setDoc(doc(alice, "netplaySessions/s1"), invite({ status: "ready" })));
  await assertFails(setDoc(doc(alice, "netplaySessions/s1"), invite({ host: side })));
  await assertFails(setDoc(doc(alice, "netplaySessions/s1"), invite({ key: "curta" })));
  await assertSucceeds(setDoc(doc(alice, "netplaySessions/s1"), invite()));
  // Leitura: os dois, e o convidado acha os convites dele.
  await assertSucceeds(getDoc(doc(bob, "netplaySessions/s1")));
  await assertFails(getDoc(doc(db("carol"), "netplaySessions/s1")));
  await assertSucceeds(getDocs(query(collection(bob, "netplaySessions"), where("guestUid", "==", "bob"), where("active", "==", true))));
  await assertFails(getDocs(query(collection(db("carol"), "netplaySessions"), where("active", "==", true))));
  // O convidado aceita e publica os endereços dele; não pode trocar o jogo nem marcar ready.
  await assertFails(updateDoc(doc(bob, "netplaySessions/s1"), { status: "ready" }));
  await assertFails(updateDoc(doc(bob, "netplaySessions/s1"), { status: "accepted", gameId: "outro" }));
  await assertFails(updateDoc(doc(bob, "netplaySessions/s1"), { status: "accepted", host: side }));
  await assertFails(updateDoc(doc(bob, "netplaySessions/s1"), { status: "accepted", guest: { port: 0, candidates: [] } }));
  await assertSucceeds(updateDoc(doc(bob, "netplaySessions/s1"), { status: "accepted", guest: side, updatedAt: serverTimestamp() }));
  // O anfitrião publica os dele e marca ready; não mexe na parte do convidado.
  await assertFails(updateDoc(doc(alice, "netplaySessions/s1"), { status: "ready", guest: { port: 1, candidates: ["198.51.100.1:1"] } }));
  await assertFails(updateDoc(doc(alice, "netplaySessions/s1"), { status: "accepted" }));
  await assertSucceeds(updateDoc(doc(alice, "netplaySessions/s1"), { status: "ready", host: side }));
  // Qualquer um encerra; um estranho não.
  await assertFails(updateDoc(doc(db("carol"), "netplaySessions/s1"), { status: "ended", active: false }));
  await assertSucceeds(updateDoc(doc(bob, "netplaySessions/s1"), { status: "ended", active: false, reason: "partner_left" }));
  await assertFails(deleteDoc(doc(bob, "netplaySessions/s1")));
  await assertSucceeds(deleteDoc(doc(alice, "netplaySessions/s1")));
});
