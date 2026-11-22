"""Imagens da página do ShmupX Lite na Google Play: ícone 512x512 (PNG 32 bits) e imagem de
destaque 1024x500 (cockpit do login com o logo e o selo LITE). Rodar da raiz do shmupx-ecosystem."""
from PIL import Image, ImageDraw, ImageFont, ImageFilter

SAIDA = "shmupx-mobile/store"

# Ícone: o mesmo do app (já 512x512).
icone = Image.open("shmupx-mobile/src-tauri/icons/icon.png").convert("RGBA").resize((512, 512), Image.LANCZOS)
icone.save(f"{SAIDA}/icon-512.png")

# Destaque: fundo do cockpit, escurecido, com o logo e o selo.
fundo = Image.open("shmupx-client/public/img/login-cockpit.jpg").convert("RGB")
escala = 1024 / fundo.width
fundo = fundo.resize((1024, round(fundo.height * escala)), Image.LANCZOS)
topo = (fundo.height - 500) // 2
fundo = fundo.crop((0, topo, 1024, topo + 500)).filter(ImageFilter.GaussianBlur(1.2))
escuro = Image.new("RGB", fundo.size, (6, 10, 14))
fundo = Image.blend(fundo, escuro, 0.45).convert("RGBA")

logo = Image.open("shmupx-client/public/img/login-logo.png").convert("RGBA")
largura = 560
logo = logo.resize((largura, round(logo.height * largura / logo.width)), Image.LANCZOS)
fundo.alpha_composite(logo, ((1024 - largura) // 2, 120))

desenho = ImageDraw.Draw(fundo)
def fonte(tamanho):
    for nome in ("seguisb.ttf", "segoeuib.ttf", "arialbd.ttf", "DejaVuSans-Bold.ttf"):
        try:
            return ImageFont.truetype(nome, tamanho)
        except OSError:
            continue
    return ImageFont.load_default()

ciano = (87, 252, 255)
selo = fonte(34)
texto = "L I T E"
caixa = desenho.textbbox((0, 0), texto, font=selo)
w, h = caixa[2] - caixa[0], caixa[3] - caixa[1]
x, y = (1024 - w) // 2, 120 + logo.height + 28
desenho.rectangle((x - 22, y - 12, x + w + 22, y + h + 18), outline=ciano, width=3)
desenho.text((x, y - caixa[1] + 2), texto, font=selo, fill=ciano)

frase = fonte(26)
legenda = "Salas, chat e lives do ShmupX no seu celular"
caixa = desenho.textbbox((0, 0), legenda, font=frase)
desenho.text(((1024 - (caixa[2] - caixa[0])) // 2, y + h + 50), legenda, font=frase, fill=(220, 236, 240))

fundo.convert("RGB").save(f"{SAIDA}/feature-1024x500.png")
print("ok:", f"{SAIDA}/icon-512.png", f"{SAIDA}/feature-1024x500.png")
