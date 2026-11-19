import styles from "./download.module.css";

export default function Footer() {
  return (
    <footer
      className={styles.footer}
      style={{ backgroundImage: "url(/download/bg_rodape.jpg)" }}
    >
      <div className={styles.footerContent}>
        <div className={styles.footerLogo}>
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src="/download/logo_rodape.png" alt="ShmupX Logo" />
        </div>

        <div className={styles.footerText}>
          <p className={styles.footerTitle}>Prepare-se e junte-se à batalha!</p>
          <p>© 2025 ShmupX. Todos os direitos reservados.</p>
          <p>
            <a href="#">Termos e Condições de Serviço.</a>
          </p>
        </div>

        <div className={styles.footerLine} />

        <div className={styles.footerSocial}>
          <p className={styles.footerTitle}>Siga-nos no</p>
          <div className={styles.footerLinks}>
            <a
              href="https://youtube.com/@shmupx"
              target="_blank"
              rel="noopener noreferrer"
              className={`${styles.footerIcon} ${styles.youtube}`}
              aria-label="YouTube"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M23.5 6.2a3 3 0 0 0-2.1-2.1C19.5 3.6 12 3.6 12 3.6s-7.5 0-9.4.5A3 3 0 0 0 .5 6.2C0 8.1 0 12 0 12s0 3.9.5 5.8a3 3 0 0 0 2.1 2.1c1.9.5 9.4.5 9.4.5s7.5 0 9.4-.5a3 3 0 0 0 2.1-2.1c.5-1.9.5-5.8.5-5.8s0-3.9-.5-5.8ZM9.6 15.6V8.4l6.2 3.6-6.2 3.6Z" />
              </svg>
            </a>
            <a
              href="https://instagram.com/shmupx"
              target="_blank"
              rel="noopener noreferrer"
              className={`${styles.footerIcon} ${styles.instagram}`}
              aria-label="Instagram"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M12 2.2c3.2 0 3.6 0 4.8.1 3.3.1 4.8 1.7 4.9 4.9.1 1.3.1 1.6.1 4.8s0 3.6-.1 4.8c-.1 3.2-1.7 4.8-4.9 4.9-1.3.1-1.6.1-4.8.1s-3.6 0-4.8-.1c-3.3-.1-4.8-1.7-4.9-4.9C2.2 15.6 2.2 15.2 2.2 12s0-3.6.1-4.8C2.4 3.9 4 2.4 7.2 2.3 8.4 2.2 8.8 2.2 12 2.2ZM12 0C8.7 0 8.3 0 7.1.1 2.7.3.3 2.7.1 7.1 0 8.3 0 8.7 0 12s0 3.7.1 4.9c.2 4.4 2.6 6.8 7 7C8.3 24 8.7 24 12 24s3.7 0 4.9-.1c4.4-.2 6.8-2.6 7-7 .1-1.2.1-1.6.1-4.9s0-3.7-.1-4.9c-.2-4.4-2.6-6.8-7-7C15.7 0 15.3 0 12 0Zm0 5.8a6.2 6.2 0 1 0 0 12.4 6.2 6.2 0 0 0 0-12.4ZM12 16a4 4 0 1 1 0-8 4 4 0 0 1 0 8Zm6.4-11.8a1.4 1.4 0 1 0 0 2.9 1.4 1.4 0 0 0 0-2.9Z" />
              </svg>
            </a>
          </div>
        </div>
      </div>
      <div
        className={styles.footerWarning}
        style={{ backgroundImage: "url(/download/warning_rodape.png)" }}
      />
    </footer>
  );
}
