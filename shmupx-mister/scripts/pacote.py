"""Monta o pacote do agente que o ShmupX Client instala no MiSTer pela rede (SSH).

    python scripts/pacote.py            # compila e gera target/pacote/

Gera `shmupx-mister-<versão>-<commit>.tar` (mesma estrutura de /media/fat/shmupx: o agente, os
scripts, o jogos.txt, Scripts/ e perfis-novos/) e `latest.json` (versão, endereço e SHA-256). Os
dois vão para s3://.../downloads/mister/ (o Client lê o latest.json e confere o SHA-256).
Os scripts vão com fim de linha LF e permissão de execução; os perfis vão em perfis-novos/ para
o instalador só copiar os que faltam no MiSTer (como o implantar.ps1).
"""
import hashlib, io, json, os, re, subprocess, sys, tarfile, time

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ALVO = "armv7-unknown-linux-musleabihf"
SITE = "https://www.shmupx.com/downloads/mister/"

os.chdir(RAIZ)
subprocess.run(["cargo", "build", "--release", "--target", ALVO], check=True)
versao = re.search(r'^version\s*=\s*"([^"]+)"', open("Cargo.toml", encoding="utf-8").read(), re.M).group(1)
commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
nome = f"shmupx-mister-{versao}-{commit}.tar"
os.makedirs("target/pacote", exist_ok=True)

def adicionar(tar, destino, conteudo, executavel):
    info = tarfile.TarInfo(destino)
    info.size = len(conteudo)
    info.mode = 0o755 if executavel else 0o644
    info.mtime = int(time.time())
    tar.addfile(info, io.BytesIO(conteudo))

with tarfile.open(f"target/pacote/{nome}", "w", format=tarfile.GNU_FORMAT) as tar:
    adicionar(tar, "shmupx-mister", open(f"target/{ALVO}/release/shmupx-mister", "rb").read(), True)
    base = "mister/shmupx"
    for pasta, _, arquivos in os.walk(base):
        for arquivo in sorted(arquivos):
            caminho = os.path.join(pasta, arquivo)
            destino = os.path.relpath(caminho, base).replace(os.sep, "/")
            conteudo = open(caminho, "rb").read()
            if arquivo.endswith((".sh", ".txt")):
                conteudo = conteudo.replace(b"\r\n", b"\n")
            adicionar(tar, destino, conteudo, arquivo.endswith(".sh"))
    for arquivo in sorted(os.listdir("perfis")):
        if arquivo.endswith(".json"):
            adicionar(tar, f"perfis-novos/{arquivo}", open(os.path.join("perfis", arquivo), "rb").read(), False)

dados = open(f"target/pacote/{nome}", "rb").read()
manifesto = {"version": versao, "commit": commit, "url": SITE + nome, "sha256": hashlib.sha256(dados).hexdigest(), "size": len(dados)}
json.dump(manifesto, open("target/pacote/latest.json", "w"), indent=2)
print(json.dumps(manifesto, indent=2))
sys.exit(0)
