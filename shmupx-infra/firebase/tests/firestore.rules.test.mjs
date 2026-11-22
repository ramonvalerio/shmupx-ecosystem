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
      "gameplays/run-a": { ownerId: "alice", status: "approved", createdAt: now },
      "gameplays/run-b": { ownerId: "bob", status: "analyzing", createdAt: now },
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

test("gameplay history is private and queries must constrain owner", async () => {
  const store = db("alice");
  await assertSucceeds(getDocs(query(collection(store, "gameplays"), where("ownerId", "==", "alice"), orderBy("createdAt", "desc"), limit(50))));
  await assertSucceeds(getDocs(query(collection(store, "gameplays"), where("ownerId", "==", "alice"), where("status", "==", "approved"), orderBy("createdAt", "desc"), limit(50))));
  await assertFails(getDoc(doc(store, "gameplays/run-b")));
  await assertFails(getDocs(collection(store, "gameplays")));
  await assertFails(updateDoc(doc(store, "gameplays/run-a"), { status: "approved", score: 999999 }));
  await assertFails(setDoc(doc(store, "gameplays/forged"), { ownerId: "alice", status: "approved" }));
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
  await assertFails(deleteDoc(doc(db("bob"), "rooms/earthion/presence/alice")));
  await assertSucceeds(deleteDoc(doc(alice, "rooms/earthion/presence/alice")));
});
