import { useState } from "react";
import { useI18n } from "@client/i18n";
import { useGameArt } from "@client/data/gameArt";
import { PLAY_MODES } from "@client/data/gameModes";
import { readMisterPlayer } from "@client/data/misterPlayer";
import { avatarOf, usePublicProfiles } from "@client/data/publicProfile";
import type { GameRoom } from "@client/types/room";
import { ALL_PLATFORMS, PLATFORM_LABEL, SessionBar, platformsFor, useMisterCompatible, usePlay } from "./play";
import type { PlayPreset } from "./prefs";
import { APP_VERSION } from "./version";

function RoomCard({ room, preset, onOpen }: { room: GameRoom; preset: PlayPreset; onOpen: () => void }) {
  const { t } = useI18n();
  const art = useGameArt(room);
  const play = usePlay();
  const misterOk = useMisterCompatible(room);
  const platforms = platformsFor(room, misterOk);
  const fits = platforms.includes(preset.platform);
  const running = play.session?.room.id === room.id || play.external?.gameId === room.id;
  const someoneRunning = play.session !== null || play.external !== null;
  const error = play.error?.roomId === room.id ? play.error.message : null;
  return (
    <div className={`lite-room-row ${running ? "is-running" : ""}`}>
      <div className="lite-room-card">
        <button type="button" className="lite-room-open" onClick={onOpen} aria-label={t("Entrar na sala de {game}", { game: room.title })}>
          <img src={art.src} alt="" loading="lazy" onError={art.onError} className={art.store ? "is-store" : undefined} />
          <span className="lite-room-info">
            <strong>{room.title}</strong>
            {platforms.length > 0 && <span className="lite-tags">
              {platforms.map((p) => <span key={p} className={`lite-mister-badge ${p === preset.platform ? "is-preset" : ""}`}>{t(PLATFORM_LABEL[p])}</span>)}
            </span>}
            {room.developer && <small>{room.developer}{room.year ? ` · ${room.year}` : ""}</small>}
          </span>
        </button>
        {platforms.length > 0 && (
          <button
            type="button"
            className={`lite-quick-play ${fits ? "" : "is-off"}`}
            disabled={play.busy || (someoneRunning && !running) || running}
            aria-label={t("Jogar {game} agora", { game: room.title })}
            title={t("Jogar com a configuração acima")}
            onClick={() => void play.quickPlay(room, preset, misterOk)}
          >
            {running ? <span className="lite-playing-dot" aria-label={t("Jogando")} /> : <span className="lite-play-arrow" aria-hidden="true">▶</span>}
          </button>
        )}
      </div>
      {error && <p className="lite-error lite-row-error" role="alert">{t(error)}</p>}
    </div>
  );
}

/** Como o ▶ da lista joga (e com o que a folha da sala já vem marcada). */
function PresetBar({ preset, onChange, onSettings }: { preset: PlayPreset; onChange: (next: PlayPreset) => void; onSettings: () => void }) {
  const { t } = useI18n();
  const play = usePlay();
  const hint = preset.platform === "mister"
    ? (readMisterPlayer() ? null : <>{t("Ligue o MiSTer para jogar na TV.")} <button type="button" className="text-action" onClick={onSettings}>{t("Ajustes")}</button></>)
    : (play.pcOnline ? null : t("Steam e Emulador abrem no seu PC: deixe o ShmupX Client aberto no PC com a mesma conta."));
  return (
    <section className="lite-preset" aria-label={t("Como jogar")}>
      <div className="lite-preset-row" role="radiogroup" aria-label={t("Plataforma")}>
        {ALL_PLATFORMS.map((p) => (
          <button key={p} type="button" role="radio" aria-checked={preset.platform === p} onClick={() => onChange({ ...preset, platform: p })}>{t(PLATFORM_LABEL[p])}</button>
        ))}
      </div>
      <div className="lite-preset-row" role="radiogroup" aria-label={t("Modo de jogo")}>
        {PLAY_MODES.map((m) => (
          <button key={m.id} type="button" role="radio" aria-checked={preset.mode === m.id || (m.id === "highscore" && preset.mode === "1cc")} disabled={m.soon} onClick={() => onChange({ ...preset, mode: m.id })}>
            {m.label}{m.soon && <small>{t("Coming soon")}</small>}
          </button>
        ))}
      </div>
      <div className="lite-preset-row" role="radiogroup" aria-label={t("Treino ou ranking")}>
        <button type="button" role="radio" aria-checked={preset.kind === "training"} onClick={() => onChange({ ...preset, kind: "training" })}>{t("Treino")}<small>{t("sem ficha")}</small></button>
        <button type="button" role="radio" aria-checked={preset.kind === "ranked"} onClick={() => onChange({ ...preset, kind: "ranked" })}>{t("Ranked")}<small>{t("gasta 1 ficha")}</small></button>
      </div>
      {hint && <p className="lite-preset-hint">{hint}</p>}
    </section>
  );
}

/** Tela inicial: Salas (com a configuração de jogo e o ▶ em cada jogo) e Ranking. */
export default function RoomList({ rooms, uid, profileName, preset, onPreset, onOpen, onSettings, onSignOut }: {
  rooms: GameRoom[];
  uid: string;
  profileName: string;
  preset: PlayPreset;
  onPreset: (next: PlayPreset) => void;
  onOpen: (id: string) => void;
  onSettings: () => void;
  onSignOut: () => void;
}) {
  const { t } = useI18n();
  const play = usePlay();
  const avatars = usePublicProfiles([uid]);
  const [query, setQuery] = useState("");
  const search = query.trim().toLocaleLowerCase("pt-BR");
  const visible = rooms.filter((room) => room.title.toLocaleLowerCase("pt-BR").includes(search));
  const generalError = play.error?.roomId === null ? play.error.message : null;
  return (
    <main className="lite-screen">
      <header className="lite-topbar">
        <img src="/img/logo.png" alt="ShmupX" className="lite-topbar-logo" />
        <span className="lite-badge small">LITE</span>
        <span className="lite-version">v{APP_VERSION}</span>
        <button type="button" className="lite-icon-button lite-gear" onClick={onSettings} aria-label={t("Ajustes")} title={t("Ajustes")}>⚙</button>
        <button type="button" className="lite-profile" onClick={onSignOut} title={t("Sair da conta")}>
          <img src={avatarOf(avatars, uid)} alt="" />
          <span>{profileName}</span>
        </button>
      </header>
      {(play.session || play.external) && <div className="lite-play lite-home-session"><SessionBar /></div>}
      <div className="lite-content">
          <PresetBar preset={preset} onChange={onPreset} onSettings={onSettings} />
          {generalError && <p className="lite-error" role="alert">{t(generalError)}</p>}
          <input className="lite-search" type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("Buscar jogo…")} aria-label={t("Buscar jogo")} />
          {rooms.length === 0 && <p className="lite-muted lite-center-text">{t("Carregando as salas…")}</p>}
          <div className="lite-room-list">
            {visible.map((room) => <RoomCard key={room.id} room={room} preset={preset} onOpen={() => onOpen(room.id)} />)}
          </div>
          <p className="lite-note">{t("Toque no jogo para entrar na sala (chat, jogadores e ranking), ou no ▶ para jogar direto com a configuração acima.")}</p>
      </div>
    </main>
  );
}
