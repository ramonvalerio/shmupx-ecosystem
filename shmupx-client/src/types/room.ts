export interface GameRoom {
  id: string;
  title: string;
  playersOnline: number;
  /** app_id da Steam usado por `inicializar_jogo_steam` (steam://run/<app_id>). */
  steamAppId: number;
}

export interface ChatMessage {
  id: string;
  author: string;
  text: string;
  timestamp: string;
}
