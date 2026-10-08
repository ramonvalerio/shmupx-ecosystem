export interface GameRoom {
  id: string;
  title: string;
  playersOnline: number;
}

export interface ChatMessage {
  id: string;
  author: string;
  text: string;
  timestamp: string;
}
