"use client";

import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import styles from "./download.module.css";
import Footer from "./Footer";

const techElement = "/download/tech_element.png";
const heroBg = "/download/bg_top.jpg";
const logo = "/download/logo.png";

// Preencha as URLs quando os instaladores estiverem publicados.
// Plataformas sem URL aparecem como "COMING SOON".
const VERSION = "0.1.0";
const RELEASE_DATE = "2026";

type PlatformId = "windows" | "linux" | "macos";

const PLATFORMS: {
  id: PlatformId;
  name: string;
  detail: string;
  file: string;
  url: string;
  icon: ReactNode;
}[] = [
  {
    id: "windows",
    name: "WINDOWS",
    detail: "Windows 10 / 11 · 64-bit",
    file: `ShmupX-Setup-${VERSION}.exe`,
    url: "",
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M3 5.5 10.5 4.5v6.8H3V5.5Zm8.7-1.1L21 3v8.3h-9.3V4.4ZM3 12.7h7.5v6.8L3 18.5v-5.8Zm8.7 0H21V21l-9.3-1.3v-7Z" />
      </svg>
    ),
  },
  {
    id: "linux",
    name: "LINUX",
    detail: "AppImage · x86_64",
    file: `ShmupX-${VERSION}.AppImage`,
    url: "",
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 2c-2.2 0-3.5 1.9-3.5 4.6 0 1.5-.4 2.4-1.2 3.7C6.2 12 5 13.8 5 16c0 1 .4 1.8 1.1 2.3-.3.9-.9 1.5-2.1 1.9v1.3c2 0 3.5-.4 4.5-1.2.8.4 1.6.7 3.5.7s2.7-.3 3.5-.7c1 .8 2.5 1.2 4.5 1.2v-1.3c-1.2-.4-1.8-1-2.1-1.9.7-.5 1.1-1.3 1.1-2.3 0-2.2-1.2-4-2.3-5.7-.8-1.3-1.2-2.2-1.2-3.7C15.5 3.9 14.2 2 12 2Zm-1.4 5.2a.9.9 0 1 1 0 1.8.9.9 0 0 1 0-1.8Zm2.8 0a.9.9 0 1 1 0 1.8.9.9 0 0 1 0-1.8Z" />
      </svg>
    ),
  },
  {
    id: "macos",
    name: "macOS",
    detail: "macOS 12+ · Apple Silicon / Intel",
    file: `ShmupX-${VERSION}.dmg`,
    url: "",
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M16.4 12.6c0-2.3 1.9-3.4 2-3.5-1.1-1.6-2.8-1.8-3.4-1.8-1.4-.1-2.8.9-3.5.9s-1.8-.8-3-.8c-1.5 0-3 .9-3.8 2.3-1.6 2.8-.4 7 1.2 9.3.8 1.1 1.7 2.4 2.9 2.3 1.2 0 1.6-.8 3-.8s1.8.8 3 .7c1.3 0 2.1-1.1 2.8-2.2.9-1.3 1.2-2.5 1.3-2.6-.1 0-2.5-1-2.5-3.8ZM14.2 5.8c.6-.8 1.1-1.8.9-2.8-.9 0-2 .6-2.6 1.4-.6.7-1.1 1.7-.9 2.7 1 .1 2-.5 2.6-1.3Z" />
      </svg>
    ),
  },
];

const STEPS = [
  { n: "01", title: "DOWNLOAD", text: "Choose your platform and grab the installer." },
  { n: "02", title: "INSTALL", text: "Run the installer and follow the on-screen steps." },
  { n: "03", title: "LOG IN", text: "Sign in with your ShmupX account to sync your scores." },
  { n: "04", title: "PLAY & RANK", text: "Pick a game, set your score and climb the ranking." },
];

const REQUIREMENTS: [string, string][] = [
  ["OS", "Windows 10+, Ubuntu 20.04+, macOS 12+"],
  ["CPU", "Dual-core 2.0 GHz or better"],
  ["RAM", "4 GB"],
  ["GPU", "OpenGL 3.3 compatible"],
  ["STORAGE", "500 MB free space"],
  ["NETWORK", "Broadband connection for ranking sync"],
];

function detectPlatform(): PlatformId {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("mac")) return "macos";
  if (ua.includes("linux") && !ua.includes("android")) return "linux";
  return "windows";
}

function SectionTitle({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <div className={styles.sectionHeader} id={id}>
      <div className={styles.techElementWrapper}>
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src={techElement} alt="" className={styles.techElementImg} />
        <span className={styles.techElementLabel}>{children}</span>
      </div>
      <span className={styles.sectionLine} />
    </div>
  );
}

export default function DownloadView() {
  const [selected, setSelected] = useState<PlatformId>("windows");

  // Detecta o sistema só no cliente (evita mismatch de hidratação).
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSelected(detectPlatform());
  }, []);

  const handleDownload = (platform: (typeof PLATFORMS)[number]) => {
    if (!platform.url) return;
    window.location.href = platform.url;
  };

  const main = PLATFORMS.find((p) => p.id === selected) ?? PLATFORMS[0];

  return (
    <div className={styles.page}>
      {/* ── Hero ── */}
      <section
        className={styles.hero}
        style={{ "--hero-bg": `url(${heroBg})` } as CSSProperties}
      >
        <div className={styles.heroInner}>
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={logo} alt="ShmupX" className={styles.heroLogo} />

          <h1 className={styles.welcome}>Bem-vindo ao ShmupX!</h1>
          <p className={styles.tagline}>A ARENA GLOBAL DOS SHMUPS</p>
          <p className={styles.heroText}>
            ShmupX é a plataforma definitiva para jogadores de shoot &apos;em up
            (shmup) do mundo todo. Compita em eventos sazonais, suba nos
            rankings globais e mostre seus high scores para provar que você
            está entre os melhores.
          </p>

          <div className={styles.ctaGroup}>
            <button
              type="button"
              className={styles.primaryBtn}
              disabled={!main.url}
              onClick={() => handleDownload(main)}
            >
              <span className={styles.btnIcon}>{main.icon}</span>
              <span>
                {main.url ? `DOWNLOAD FOR ${main.name}` : `${main.name} · COMING SOON`}
              </span>
            </button>

            <a className={styles.otherLink} href="#platforms">
              Other platforms
            </a>
          </div>

          <ul className={styles.chips}>
            <li>v{VERSION}</li>
            <li>{RELEASE_DATE}</li>
            <li>{main.detail}</li>
          </ul>
        </div>
      </section>

      <div className={styles.container}>
        {/* ── Platforms ── */}
        <SectionTitle id="platforms">PLATFORMS</SectionTitle>
        <div className={styles.grid}>
          {PLATFORMS.map((p) => (
            <div
              key={p.id}
              className={`${styles.card} ${p.id === selected ? styles.cardSelected : ""}`}
            >
              {p.id === selected && <span className={styles.badge}>YOUR SYSTEM</span>}
              <div className={styles.cardIcon}>{p.icon}</div>
              <h3 className={styles.cardName}>{p.name}</h3>
              <p className={styles.cardDetail}>{p.detail}</p>
              <p className={styles.cardFile}>{p.file}</p>
              <button
                type="button"
                className={styles.cardBtn}
                disabled={!p.url}
                onClick={() => handleDownload(p)}
              >
                {p.url ? "DOWNLOAD" : "COMING SOON"}
              </button>
            </div>
          ))}
        </div>

        {/* ── Getting started + requirements side by side ── */}
        <div className={styles.twoCol}>
          <div className={styles.col}>
            <SectionTitle>HOW TO START</SectionTitle>
            <ol className={styles.steps}>
              {STEPS.map((s) => (
                <li key={s.n} className={styles.step}>
                  <span className={styles.stepNum}>{s.n}</span>
                  <div>
                    <h4 className={styles.stepTitle}>{s.title}</h4>
                    <p className={styles.stepText}>{s.text}</p>
                  </div>
                </li>
              ))}
            </ol>
          </div>

          <div className={styles.col}>
            <SectionTitle>REQUIREMENTS</SectionTitle>
            <div className={styles.reqBox}>
              {REQUIREMENTS.map(([k, v]) => (
                <div key={k} className={styles.reqRow}>
                  <span className={styles.reqKey}>{k}</span>
                  <span className={styles.reqValue}>{v}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      <Footer />
    </div>
  );
}
