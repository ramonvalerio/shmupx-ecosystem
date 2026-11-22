"""Liga a assinatura de release no projeto Android gerado pelo `tauri android init` (lê
gen/android/keystore.properties, que o CI escreve com o keystore dos secrets)."""
import sys

caminho = sys.argv[1]
texto = open(caminho, encoding="utf-8").read()

ASSINATURA = """
    signingConfigs {
        create("release") {
            val propriedades = java.util.Properties()
            val arquivo = rootProject.file("keystore.properties")
            if (arquivo.exists()) java.io.FileInputStream(arquivo).use { propriedades.load(it) }
            keyAlias = propriedades["keyAlias"] as String
            keyPassword = propriedades["password"] as String
            storeFile = file(propriedades["storeFile"] as String)
            storePassword = propriedades["password"] as String
        }
    }
"""

if "signingConfigs {" not in texto:
    inicio = texto.index("android {") + len("android {")
    texto = texto[:inicio] + ASSINATURA + texto[inicio:]
    release = 'getByName("release") {'
    posicao = texto.index(release) + len(release)
    texto = texto[:posicao] + '\n            signingConfig = signingConfigs.getByName("release")' + texto[posicao:]

open(caminho, "w", encoding="utf-8").write(texto)
print("assinatura de release ligada em", caminho)
