"""Chamada opcional à API do xAI (Grok) para conferir recortes de baixa confiança."""
import base64
import os
import re

import cv2

API_URL = "https://api.x.ai/v1/chat/completions"
# preços por 1M tokens (US$) – ajuste com --price-in/--price-out se mudarem
DEFAULT_PRICE_IN = 2.00
DEFAULT_PRICE_OUT = 6.00


class Grok:
    def __init__(self, model="grok-4.7", timeout=60):
        self.key = os.environ.get("XAI_API_KEY")
        if not self.key:
            raise SystemExit("--grok usado, mas a variável de ambiente XAI_API_KEY não está definida.")
        self.model = model
        self.timeout = timeout
        self.prompt_tokens = 0
        self.completion_tokens = 0
        self.total_tokens = 0
        self.calls = 0

    def read_number(self, img_bgr, what="the 8-digit SCORE"):
        import requests  # importado só quando necessário
        ok, png = cv2.imencode(".png", img_bgr)
        b64 = base64.b64encode(png.tobytes()).decode()
        body = {
            "model": self.model,
            "temperature": 0,
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": f"data:image/png;base64,{b64}", "detail": "low"}},
                    {"type": "text", "text": f"Read {what} shown in this video game HUD crop. "
                                              "Answer with digits only, no spaces or commas. "
                                              "Note: in this pixel font a 3 has small left-side strokes; "
                                              "an 8 has a full horizontal bar in the middle."},
                ],
            }],
        }
        r = requests.post(API_URL, json=body, timeout=self.timeout,
                          headers={"Authorization": f"Bearer {self.key}", "Content-Type": "application/json"})
        r.raise_for_status()
        data = r.json()
        u = data.get("usage", {}) or {}
        self.prompt_tokens += int(u.get("prompt_tokens", 0) or 0)
        self.completion_tokens += int(u.get("completion_tokens", 0) or 0)
        self.total_tokens += int(u.get("total_tokens", 0) or 0)
        self.calls += 1
        txt = data["choices"][0]["message"]["content"] or ""
        m = re.search(r"\d{3,}", txt.replace(",", "").replace(".", ""))
        return int(m.group()) if m else None

    def cost_usd(self, price_in=DEFAULT_PRICE_IN, price_out=DEFAULT_PRICE_OUT):
        return self.prompt_tokens / 1e6 * price_in + self.completion_tokens / 1e6 * price_out
