"""Leitura de vídeo só com OpenCV (sem ffmpeg externo)."""
import cv2

# Opcional (--progress): chamado com a fração do vídeo (0 a 1) já percorrida na passada sequencial.
progress_hook = None


class Video:
    def __init__(self, path):
        self.path = path
        self.cap = cv2.VideoCapture(path)
        if not self.cap.isOpened():
            raise SystemExit(f"Não foi possível abrir o vídeo: {path}")
        self.fps = self.cap.get(cv2.CAP_PROP_FPS) or 30.0
        if self.fps <= 1 or self.fps > 240:
            self.fps = 30.0
        self.nframes = int(self.cap.get(cv2.CAP_PROP_FRAME_COUNT) or 0)
        self.width = int(self.cap.get(cv2.CAP_PROP_FRAME_WIDTH))
        self.height = int(self.cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
        self.frames_read = 0  # quadros efetivamente analisados

    @property
    def duration(self):
        return self.nframes / self.fps if self.nframes else 0.0

    def frame_at(self, t):
        """Quadro BGR mais próximo do tempo t (s)."""
        self.cap.set(cv2.CAP_PROP_POS_MSEC, max(0.0, t) * 1000.0)
        ok, frame = self.cap.read()
        if ok:
            self.frames_read += 1
        return frame if ok else None

    def sequential(self, step_s):
        """Passa pelo vídeo inteiro uma vez; devolve (t, frame) a cada step_s segundos.
        grab() sem retrieve() evita conversão de cor nos quadros pulados."""
        self.cap.set(cv2.CAP_PROP_POS_FRAMES, 0)
        step = max(1, int(round(step_s * self.fps)))
        idx = 0
        last_idx = -1
        while True:
            if not self.cap.grab():
                break
            if idx % step == 0:
                ok, frame = self.cap.retrieve()
                if ok:
                    self.frames_read += 1
                    yield idx / self.fps, frame
                    if progress_hook is not None and self.nframes:
                        progress_hook(idx / self.nframes)
            last_idx = idx
            idx += 1
        # corrige duração quando o contêiner informa contagem errada
        if last_idx + 1 > 0:
            self.nframes = last_idx + 1

    def window(self, t0, t1, fps):
        """Quadros entre t0 e t1 com taxa aproximada fps (busca + leitura sequencial)."""
        t0 = max(0.0, t0)
        self.cap.set(cv2.CAP_PROP_POS_MSEC, t0 * 1000.0)
        step = max(1, int(round(self.fps / fps)))
        i = 0
        while True:
            if not self.cap.grab():
                break
            t = self.cap.get(cv2.CAP_PROP_POS_MSEC) / 1000.0
            if t > t1:
                break
            if i % step == 0:
                ok, frame = self.cap.retrieve()
                if ok:
                    self.frames_read += 1
                    yield t, frame
            i += 1
