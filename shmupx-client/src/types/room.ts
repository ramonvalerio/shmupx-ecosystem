export interface GameRoom {
  id: string;
  title: string;
  playersOnline: number;
  /** app_id da Steam usado por `inicializar_jogo_steam` (steam://run/<app_id>). */
  steamAppId: number;
  /** Imagem do card enviada pelo Lab (WebP 16:9); ausente = cartão tipográfico. */
  coverUrl?: string;
  /** Metadados do catálogo (Lab); ausentes quando o admin não preencheu. */
  platform?: string;
  developer?: string;
  publisher?: string;
  genre?: string;
  year?: number;
  /** Perguntas e respostas do RobotX (Lab); vazio = RobotX ainda indisponível na sala. */
  robotx?: { question: string; answer: string }[];
}

export interface ChatMessage {
  id: string;
  author: string;
  /** uid de quem enviou (Firebase); ausente em mensagens de sistema. */
  authorId?: string;
  text: string;
  timestamp: string;
  /** Ainda não confirmada pelo servidor. */
  pending?: boolean;
  /** Resposta do RobotX gerada neste PC: só quem perguntou vê, nada vai ao servidor. */
  local?: boolean;
  /** Pergunta que a resposta local do RobotX atende (aparece citada acima do texto). */
  inReplyTo?: string;
}
