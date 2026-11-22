"""Liga a assinatura de release no projeto Android gerado pelo `tauri android init` (lê
gen/android/keystore.properties, que o CI escreve com o keystore dos secrets)."""
import sys

caminho = sys.argv[1]
texto = open(caminho, encoding="utf-8").read()

# Dentro de `android { }`, `java` é a extensão do Gradle: as classes vêm importadas no topo.
IMPORTS = ["import java.util.Properties", "import java.io.FileInputStream"]

ASSINATURA = """
    signingConfigs {
        create("release") {
            val propriedades = Properties()
            val arquivo = rootProject.file("keystore.properties")
            if (arquivo.exists()) FileInputStream(arquivo).use { entrada -> propriedades.load(entrada) }
            keyAlias = propriedades.getProperty("keyAlias")
            keyPassword = propriedades.getProperty("password")
            storeFile = file(propriedades.getProperty("storeFile"))
            storePassword = propriedades.getProperty("password")
        }
    }
"""

if "signingConfigs {" not in texto:
    faltam = [linha for linha in IMPORTS if linha not in texto]
    if faltam:
        texto = "\n".join(faltam) + "\n" + texto
    inicio = texto.index("android {") + len("android {")
    texto = texto[:inicio] + ASSINATURA + texto[inicio:]
    release = 'getByName("release") {'
    posicao = texto.index(release) + len(release)
    texto = texto[:posicao] + '\n            signingConfig = signingConfigs.getByName("release")' + texto[posicao:]

open(caminho, "w", encoding="utf-8").write(texto)
print("assinatura de release ligada em", caminho)
